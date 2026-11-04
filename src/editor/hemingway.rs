//! Hemingway mode: the text can only be typed, not edited, in typewriter mode.

use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::OpalView;

impl OpalView {
    pub fn hemingway(&self) -> bool {
        self.imp().hemingway.get()
    }

    pub fn set_hemingway(&self, on: bool) {
        let imp = self.imp();
        imp.hemingway.set(on);
        // The mouse falls through to the scrolled window and only scrolls.
        self.set_can_target(!on);
        // The menu has Select All and Delete.
        self.action_set_enabled("menu.popup", !on);
        if on {
            imp.typewriter_before.set(self.typewriter());
            self.set_typewriter(true);
            // Drops the selection and centres the cursor line.
            let buffer = self.buffer();
            buffer.place_cursor(&buffer.iter_at_mark(&buffer.get_insert()));
            self.set_overwrite(false);
        } else {
            self.set_typewriter(imp.typewriter_before.get());
        }
    }

    /// Stops the keys that have no virtual method to override in `imp.rs`.
    /// Connect before autocorrect: a stopped undo must not reach its handlers,
    /// which expect to see the end of the replay.
    pub(super) fn connect_hemingway(&self) {
        self.connect_select_all(|view, _| {
            if view.hemingway() {
                view.stop_signal_emission_by_name("select-all");
            }
        });
        let buffer = self.buffer();
        let weak = self.downgrade();
        buffer.connect_undo(move |buffer| {
            if weak.upgrade().is_some_and(|view| view.hemingway()) {
                buffer.stop_signal_emission_by_name("undo");
            }
        });
        let weak = self.downgrade();
        buffer.connect_redo(move |buffer| {
            if weak.upgrade().is_some_and(|view| view.hemingway()) {
                buffer.stop_signal_emission_by_name("redo");
            }
        });
    }
}
