//! Helpers for GTK tests. GTK is single-threaded, so the test binaries use
//! `harness = false` and drive the main loop by hand.
#![allow(dead_code)]

use std::time::{Duration, Instant};

use gtk::glib;
use gtk::prelude::*;

/// Runs `tests` in order, honouring an optional name filter (first argument).
pub fn run(tests: &[(&str, fn())]) {
    let filter = std::env::args().skip(1).find(|a| !a.starts_with('-'));
    let mut failed = Vec::new();
    for (name, test) in tests {
        if filter.as_deref().is_some_and(|f| !name.contains(f)) {
            continue;
        }
        match std::panic::catch_unwind(*test) {
            Ok(()) => println!("test {name} ... ok"),
            Err(_) => {
                println!("test {name} ... FAILED");
                failed.push(*name);
            }
        }
    }
    if !failed.is_empty() {
        eprintln!("failed: {failed:?}");
        std::process::exit(1);
    }
}

/// Processes main-loop events for `duration`.
pub fn pump_for(duration: Duration) {
    let ctx = glib::MainContext::default();
    let end = Instant::now() + duration;
    while Instant::now() < end {
        while ctx.iteration(false) {}
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Lets a few frames render.
pub fn settle() {
    pump_for(Duration::from_millis(150));
}

pub fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !cond() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        pump_for(Duration::from_millis(10));
    }
}

/// Puts `child` in a scrolled window and waits until it is on screen.
pub fn show(child: &impl IsA<gtk::Widget>) -> gtk::Window {
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(child)
        .build();
    let window = gtk::Window::builder().default_width(900).default_height(700).child(&scroller).build();
    window.present();
    wait_until("window on screen", || child.is_mapped() && child.width() > 0);
    settle();
    window
}

/// Renders `widget` as currently shown into a PNG file.
pub fn save_png(widget: &impl IsA<gtk::Widget>, path: &str) {
    let (w, h) = (widget.width() as f32, widget.height() as f32);
    let paintable = gtk::WidgetPaintable::new(Some(widget));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, w as f64, h as f64);
    let node = snapshot.to_node().expect("the widget drew something");
    let renderer = widget.native().and_then(|n| n.renderer()).expect("the widget is on screen");
    let texture = renderer.render_texture(&node, Some(&gtk::graphene::Rect::new(0.0, 0.0, w, h)));
    texture.save_to_png(path).expect("PNG written");
}
