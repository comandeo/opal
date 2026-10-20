//! GTK-level tests of the window: opening, autosave, conflicts.
//! Run: `cargo test --test gtk_window [name-filter]` (needs a display).

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use gtk::gio;
use gtk::prelude::*;
use opal::config::{Config, Theme};
use opal::editor::OpalView;

thread_local! {
    static APP: gtk::Application = {
        let app = gtk::Application::builder()
            .application_id("opal.WindowTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(gio::Cancellable::NONE).expect("the application registers");
        app
    };
}

fn main() {
    gtk::init().expect("GTK needs a display (Wayland or X11)");
    common::run(&[
        ("opens_file_and_titles_window", opens_file_and_titles_window),
        ("autosaves_after_a_pause", autosaves_after_a_pause),
        ("ctrl_s_saves_at_once", ctrl_s_saves_at_once),
        ("close_flushes_pending_edit", close_flushes_pending_edit),
        ("external_change_is_not_overwritten", external_change_is_not_overwritten),
        ("missing_file_is_created_on_save", missing_file_is_created_on_save),
        ("non_utf8_file_is_refused", non_utf8_file_is_refused),
        ("nul_file_is_refused", nul_file_is_refused),
        ("failed_save_still_lets_the_window_close", failed_save_still_lets_the_window_close),
        ("typewriter_action_toggles", typewriter_action_toggles),
        ("theme_follows_config", theme_follows_config),
        ("window_starts_without_gtk_warnings", window_starts_without_gtk_warnings),
    ]);
}

fn open(path: Option<&Path>) -> (gtk::ApplicationWindow, OpalView) {
    let window = APP.with(|app| opal::window::build(app, &Config::default(), path.map(Path::to_path_buf)));
    let view = window
        .child()
        .and_downcast::<gtk::ScrolledWindow>()
        .and_then(|s| s.child())
        .and_downcast::<OpalView>()
        .expect("window → scrolled window → view");
    common::wait_until("window on screen", || view.is_mapped());
    (window, view)
}

fn title(window: &gtk::ApplicationWindow) -> String {
    window.title().map(|t| t.to_string()).unwrap_or_default()
}

fn append(view: &OpalView, text: &str) {
    let buffer = view.buffer();
    buffer.insert(&mut buffer.end_iter(), text);
}

fn note(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let path = dir.path().join("note.md");
    fs::write(&path, "# Hi\n").unwrap();
    path
}

/// Visible dialogs (AlertDialog shows a window transient for ours).
fn dialogs(window: &gtk::ApplicationWindow) -> Vec<gtk::Window> {
    let parent: gtk::Window = window.clone().upcast();
    gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Window>().ok())
        .filter(|w| w.is_visible() && w.transient_for().as_ref() == Some(&parent))
        .collect()
}

/// Clicks the button labelled `label` somewhere inside `widget`.
fn click(widget: &gtk::Widget, label: &str) -> bool {
    if let Some(button) = widget.downcast_ref::<gtk::Button>() {
        if button.label().as_deref() == Some(label) {
            button.emit_clicked();
            return true;
        }
    }
    let mut child = widget.first_child();
    while let Some(c) = child {
        if click(&c, label) {
            return true;
        }
        child = c.next_sibling();
    }
    false
}

fn dismiss_dialogs(window: &gtk::ApplicationWindow) {
    for dialog in dialogs(window) {
        dialog.close();
    }
    common::wait_until("dialogs closed", || dialogs(window).is_empty());
}

fn opens_file_and_titles_window() {
    let dir = tempfile::tempdir().unwrap();
    let path = note(&dir);
    let (window, view) = open(Some(&path));
    assert_eq!(view.text(), "# Hi\n");
    assert_eq!(title(&window), "note.md — opal");
    window.destroy();
}

fn autosaves_after_a_pause() {
    let dir = tempfile::tempdir().unwrap();
    let path = note(&dir);
    let (window, view) = open(Some(&path));
    append(&view, "x");
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Hi\n", "not saved before the pause");
    common::pump_for(Duration::from_millis(1500));
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Hi\nx");
    window.destroy();
}

fn ctrl_s_saves_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = note(&dir);
    let (window, view) = open(Some(&path));
    append(&view, "y");
    WidgetExt::activate_action(&window, "win.save", None).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Hi\ny");
    window.destroy();
}

fn close_flushes_pending_edit() {
    let dir = tempfile::tempdir().unwrap();
    let path = note(&dir);
    let (window, view) = open(Some(&path));
    append(&view, "z");
    window.close();
    common::wait_until("window closed", || !window.is_visible());
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Hi\nz");
}

fn external_change_is_not_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let path = note(&dir);
    let (window, view) = open(Some(&path));
    fs::write(&path, "theirs, longer").unwrap();
    append(&view, "w");
    common::pump_for(Duration::from_millis(1500));
    assert_eq!(fs::read_to_string(&path).unwrap(), "theirs, longer");
    assert!(title(&window).starts_with("• "), "title: {}", title(&window));
    window.destroy();
}

fn missing_file_is_created_on_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("new.md");
    let (window, view) = open(Some(&path));
    assert_eq!(view.text(), "");
    assert_eq!(title(&window), "new.md — opal");
    append(&view, "new");
    WidgetExt::activate_action(&window, "win.save", None).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    window.destroy();
}

fn non_utf8_file_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.md");
    fs::write(&path, [0xff, 0xfe, b'a']).unwrap();
    let (window, view) = open(Some(&path));
    assert_eq!(view.text(), "");
    assert_eq!(title(&window), "Без имени — opal");
    window.destroy();
}

fn nul_file_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nul.md");
    fs::write(&path, b"keep\0me").unwrap();
    let (window, view) = open(Some(&path));
    assert_eq!(view.text(), "");
    assert_eq!(title(&window), "Без имени — opal");
    append(&view, "x");
    common::pump_for(Duration::from_millis(1500));
    assert_eq!(fs::read(&path).unwrap(), b"keep\0me", "the file must stay untouched");
    window.destroy();
}

fn failed_save_still_lets_the_window_close() {
    let dir = tempfile::tempdir().unwrap();
    let path = note(&dir);
    let (window, view) = open(Some(&path));
    append(&view, "z");
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
    if fs::write(dir.path().join("probe"), "").is_ok() {
        // Running as root: directory permissions are not enforced.
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        window.destroy();
        return;
    }
    for _ in 0..2 {
        // Every explicit Ctrl+S reports the failure, not only the first one.
        WidgetExt::activate_action(&window, "win.save", None).unwrap();
        common::wait_until("save error shown", || !dialogs(&window).is_empty());
        dismiss_dialogs(&window);
    }
    window.close();
    common::wait_until("a way out offered", || !dialogs(&window).is_empty());
    let dialog = dialogs(&window).remove(0);
    assert!(click(dialog.upcast_ref(), "Не сохранять"), "no «Не сохранять» button");
    common::wait_until("window closed", || !window.is_visible());
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Hi\n");
}

fn typewriter_action_toggles() {
    let (window, view) = open(None);
    let before = view.typewriter();
    WidgetExt::activate_action(&window, "win.typewriter", None).unwrap();
    assert_ne!(view.typewriter(), before);
    window.destroy();
}

/// GTK 4.20 deprecated `gtk-application-prefer-dark-theme` (setting it prints a
/// warning on every start); newer GTK must get `gtk-interface-color-scheme`.
fn theme_follows_config() {
    let settings = gtk::Settings::default().expect("a display");
    for (theme, nick) in [(Theme::Dark, "dark"), (Theme::Light, "light")] {
        let config = Config { theme, ..Config::default() };
        let window = APP.with(|app| opal::window::build(app, &config, None));
        if gtk::check_version(4, 20, 0).is_none() {
            let value = settings.property_value("gtk-interface-color-scheme");
            let (_, scheme) = gtk::glib::EnumValue::from_value(&value).expect("an enum property");
            assert_eq!(scheme.nick(), nick);
        } else {
            assert_eq!(settings.is_gtk_application_prefer_dark_theme(), theme == Theme::Dark);
        }
        window.destroy();
    }
}

/// Runs one window test in a child process: GTK must not print warnings
/// (they would reach the user's terminal on every start).
fn window_starts_without_gtk_warnings() {
    let out = std::process::Command::new(std::env::current_exe().expect("test binary path"))
        .arg("opens_file_and_titles_window")
        .output()
        .expect("the test binary runs");
    assert!(out.status.success(), "child failed: {out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("WARNING"), "GTK printed warnings:\n{stderr}");
}
