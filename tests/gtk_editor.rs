//! GTK-level tests of the editor view. They open windows, so they need a
//! display. Run: `cargo test --test gtk_editor [name-filter]`.

mod common;

use std::time::{Duration, Instant};

use gtk::prelude::*;
use opal::config::Config;
use opal::editor::OpalView;
use opal::restyle::Style;

fn main() {
    gtk::init().expect("GTK needs a display (Wayland or X11)");
    common::run(&[
        ("full_text_keeps_hidden_markup", full_text_keeps_hidden_markup),
        ("markup_follows_cursor", markup_follows_cursor),
        ("tag_changes_are_not_undoable", tag_changes_are_not_undoable),
        ("undo_restores_styling", undo_restores_styling),
        ("multibyte_offsets", multibyte_offsets),
        ("crlf_round_trip", crlf_round_trip),
        ("typing_restyles", typing_restyles),
        ("arrows_walk_through_hidden_markup", arrows_walk_through_hidden_markup),
        ("vertical_move_lands_on_text", vertical_move_lands_on_text),
        ("restyle_is_fast", restyle_is_fast),
        ("large_paste_does_not_hang", large_paste_does_not_hang),
        ("column_is_centred", column_is_centred),
        ("typewriter_keeps_cursor_line_centred", typewriter_keeps_cursor_line_centred),
    ]);
}

fn view_with(text: &str) -> OpalView {
    let view = OpalView::new(&Config::default());
    view.load_text(text);
    view
}

fn place(view: &OpalView, offset: i32) {
    let buffer = view.buffer();
    buffer.place_cursor(&buffer.iter_at_offset(offset));
}

fn cursor(view: &OpalView) -> i32 {
    view.buffer().cursor_position()
}

/// Dense prose-like Markdown of at least `len` bytes.
fn prose(len: usize) -> String {
    let block = "Это **жирный** текст и *курсив*, а ещё `код` и [ссылка](http://x). Обычные слова, много слов.\n\n> цитата\n> ещё\n\n- пункт\n- [ ] задача\n\n";
    let mut text = String::from("# Заголовок\n\n");
    while text.len() < len {
        text.push_str(block);
    }
    text
}

fn full_text_keeps_hidden_markup() {
    let text = "a **b** c\n> q\n";
    let view = view_with(text);
    place(&view, 0);
    assert!(view.is_hidden_at(2));
    assert_eq!(view.text(), text);
}

fn markup_follows_cursor() {
    let view = view_with("a **b** c");
    place(&view, 0);
    assert!(view.is_hidden_at(2) && view.is_hidden_at(5));
    place(&view, 4);
    assert!(!view.is_hidden_at(2) && !view.is_hidden_at(5));
    assert!(view.has_style_at(Style::Strong, 4));
    place(&view, 9);
    assert!(view.is_hidden_at(2) && view.is_hidden_at(6));
}

fn tag_changes_are_not_undoable() {
    let text = "a **b** c";
    let view = view_with(text);
    for offset in [4, 0, 5, 9] {
        place(&view, offset);
    }
    let buffer = view.buffer();
    assert!(!buffer.can_undo(), "moving the cursor must not create undo steps");
    buffer.insert(&mut buffer.end_iter(), "x");
    assert!(buffer.can_undo());
    buffer.undo();
    assert_eq!(view.text(), text);
}

fn undo_restores_styling() {
    let view = view_with("a b c");
    let buffer = view.buffer();
    place(&view, 2);
    buffer.insert_at_cursor("**");
    place(&view, 5);
    buffer.insert_at_cursor("**");
    place(&view, 0);
    assert_eq!(view.text(), "a **b** c");
    assert!(view.has_style_at(Style::Strong, 4) && view.is_hidden_at(2));
    buffer.undo();
    assert_eq!(view.text(), "a **b c");
    assert!(!view.has_style_at(Style::Strong, 4));
    assert!(!view.is_hidden_at(2), "unpaired markup is plain text");
}

fn multibyte_offsets() {
    // chars: П р и в е т ␠ 🙂 ␠ * * м и р * *  → `*` at 9, 10, 14, 15
    let view = view_with("Привет 🙂 **мир**");
    place(&view, 0);
    assert!(view.is_hidden_at(9) && view.is_hidden_at(15));
    assert!(!view.is_hidden_at(11));
    place(&view, 12);
    assert!(!view.is_hidden_at(9));
    assert!(view.has_style_at(Style::Strong, 12));
}

fn crlf_round_trip() {
    let text = "# T\r\n\r\n**b**\r\nmixed\nend\r\n";
    let view = view_with(text);
    place(&view, 0);
    assert_eq!(view.text(), text);
    let buffer = view.buffer();
    buffer.insert(&mut buffer.end_iter(), "x");
    assert_eq!(view.text(), format!("{text}x"));
}

fn typing_restyles() {
    let view = view_with("a ");
    place(&view, 2);
    let buffer = view.buffer();
    for ch in ["*", "*", "b", "*", "*"] {
        buffer.insert_at_cursor(ch);
    }
    assert_eq!(view.text(), "a **b**");
    assert!(view.has_style_at(Style::Strong, 4));
    assert!(!view.is_hidden_at(2), "the cursor is at the element's end");
    place(&view, 0);
    assert!(view.is_hidden_at(2) && view.is_hidden_at(6));
}

fn arrows_walk_through_hidden_markup() {
    let view = view_with("a **b** c");
    let window = common::show(&view);
    place(&view, 0);
    let mut seen = vec![0];
    for _ in 0..12 {
        view.emit_move_cursor(gtk::MovementStep::VisualPositions, 1, false);
        let c = cursor(&view);
        if c == *seen.last().unwrap() {
            break;
        }
        seen.push(c);
        if (2..=7).contains(&c) {
            assert!(!view.is_hidden_at(2), "markup must show with the cursor at {c}");
        }
    }
    println!("    right: {seen:?}");
    assert_eq!(*seen.last().unwrap(), 9, "the cursor got stuck: {seen:?}");
    let mut seen = vec![9];
    for _ in 0..12 {
        view.emit_move_cursor(gtk::MovementStep::VisualPositions, -1, false);
        let c = cursor(&view);
        if c == *seen.last().unwrap() {
            break;
        }
        seen.push(c);
    }
    println!("    left: {seen:?}");
    assert_eq!(*seen.last().unwrap(), 0, "the cursor got stuck: {seen:?}");
    window.destroy();
}

fn vertical_move_lands_on_text() {
    let view = view_with("**bold** word\nplain line here\n");
    let window = common::show(&view);
    place(&view, 18); // "plai|n" on the second line
    view.emit_move_cursor(gtk::MovementStep::DisplayLines, -1, false);
    let buffer = view.buffer();
    assert_eq!(buffer.iter_at_offset(cursor(&view)).line(), 0);
    window.destroy();
}

fn restyle_is_fast() {
    let text = prose(100_000);
    let view = view_with(&text);
    let buffer = view.buffer();
    place(&view, buffer.char_count() / 2);
    let mut times: Vec<Duration> = (0..21)
        .map(|_| {
            let started = Instant::now();
            buffer.insert_at_cursor("x");
            started.elapsed()
        })
        .collect();
    times.sort();
    let median = times[times.len() / 2];
    println!("    restyle median {median:?} on {} bytes", text.len());
    if !cfg!(debug_assertions) {
        assert!(median < Duration::from_millis(5), "too slow: {median:?}");
    }
}

fn large_paste_does_not_hang() {
    let view = view_with("start\n");
    let buffer = view.buffer();
    let text = prose(1_000_000);
    let started = Instant::now();
    buffer.insert(&mut buffer.end_iter(), &text);
    place(&view, 0);
    let took = started.elapsed();
    println!("    1 MB paste took {took:?}");
    let limit = if cfg!(debug_assertions) { 30 } else { 2 };
    assert!(took < Duration::from_secs(limit), "too slow: {took:?}");
    assert_eq!(view.text().len(), "start\n".len() + text.len());
}

fn column_is_centred() {
    let view = view_with("text");
    let window = common::show(&view);
    let expected = opal::layout::side_margin(view.width(), 70, view.char_width());
    assert_eq!(view.left_margin(), expected);
    assert_eq!(view.right_margin(), expected);
    common::settle();
    assert_eq!(view.left_margin(), expected, "margins must settle, not oscillate");
    window.destroy();
}

fn typewriter_keeps_cursor_line_centred() {
    let text: String = (0..300).map(|i| format!("строка {i}\n")).collect();
    let view = view_with(&text); // typewriter is on by default
    let window = common::show(&view);
    let buffer = view.buffer();
    for line in [150, 0, 299] {
        buffer.place_cursor(&buffer.iter_at_line(line).expect("line exists"));
        common::settle();
        let loc = view.iter_location(&buffer.iter_at_offset(cursor(&view)));
        let visible = view.visible_rect();
        let line_centre = loc.y() + loc.height() / 2;
        let view_centre = visible.y() + visible.height() / 2;
        assert!(
            (line_centre - view_centre).abs() <= loc.height(),
            "line {line}: line centre {line_centre}, view centre {view_centre}"
        );
    }
    view.set_typewriter(false);
    common::settle();
    assert_eq!(view.top_margin(), 2 * view.line_height());
    view.set_typewriter(true);
    common::settle();
    assert!(view.top_margin() > 2 * view.line_height());
    window.destroy();
}
