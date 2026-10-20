//! State of the OpalView widget.

use std::cell::{Cell, OnceCell, RefCell};

use gtk::gdk;
use gtk::glib;
use gtk::subclass::prelude::*;

use super::tags::{Palette, Tags};
use crate::offsets::CharIndex;
use crate::restyle::Styler;

#[derive(Default)]
pub struct OpalView {
    pub styler: RefCell<Styler>,
    /// Byte ↔ char mapping of the current text.
    pub index: RefCell<CharIndex>,
    pub tags: OnceCell<Tags>,
    pub palette: OnceCell<Palette>,
    /// Column width in characters and the font metrics that turn it into pixels.
    pub column_chars: Cell<u32>,
    pub char_width: Cell<f64>,
    pub line_height: Cell<i32>,
    pub typewriter: Cell<bool>,
    /// Current side and top/bottom margins in pixels.
    pub side: Cell<i32>,
    pub vertical: Cell<i32>,
    /// Set while a document is loaded: the signal handlers stay quiet.
    pub loading: Cell<bool>,
    /// A mouse button is held (e.g. selecting): typewriter mode must not scroll.
    pub mouse_down: Cell<bool>,
    /// `OPAL_BENCH=restyle`: print how long every restyle took.
    pub bench: Cell<bool>,
    /// Check boxes drawn in the last frame: rectangle in buffer coordinates and
    /// byte offset of the character between the brackets.
    pub checkboxes: RefCell<Vec<(gdk::Rectangle, usize)>>,
    /// Typographic replacements while typing are on.
    pub autocorrect: Cell<bool>,
    /// Marks right after typed `-` and `"` waiting to be looked at.
    pub typed: RefCell<Vec<gtk::TextMark>>,
    /// Inside undo or redo: re-inserted text is not corrected.
    pub replaying: Cell<bool>,
}

#[glib::object_subclass]
impl ObjectSubclass for OpalView {
    const NAME: &'static str = "OpalView";
    type Type = super::OpalView;
    type ParentType = gtk::TextView;
}

impl ObjectImpl for OpalView {}
impl WidgetImpl for OpalView {
    fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
        self.parent_size_allocate(width, height, baseline);
        self.obj().update_margins(width, height);
    }
}
impl TextViewImpl for OpalView {
    fn snapshot_layer(&self, layer: gtk::TextViewLayer, snapshot: gtk::Snapshot) {
        self.parent_snapshot_layer(layer, snapshot.clone());
        self.obj().draw_decorations(layer, &snapshot);
    }
}
