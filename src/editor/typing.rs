//! Typographic replacements while typing; the rules are in `crate::autocorrect`.

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::OpalView;
use crate::autocorrect;
use crate::markdown::Kind;

impl OpalView {
    /// A typed `-` or `"` is looked at right after the keystroke. The
    /// replacement is an undo step of its own, so Ctrl+Z brings the typed text back.
    pub(super) fn connect_autocorrect(&self) {
        let buffer = self.buffer();
        let weak = self.downgrade();
        buffer.connect_insert_text(move |buffer, at, text| {
            let Some(view) = weak.upgrade() else { return };
            let imp = view.imp();
            if !imp.autocorrect.get() || imp.loading.get() || imp.replaying.get() || !matches!(text, "-" | "\"") {
                return;
            }
            // Right gravity: once the char is in, the mark sits right after it.
            imp.typed.borrow_mut().push(buffer.create_mark(None, at, false));
            if imp.typed.borrow().len() == 1 {
                let weak = view.downgrade();
                glib::idle_add_local_once(move || {
                    if let Some(view) = weak.upgrade() {
                        view.autocorrect();
                    }
                });
            }
        });
        // Undo and redo put typed text back; it must not be corrected again.
        for signal in ["undo", "redo"] {
            for after in [false, true] {
                let weak = self.downgrade();
                buffer.connect_local(signal, after, move |_| {
                    if let Some(view) = weak.upgrade() {
                        view.imp().replaying.set(!after);
                    }
                    None
                });
            }
        }
    }

    fn autocorrect(&self) {
        let buffer = self.buffer();
        let marks: Vec<gtk::TextMark> = self.imp().typed.borrow_mut().drain(..).collect();
        for mark in marks {
            let end = buffer.iter_at_mark(&mark).offset();
            buffer.delete_mark(&mark);
            if self.in_code(end) {
                continue;
            }
            let before = buffer.text(&buffer.start_iter(), &buffer.iter_at_offset(end), true);
            let Some(replace) = autocorrect::after_typing(&before) else { continue };
            let chars = before[before.len() - replace.len..].chars().count() as i32;
            let (mut from, mut to) = (buffer.iter_at_offset(end - chars), buffer.iter_at_offset(end));
            buffer.begin_user_action();
            buffer.delete(&mut from, &mut to);
            buffer.insert(&mut from, replace.with);
            buffer.end_user_action();
        }
    }

    /// Whether the char before `char_offset` is in a code span or code block.
    fn in_code(&self, char_offset: i32) -> bool {
        let imp = self.imp();
        let byte = imp.index.borrow().byte_at(char_offset - 1);
        imp.styler
            .borrow()
            .elements()
            .iter()
            .any(|e| matches!(e.kind, Kind::Code | Kind::CodeBlock) && e.range.contains(&byte))
    }
}
