//! Centred column and typewriter mode.

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::OpalView;
use crate::layout;
use crate::restyle::Style;

const QUOTE_INDENT: i32 = 20;
/// Room left of list items for the drawn bullet or check box.
pub(super) const LIST_INDENT: i32 = 24;

impl OpalView {
    pub fn typewriter(&self) -> bool {
        self.imp().typewriter.get()
    }

    pub fn set_typewriter(&self, on: bool) {
        let imp = self.imp();
        imp.typewriter.set(on);
        imp.vertical.set(layout::vertical_margin(self.height(), imp.line_height.get(), on));
        self.apply_margins();
    }

    /// Called on every allocation. Margins change the allocation themselves,
    /// so they are applied later and only when they differ.
    pub(super) fn update_margins(&self, width: i32, height: i32) {
        let imp = self.imp();
        let side = layout::side_margin(width, imp.column_chars.get(), imp.char_width.get());
        let vertical = layout::vertical_margin(height, imp.line_height.get(), imp.typewriter.get());
        if side == imp.side.get() && vertical == imp.vertical.get() {
            return;
        }
        imp.side.set(side);
        imp.vertical.set(vertical);
        let weak = self.downgrade();
        glib::idle_add_local_once(move || {
            if let Some(view) = weak.upgrade() {
                view.apply_margins();
            }
        });
    }

    fn apply_margins(&self) {
        let imp = self.imp();
        let (side, vertical) = (imp.side.get(), imp.vertical.get());
        self.set_left_margin(side);
        self.set_right_margin(side);
        self.set_top_margin(vertical);
        self.set_bottom_margin(vertical);
        // A tag's left margin replaces the view's one for its lines.
        let tags = imp.tags.get().expect("tags are created in setup");
        tags.get(Style::Quote).set_property("left-margin", side + QUOTE_INDENT);
        tags.get(Style::ListItem).set_property("left-margin", side + LIST_INDENT);
        self.recenter();
    }
}
