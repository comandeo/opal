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

/// Runs the editor: `opal [file]`. It gives the shell its prompt back at once:
/// the editor runs on in a copy detached from the terminal, unless
/// OPAL_FOREGROUND (or OPAL_BENCH) is set.
pub fn run() -> glib::ExitCode {
    // Read before detaching, so that config warnings reach the terminal.
    let config = config::load();
    let foreground = std::env::var_os("OPAL_FOREGROUND").is_some() || std::env::var_os("OPAL_BENCH").is_some();
    if !foreground && detach() {
        return glib::ExitCode::SUCCESS;
    }
    // The GPU renderers cost ~75 ms of startup and load the GPU driver stack;
    // for a text view the CPU renderer draws a frame well within budget. A
    // renderer chosen with GSK_RENDERER still wins.
    if std::env::var_os("GSK_RENDERER").is_none() {
        // SAFETY: GTK has not started yet and the process has a single thread.
        unsafe { std::env::set_var("GSK_RENDERER", "cairo") };
    }
    let path = std::env::args_os().nth(1).map(PathBuf::from);
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

/// Starts a copy of opal with the same arguments that is not tied to the
/// terminal: its own process group and no stdin/stdout/stderr, so closing the
/// terminal or pressing Ctrl+C there leaves it alone. Returns whether it runs.
fn detach() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let mut command = std::process::Command::new(exe);
    command
        .args(std::env::args_os().skip(1))
        .env("OPAL_FOREGROUND", "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    command.spawn().is_ok()
}
