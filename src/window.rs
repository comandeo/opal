//! The application window: one document, autosave and dialogs.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::config::{Config, Theme};
use crate::document::{self, SaveOutcome, Stamp};
use crate::editor::OpalView;

const AUTOSAVE_DELAY: Duration = Duration::from_secs(1);
const UNTITLED: &str = "Без имени";

struct Ui {
    window: gtk::ApplicationWindow,
    view: OpalView,
    path: RefCell<Option<PathBuf>>,
    stamp: Cell<Option<Stamp>>,
    autosave: RefCell<Option<glib::SourceId>>,
    /// Autosave waits until the user resolves a conflict with an external change.
    paused: Cell<bool>,
    /// The last save failed.
    failed: Cell<bool>,
    /// Closing was confirmed; the next close request goes through.
    closing: Cell<bool>,
}

/// Builds and shows the window for `path`, or for an empty document.
pub fn build(app: &gtk::Application, config: &Config, path: Option<PathBuf>) -> gtk::ApplicationWindow {
    if let Some(settings) = gtk::Settings::default() {
        apply_theme(&settings, config.theme);
    }
    let view = OpalView::new(config);
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&view)
        .build();
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .default_width(1000)
        .default_height(800)
        .child(&scroller)
        .build();
    let ui = Rc::new(Ui {
        window: window.clone(),
        view,
        path: RefCell::new(None),
        stamp: Cell::new(None),
        autosave: RefCell::new(None),
        paused: Cell::new(false),
        failed: Cell::new(false),
        closing: Cell::new(false),
    });

    let buffer = ui.view.buffer();
    let weak = Rc::downgrade(&ui);
    buffer.connect_changed(move |_| {
        if let Some(ui) = weak.upgrade() {
            schedule_autosave(&ui);
        }
    });
    let weak = Rc::downgrade(&ui);
    buffer.connect_modified_changed(move |_| {
        if let Some(ui) = weak.upgrade() {
            update_title(&ui);
        }
    });
    install_actions(&ui, app);
    // The window owns the state through this handler.
    let owner = ui.clone();
    window.connect_close_request(move |_| on_close_request(&owner));

    if std::env::var("OPAL_BENCH").as_deref() == Ok("startup") {
        let app = app.clone();
        window.connect_realize(move |w| {
            if let Some(clock) = w.frame_clock() {
                let app = app.clone();
                clock.connect_after_paint(move |_| app.quit());
            }
        });
    }

    window.present();
    update_title(&ui);
    if let Some(path) = path {
        open_path(&ui, path);
    }
    ui.view.grab_focus();
    window
}

/// GTK 4.20 replaced `gtk-application-prefer-dark-theme` (setting it now prints
/// a warning) with `gtk-interface-color-scheme`, which the GTK 4.10 bindings do
/// not know, so it is set by name.
fn apply_theme(settings: &gtk::Settings, theme: Theme) {
    if gtk::check_version(4, 20, 0).is_none() {
        let name = "gtk-interface-color-scheme";
        let nick = if theme == Theme::Dark { "dark" } else { "light" };
        let value = settings
            .find_property(name)
            .and_then(|pspec| glib::EnumClass::with_type(pspec.value_type()))
            .and_then(|class| class.to_value_by_nick(nick));
        if let Some(value) = value {
            settings.set_property_from_value(name, &value);
        }
    } else {
        settings.set_gtk_application_prefer_dark_theme(theme == Theme::Dark);
    }
}

fn install_actions(ui: &Rc<Ui>, app: &gtk::Application) {
    let add = |name: &str, accel: &str, activate: fn(&Rc<Ui>)| {
        let action = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(ui);
        action.connect_activate(move |_, _| {
            if let Some(ui) = weak.upgrade() {
                activate(&ui);
            }
        });
        ui.window.add_action(&action);
        app.set_accels_for_action(&format!("win.{name}"), &[accel]);
    };
    add("save", "<Control>s", |ui| {
        if ui.path.borrow().is_some() {
            save(ui, false);
        } else {
            save_as(ui, |_| {});
        }
    });
    add("open", "<Control>o", |ui| finish_document(ui, open_dialog));
    add("new", "<Control>n", |ui| finish_document(ui, |ui| show_document(ui, None, None, "")));
    add("typewriter", "<Control>t", |ui| ui.view.set_typewriter(!ui.view.typewriter()));
    add("close", "<Control>q", |ui| ui.window.close());
}

fn on_close_request(ui: &Rc<Ui>) -> glib::Propagation {
    if ui.closing.get() {
        return glib::Propagation::Proceed;
    }
    finish_document(ui, |ui| {
        ui.closing.set(true);
        let window = ui.window.clone();
        glib::idle_add_local_once(move || window.close());
    });
    glib::Propagation::Stop
}

fn update_title(ui: &Ui) {
    let path = ui.path.borrow();
    let name = path
        .as_ref()
        .and_then(|p| p.file_name())
        .map_or_else(|| UNTITLED.to_string(), |n| n.to_string_lossy().into_owned());
    let unsaved =
        ui.view.buffer().is_modified() && (path.is_none() || ui.failed.get() || ui.paused.get());
    let mark = if unsaved { "• " } else { "" };
    ui.window.set_title(Some(&format!("{mark}{name} — opal")));
}

fn alert(ui: &Ui, message: &str, detail: &str) {
    gtk::AlertDialog::builder()
        .message(message)
        .detail(detail)
        .modal(true)
        .build()
        .show(Some(&ui.window));
}

/// Replaces the document shown in the window.
fn show_document(ui: &Rc<Ui>, path: Option<PathBuf>, stamp: Option<Stamp>, text: &str) {
    cancel_autosave(ui);
    ui.view.load_text(text);
    cancel_autosave(ui); // load_text emitted `changed`
    *ui.path.borrow_mut() = path;
    ui.stamp.set(stamp);
    ui.paused.set(false);
    ui.failed.set(false);
    update_title(ui);
}

fn open_path(ui: &Rc<Ui>, path: PathBuf) {
    let path = document::resolve(&path);
    match document::load(&path) {
        Ok((text, stamp)) => show_document(ui, Some(path), stamp, &text),
        Err(e) => alert(ui, &format!("Не удалось открыть «{}»", path.display()), &e.to_string()),
    }
}

fn open_dialog(ui: &Rc<Ui>) {
    let dialog = gtk::FileDialog::builder().title("Открыть").modal(true).build();
    let weak = Rc::downgrade(ui);
    dialog.open(Some(&ui.window), gio::Cancellable::NONE, move |result| {
        let (Some(ui), Ok(file)) = (weak.upgrade(), result) else { return };
        if let Some(path) = file.path() {
            open_path(&ui, path);
        }
    });
}

fn schedule_autosave(ui: &Rc<Ui>) {
    if ui.path.borrow().is_none() || ui.paused.get() {
        return;
    }
    cancel_autosave(ui);
    let weak = Rc::downgrade(ui);
    let id = glib::timeout_add_local_once(AUTOSAVE_DELAY, move || {
        let Some(ui) = weak.upgrade() else { return };
        ui.autosave.borrow_mut().take(); // this source is finishing
        if ui.view.buffer().is_modified() {
            save(&ui, false);
        }
    });
    *ui.autosave.borrow_mut() = Some(id);
}

fn cancel_autosave(ui: &Ui) {
    if let Some(id) = ui.autosave.borrow_mut().take() {
        id.remove();
    }
}

/// Saves to the current path; returns whether the text is now on disk.
fn save(ui: &Rc<Ui>, force: bool) -> bool {
    cancel_autosave(ui);
    let Some(path) = ui.path.borrow().clone() else { return false };
    let text = ui.view.text();
    match document::save(&path, &text, ui.stamp.get(), force) {
        Ok(SaveOutcome::Saved(stamp)) => {
            ui.stamp.set(Some(stamp));
            ui.failed.set(false);
            ui.paused.set(false);
            ui.view.buffer().set_modified(false);
            update_title(ui);
            true
        }
        Ok(SaveOutcome::Conflict) => {
            ui.paused.set(true);
            update_title(ui);
            ask_conflict(ui);
            false
        }
        Err(e) => {
            let first = !ui.failed.replace(true);
            update_title(ui);
            if first {
                alert(ui, "Не удалось сохранить", &e.to_string());
            }
            false
        }
    }
}

fn ask_conflict(ui: &Rc<Ui>) {
    let dialog = gtk::AlertDialog::builder()
        .message("Файл изменён другой программой")
        .detail("Автосохранение приостановлено. Загрузить версию с диска (несохранённые правки пропадут) или перезаписать файл своей версией?")
        .buttons(["Загрузить с диска", "Перезаписать"])
        .default_button(1)
        .modal(true)
        .build();
    let weak = Rc::downgrade(ui);
    dialog.choose(Some(&ui.window), gio::Cancellable::NONE, move |result| {
        let Some(ui) = weak.upgrade() else { return };
        match result {
            Ok(0) => {
                let path = ui.path.borrow().clone();
                if let Some(path) = path {
                    open_path(&ui, path);
                }
            }
            Ok(1) => {
                save(&ui, true);
            }
            _ => {} // dismissed: stays paused; Ctrl+S asks again
        }
    });
}

/// Asks for a path, saves there, then runs `next`.
fn save_as(ui: &Rc<Ui>, next: impl FnOnce(&Rc<Ui>) + 'static) {
    let dialog = gtk::FileDialog::builder()
        .title("Сохранить как")
        .initial_name(format!("{UNTITLED}.md"))
        .modal(true)
        .build();
    let weak = Rc::downgrade(ui);
    dialog.save(Some(&ui.window), gio::Cancellable::NONE, move |result| {
        let (Some(ui), Ok(file)) = (weak.upgrade(), result) else { return };
        let Some(path) = file.path() else { return };
        *ui.path.borrow_mut() = Some(document::resolve(&path));
        ui.stamp.set(None);
        // The dialog already confirmed overwriting an existing file.
        if save(&ui, true) {
            next(&ui);
        }
    });
}

/// Saves the current document, or asks what to do with an untitled one, then
/// runs `next`. If saving fails, `next` does not run (the dialog explains why).
fn finish_document(ui: &Rc<Ui>, next: impl FnOnce(&Rc<Ui>) + 'static) {
    let buffer = ui.view.buffer();
    if !buffer.is_modified() {
        return next(ui);
    }
    if ui.path.borrow().is_some() {
        if save(ui, false) {
            next(ui);
        }
        return;
    }
    if buffer.char_count() == 0 {
        return next(ui);
    }
    let dialog = gtk::AlertDialog::builder()
        .message("Сохранить документ?")
        .detail("У документа нет имени, и он не сохранён.")
        .buttons(["Сохранить", "Не сохранять", "Отмена"])
        .cancel_button(2)
        .default_button(0)
        .modal(true)
        .build();
    let weak = Rc::downgrade(ui);
    dialog.choose(Some(&ui.window), gio::Cancellable::NONE, move |result| {
        let Some(ui) = weak.upgrade() else { return };
        match result {
            Ok(0) => save_as(&ui, next),
            Ok(1) => next(&ui),
            _ => {}
        }
    });
}
