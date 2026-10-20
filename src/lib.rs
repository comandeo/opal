//! opal — a minimalist Markdown editor.

pub mod config;
pub mod document;
pub mod editor;
pub mod layout;
pub mod markdown;
pub mod offsets;
pub mod restyle;
pub mod reveal;
pub mod window;

use std::path::PathBuf;

use gtk::prelude::*;
use gtk::{gio, glib};

/// Runs the editor: `opal [file]`.
pub fn run() -> glib::ExitCode {
    // The GPU renderers cost ~75 ms of startup and load the GPU driver stack;
    // for a text view the CPU renderer draws a frame well within budget. A
    // renderer chosen with GSK_RENDERER still wins.
    if std::env::var_os("GSK_RENDERER").is_none() {
        // SAFETY: GTK has not started yet and the process has a single thread.
        unsafe { std::env::set_var("GSK_RENDERER", "cairo") };
    }
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    let config = config::load();
    let app = gtk::Application::builder()
        .application_id("opal.Editor")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(move |app| {
        window::build(app, &config, path.clone());
    });
    // Arguments are handled above; GTK gets none so it does not try to open files.
    app.run_with_args(&["opal"])
}
