//! Decorations drawn around the text, and clicks on check boxes and links.

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, graphene};

use super::OpalView;
use crate::markdown::Kind;
use crate::restyle::Style;

/// Space between a drawn glyph and the text it precedes.
const GLYPH_GAP: i32 = 6;

impl OpalView {
    pub(super) fn draw_decorations(&self, layer: gtk::TextViewLayer, snapshot: &gtk::Snapshot) {
        let imp = self.imp();
        let Some(palette) = imp.palette.get() else { return };
        let styler = imp.styler.borrow();
        let index = imp.index.borrow();
        let buffer = self.buffer();
        let rect = self.visible_rect();
        // The visible area extends past the text into the typewriter margins;
        // `line_at_y` clamps to the first and the last line.
        let (top, _) = self.line_at_y(rect.y());
        let (mut bottom, _) = self.line_at_y(rect.y() + rect.height());
        bottom.forward_to_line_end();
        let (top, bottom) = (index.byte_at(top.offset()), index.byte_at(bottom.offset()));
        let side = imp.side.get() as f32;
        let column = (self.width() - 2 * imp.side.get()) as f32;
        let iter = |b: usize| buffer.iter_at_offset(index.char_at(b));
        // Top of the first line and bottom of the last line of a byte range.
        let span = |start: usize, end: usize| {
            let (y0, _) = self.line_yrange(&iter(start));
            let (y1, h1) = self.line_yrange(&iter(end.saturating_sub(1).max(start)));
            (y0 as f32, (y1 + h1 - y0) as f32)
        };
        let above = layer == gtk::TextViewLayer::AboveText;
        if above {
            imp.checkboxes.borrow_mut().clear();
        }
        for e in styler.elements().iter().filter(|e| e.range.start <= bottom && e.range.end >= top) {
            let hidden = e.markers.first().is_some_and(|m| !styler.is_visible(&m.range));
            match &e.kind {
                Kind::CodeBlock if !above => {
                    let (y, h) = span(e.range.start, e.range.end);
                    snapshot.append_color(&palette.code_bg, &graphene::Rect::new(side - 8.0, y, column + 16.0, h));
                }
                Kind::Quote if above => {
                    let (y, h) = span(e.range.start, e.range.end);
                    snapshot.append_color(&palette.line, &graphene::Rect::new(side + 2.0, y, 3.0, h));
                }
                Kind::Rule if above && hidden => {
                    let (y, h) = self.line_yrange(&iter(e.range.start));
                    let middle = (y + h / 2) as f32;
                    snapshot.append_color(&palette.line, &graphene::Rect::new(side, middle, column, 1.0));
                }
                Kind::ListItem { ordered: false } if above && hidden => {
                    self.draw_glyph(snapshot, "•", &self.text_start(iter(e.range.start)), &palette.glyph);
                }
                Kind::Task { checked } if above && hidden => {
                    let marker = &e.markers[0].range;
                    let glyph = if *checked { "☑" } else { "☐" };
                    let rect = self.draw_glyph(snapshot, glyph, &self.text_start(iter(e.range.start)), &palette.glyph);
                    let bracket = styler.text()[marker.clone()].find('[').unwrap_or(0);
                    imp.checkboxes.borrow_mut().push((rect, marker.start + bracket + 1));
                }
                _ => {}
            }
        }
    }

    /// Steps back from a list item's start over hidden markup on its line (e.g.
    /// the `> ` of a quote): the item's text is drawn where that run begins, and
    /// GTK reports wrong positions for chars that follow hidden text.
    fn text_start(&self, mut at: gtk::TextIter) -> gtk::TextIter {
        let hidden = self.imp().tags.get().expect("tags are created in setup").get(Style::Hidden);
        let mut prev = at.clone();
        while prev.backward_char() && prev.char() != '\n' && prev.has_tag(hidden) {
            at = prev.clone();
        }
        at
    }

    /// Draws `glyph` left of `at`, the start of a hidden run or of visible text
    /// (positions GTK reports correctly).
    fn draw_glyph(&self, snapshot: &gtk::Snapshot, glyph: &str, at: &gtk::TextIter, color: &gdk::RGBA) -> gdk::Rectangle {
        let loc = self.iter_location(at);
        let layout = self.create_pango_layout(Some(glyph));
        let (w, h) = layout.pixel_size();
        let x = loc.x() - w - GLYPH_GAP;
        let y = loc.y() + (loc.height() - h) / 2;
        snapshot.save();
        snapshot.translate(&graphene::Point::new(x as f32, y as f32));
        snapshot.append_layout(&layout, color);
        snapshot.restore();
        gdk::Rectangle::new(x, y, w, h)
    }

    pub fn checkbox_rects(&self) -> Vec<(gdk::Rectangle, usize)> {
        self.imp().checkboxes.borrow().clone()
    }

    /// Flips `[ ]` ↔ `[x]`; `byte` is the offset of the char between the brackets.
    pub fn toggle_task(&self, byte: usize) {
        let buffer = self.buffer();
        let offset = self.imp().index.borrow().char_at(byte);
        let mut start = buffer.iter_at_offset(offset);
        let mut end = buffer.iter_at_offset(offset + 1);
        let replacement = if start.char() == ' ' { "x" } else { " " };
        buffer.begin_user_action();
        buffer.delete(&mut start, &mut end);
        buffer.insert(&mut start, replacement);
        buffer.end_user_action();
    }

    /// URL of the link at a char offset.
    pub fn link_at(&self, char_offset: i32) -> Option<String> {
        let imp = self.imp();
        let byte = imp.index.borrow().byte_at(char_offset);
        imp.styler.borrow().elements().iter().find_map(|e| match &e.kind {
            Kind::Link { url } if e.range.start <= byte && byte < e.range.end => Some(url.clone()),
            _ => None,
        })
    }

    /// Clicks on drawn check boxes toggle them; Ctrl+click opens links.
    pub(super) fn connect_clicks(&self) {
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_PRIMARY);
        click.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.downgrade();
        click.connect_pressed(move |gesture, _, x, y| {
            let Some(view) = weak.upgrade() else { return };
            let (bx, by) = view.window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);
            if gesture.current_event_state().contains(gdk::ModifierType::CONTROL_MASK) {
                let url = view.iter_at_location(bx, by).and_then(|it| view.link_at(it.offset()));
                if let Some(url) = url {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                    view.open_link(&url);
                    return;
                }
            }
            let hit = view.checkbox_rects().into_iter().find(|(r, _)| r.contains_point(bx, by));
            if let Some((_, byte)) = hit {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                view.toggle_task(byte);
            }
        });
        self.add_controller(click);
    }

    fn open_link(&self, url: &str) {
        let launcher = gtk::UriLauncher::builder().uri(link_uri(url)).build();
        let window = self.root().and_downcast::<gtk::Window>();
        launcher.launch(window.as_ref(), gio::Cancellable::NONE, |result| {
            if let Err(e) = result {
                eprintln!("opal: не удалось открыть ссылку: {e}");
            }
        });
    }
}

/// `<me@x.org>` autolinks carry a bare address; give it a scheme.
pub fn link_uri(url: &str) -> String {
    if !url.contains(':') && url.contains('@') { format!("mailto:{url}") } else { url.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_uris() {
        assert_eq!(link_uri("http://x"), "http://x");
        assert_eq!(link_uri("me@x.org"), "mailto:me@x.org");
        assert_eq!(link_uri("mailto:me@x.org"), "mailto:me@x.org");
        assert_eq!(link_uri("notes.md"), "notes.md");
    }
}
