//! The Markdown text view: styling, hidden markup, typewriter scrolling.

mod decor;
mod imp;
mod margins;
mod tags;
mod typing;

use std::ops::Range;
use std::time::Instant;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, pango};

use crate::config::Config;
use crate::offsets::CharIndex;
use crate::restyle::{Style, TagOps};
use tags::{Palette, Tags};

pub use decor::link_uri;

glib::wrapper! {
    pub struct OpalView(ObjectSubclass<imp::OpalView>)
        @extends gtk::TextView, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Scrollable;
}

impl OpalView {
    pub fn new(config: &Config) -> Self {
        let view: Self = glib::Object::new();
        view.setup(config);
        view
    }

    fn setup(&self, config: &Config) {
        let imp = self.imp();
        self.set_wrap_mode(gtk::WrapMode::WordChar);
        // Typewriter scrolling must be instant; GTK animates `scroll_to_mark`
        // through the scrolled window's adjustment unless animations are off.
        self.settings().set_gtk_enable_animations(false);
        self.add_css_class("opal");
        let font = pango::FontDescription::from_string(&config.font);
        tags::install_font_css(&font);
        let palette = Palette::for_theme(config.theme);
        let _ = imp.tags.set(Tags::new(&self.buffer().tag_table(), config, &palette));
        let _ = imp.palette.set(palette);
        let metrics = self.pango_context().metrics(Some(&font), None);
        imp.char_width.set(metrics.approximate_char_width() as f64 / pango::SCALE as f64);
        imp.line_height.set((metrics.ascent() + metrics.descent()) / pango::SCALE);
        imp.column_chars.set(config.column_width);
        imp.typewriter.set(config.typewriter);
        imp.bench.set(std::env::var("OPAL_BENCH").as_deref() == Ok("restyle"));
        self.connect_buffer_signals();
        self.connect_mouse_tracking();
        self.connect_clicks();
        imp.autocorrect.set(config.autocorrect);
        self.connect_autocorrect();
    }

    /// Replaces the whole text: no undo history, cursor at the start, unmodified.
    pub fn load_text(&self, text: &str) {
        let imp = self.imp();
        let buffer = self.buffer();
        imp.loading.set(true);
        buffer.begin_irreversible_action();
        buffer.set_text(text);
        buffer.end_irreversible_action();
        buffer.place_cursor(&buffer.start_iter());
        imp.loading.set(false);
        let ops = imp.styler.borrow_mut().reset(text, 0..0);
        *imp.index.borrow_mut() = CharIndex::new(text);
        self.apply(&ops);
        buffer.set_modified(false);
        self.recenter();
    }

    /// The document text, including hidden markup.
    pub fn text(&self) -> String {
        let buffer = self.buffer();
        buffer.text(&buffer.start_iter(), &buffer.end_iter(), true).to_string()
    }

    pub fn has_style_at(&self, style: Style, char_offset: i32) -> bool {
        let tag = self.imp().tags.get().expect("tags are created in setup").get(style);
        self.buffer().iter_at_offset(char_offset).has_tag(tag)
    }

    pub fn is_hidden_at(&self, char_offset: i32) -> bool {
        self.has_style_at(Style::Hidden, char_offset)
    }

    pub fn char_width(&self) -> f64 {
        self.imp().char_width.get()
    }

    pub fn line_height(&self) -> i32 {
        self.imp().line_height.get()
    }

    fn connect_buffer_signals(&self) {
        let buffer = self.buffer();
        let weak = self.downgrade();
        buffer.connect_changed(move |_| {
            if let Some(view) = weak.upgrade() {
                view.on_text_changed();
            }
        });
        let weak = self.downgrade();
        buffer.connect_mark_set(move |buffer, _, mark| {
            if *mark == buffer.get_insert() || *mark == buffer.selection_bound() {
                if let Some(view) = weak.upgrade() {
                    view.on_cursor_moved();
                }
            }
        });
    }

    /// Tracks the mouse button so that typewriter mode does not scroll while
    /// the user selects with the mouse; it re-centres on release.
    fn connect_mouse_tracking(&self) {
        let legacy = gtk::EventControllerLegacy::new();
        legacy.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.downgrade();
        legacy.connect_event(move |_, event| {
            if let Some(view) = weak.upgrade() {
                match event.event_type() {
                    gdk::EventType::ButtonPress => view.imp().mouse_down.set(true),
                    gdk::EventType::ButtonRelease => {
                        view.imp().mouse_down.set(false);
                        view.recenter();
                    }
                    _ => {}
                }
            }
            glib::Propagation::Proceed
        });
        self.add_controller(legacy);
    }

    fn on_text_changed(&self) {
        let imp = self.imp();
        if imp.loading.get() {
            return;
        }
        let started = Instant::now();
        let buffer = self.buffer();
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), true);
        let index = CharIndex::new(&text);
        let cursor = cursor_bytes(&buffer, &index);
        let ops = imp.styler.borrow_mut().text_changed(&text, cursor);
        *imp.index.borrow_mut() = index;
        self.apply(&ops);
        if imp.bench.get() {
            eprintln!("opal: restyle {:?}", started.elapsed());
        }
        self.recenter();
    }

    fn on_cursor_moved(&self) {
        let imp = self.imp();
        if imp.loading.get() {
            return;
        }
        let buffer = self.buffer();
        // Inside an insertion the cursor can move before `changed` arrives;
        // the `changed` handler then takes care of the cursor too.
        if buffer.char_count() != imp.index.borrow().char_len() {
            return;
        }
        let cursor = cursor_bytes(&buffer, &imp.index.borrow());
        let ops = imp.styler.borrow_mut().cursor_moved(cursor);
        self.apply(&ops);
        if !imp.mouse_down.get() {
            self.recenter();
        }
    }

    fn apply(&self, ops: &TagOps) {
        let imp = self.imp();
        let tags = imp.tags.get().expect("tags are created in setup");
        let buffer = self.buffer();
        let index = imp.index.borrow();
        let iter = |b: usize| buffer.iter_at_offset(index.char_at(b));
        if let Some(r) = &ops.clear {
            for style in Style::ALL {
                buffer.remove_tag(tags.get(style), &iter(r.start), &iter(r.end));
            }
        }
        for (style, r) in &ops.remove {
            buffer.remove_tag(tags.get(*style), &iter(r.start), &iter(r.end));
        }
        for (style, r) in &ops.add {
            buffer.apply_tag(tags.get(*style), &iter(r.start), &iter(r.end));
        }
    }

    /// In typewriter mode, scrolls so that the cursor line is in the middle.
    fn recenter(&self) {
        if self.imp().typewriter.get() {
            self.scroll_to_mark(&self.buffer().get_insert(), 0.0, true, 0.0, 0.5);
        }
    }
}

/// Cursor and selection bound as a byte range (possibly reversed).
fn cursor_bytes(buffer: &gtk::TextBuffer, index: &CharIndex) -> Range<usize> {
    let insert = buffer.iter_at_mark(&buffer.get_insert()).offset();
    let bound = buffer.iter_at_mark(&buffer.selection_bound()).offset();
    index.byte_at(insert)..index.byte_at(bound)
}
