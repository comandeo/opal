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
        ("decorations_render", decorations_render),
        ("checkbox_toggles", checkbox_toggles),
        ("link_lookup", link_lookup),
        ("glyphs_sit_left_of_the_text", glyphs_sit_left_of_the_text),
        ("autocorrect_while_typing", autocorrect_while_typing),
        ("undo_brings_typed_text_back", undo_brings_typed_text_back),
        ("autocorrect_leaves_code_alone", autocorrect_leaves_code_alone),
        ("autocorrect_can_be_switched_off", autocorrect_can_be_switched_off),
        ("hemingway_cannot_delete", hemingway_cannot_delete),
        ("hemingway_cannot_move_the_cursor", hemingway_cannot_move_the_cursor),
        ("hemingway_types_and_corrects", hemingway_types_and_corrects),
        ("hemingway_cannot_undo_or_redo", hemingway_cannot_undo_or_redo),
        ("hemingway_drops_the_selection", hemingway_drops_the_selection),
        ("hemingway_cannot_select_all", hemingway_cannot_select_all),
        ("hemingway_cannot_overwrite", hemingway_cannot_overwrite),
        ("hemingway_mouse_does_not_reach_the_text", hemingway_mouse_does_not_reach_the_text),
        ("hemingway_has_no_context_menu", hemingway_has_no_context_menu),
        ("hemingway_opens_documents_at_the_end", hemingway_opens_documents_at_the_end),
        ("hemingway_brings_typewriter_mode", hemingway_brings_typewriter_mode),
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

fn decorations_render() {
    let text = "# Заголовок\n\n> цитата\n> вторая строка\n\n- пункт\n- [ ] задача\n- [x] готово\n\n---\n\n```\nкод\n```\n\nконец";
    let view = view_with(text);
    let window = common::show(&view);
    place(&view, view.buffer().char_count());
    common::settle();
    let path = concat!(env!("CARGO_TARGET_TMPDIR"), "/opal-decorations.png");
    common::save_png(&view, path);
    println!("    screenshot: {path}");
    assert_eq!(view.checkbox_rects().len(), 2);
    window.destroy();
}

fn checkbox_toggles() {
    let view = view_with("- [ ] задача\n\nконец");
    let window = common::show(&view);
    place(&view, view.buffer().char_count());
    common::settle();
    let boxes = view.checkbox_rects();
    assert_eq!(boxes.len(), 1);
    let (rect, byte) = boxes[0];
    // The hidden `- [ ] ` has no width: the text starts where it begins. (GTK
    // reports bogus positions for chars after a hidden run, so not 'з' itself.)
    let text_x = view.iter_location(&view.buffer().iter_at_offset(0)).x();
    assert!(rect.x() + rect.width() <= text_x, "the box is drawn left of the text");
    view.toggle_task(byte);
    assert_eq!(view.text(), "- [x] задача\n\nконец");
    view.toggle_task(byte);
    assert_eq!(view.text(), "- [ ] задача\n\nконец");
    window.destroy();
}

fn link_lookup() {
    let view = view_with("see [the site](http://example.org) and <me@x.org>");
    assert_eq!(view.link_at(6).as_deref(), Some("http://example.org"));
    assert_eq!(view.link_at(42).as_deref(), Some("me@x.org"));
    assert_eq!(view.link_at(1), None);
}

fn glyphs_sit_left_of_the_text() {
    // Hidden markup before the item (`> `), and a visible number before the box.
    let text = "> - пункт в цитате\n> - [ ] задача в цитате\n\n1. [ ] нумерованная\n\nконец";
    let view = view_with(text);
    let window = common::show(&view);
    place(&view, view.buffer().char_count());
    common::settle();
    let path = concat!(env!("CARGO_TARGET_TMPDIR"), "/opal-glyphs.png");
    common::save_png(&view, path);
    println!("    screenshot: {path}");
    let buffer = view.buffer();
    let boxes = view.checkbox_rects();
    assert_eq!(boxes.len(), 2);
    for (line, (rect, _)) in [1, 3].into_iter().zip(boxes) {
        // The first char of a line is reported correctly even when hidden.
        let text_x = view.iter_location(&buffer.iter_at_line(line).expect("line exists")).x();
        let right = rect.x() + rect.width();
        assert!(right <= text_x, "line {line}: the box ends at {right}, the text starts at {text_x}");
    }
    window.destroy();
}

/// Types like the keyboard does: one interactive insertion per key, each in a
/// user action, letting the main loop run in between.
fn type_keys(view: &OpalView, keys: &str) {
    let buffer = view.buffer();
    for ch in keys.chars() {
        buffer.begin_user_action();
        buffer.insert_interactive_at_cursor(&ch.to_string(), true);
        buffer.end_user_action();
        common::pump_for(Duration::from_millis(5));
    }
}

fn autocorrect_while_typing() {
    let view = view_with("");
    type_keys(&view, "Он сказал -- \"да\". \"Цитата \"внутри\" цитаты\"");
    assert_eq!(view.text(), "Он сказал — «да». «Цитата „внутри“ цитаты»");
}

fn undo_brings_typed_text_back() {
    let view = view_with("");
    let buffer = view.buffer();
    type_keys(&view, "a--");
    assert_eq!(view.text(), "a—");
    buffer.undo();
    common::pump_for(Duration::from_millis(20));
    assert_eq!(view.text(), "a--");
    type_keys(&view, "-");
    assert_eq!(view.text(), "a---", "a third hyphen keeps the restored --");
    type_keys(&view, " \"");
    assert_eq!(view.text(), "a--- «");
    buffer.undo();
    common::pump_for(Duration::from_millis(20));
    assert_eq!(view.text(), "a--- \"", "undo must not be corrected again");
}

fn autocorrect_leaves_code_alone() {
    let view = view_with("`ab`\n\n```\nx\n```\n");
    place(&view, 2);
    type_keys(&view, "--");
    place(&view, 13); // after `x` in the code block
    type_keys(&view, "\"");
    assert_eq!(view.text(), "`a--b`\n\n```\nx\"\n```\n");
}

fn autocorrect_can_be_switched_off() {
    let view = OpalView::new(&Config { autocorrect: false, ..Config::default() });
    view.load_text("");
    type_keys(&view, "-- \"");
    assert_eq!(view.text(), "-- \"");
}

fn hemingway_cannot_delete() {
    let view = view_with("один два три");
    place(&view, 8);
    view.set_hemingway(true);
    view.emit_backspace();
    view.emit_delete_from_cursor(gtk::DeleteType::Chars, 1);
    view.emit_delete_from_cursor(gtk::DeleteType::Chars, -1);
    view.emit_delete_from_cursor(gtk::DeleteType::WordEnds, -1);
    view.emit_delete_from_cursor(gtk::DeleteType::Paragraphs, 1);
    assert_eq!(view.text(), "один два три");
    view.set_hemingway(false);
    view.emit_backspace();
    assert_eq!(view.text(), "один дв три");
}

fn hemingway_cannot_move_the_cursor() {
    let view = view_with("один **два** три\n\nвторая строка\n\nтретья");
    let window = common::show(&view);
    place(&view, 8);
    view.set_hemingway(true);
    let steps = [
        gtk::MovementStep::LogicalPositions,
        gtk::MovementStep::VisualPositions,
        gtk::MovementStep::Words,
        gtk::MovementStep::DisplayLines,
        gtk::MovementStep::DisplayLineEnds,
        gtk::MovementStep::Paragraphs,
        gtk::MovementStep::ParagraphEnds,
        gtk::MovementStep::Pages,
        gtk::MovementStep::BufferEnds,
        gtk::MovementStep::HorizontalPages,
    ];
    for step in steps {
        for count in [1, -1] {
            for extend in [false, true] {
                view.emit_move_cursor(step, count, extend);
                assert_eq!(cursor(&view), 8, "{step:?} by {count}, extend {extend}");
                assert!(!view.buffer().has_selection(), "{step:?} by {count}, extend {extend}");
            }
        }
    }
    view.set_hemingway(false);
    view.emit_move_cursor(gtk::MovementStep::BufferEnds, 1, false);
    assert_eq!(cursor(&view), 39);
    window.destroy();
}

fn hemingway_types_and_corrects() {
    let view = view_with("начало конец");
    place(&view, 7);
    view.set_hemingway(true);
    type_keys(&view, "да -- \"нет\"\n");
    view.emit_insert_at_cursor("ещё ");
    assert_eq!(view.text(), "начало да — «нет»\nещё конец");
}

fn hemingway_cannot_undo_or_redo() {
    let view = view_with("");
    let buffer = view.buffer();
    type_keys(&view, "абв");
    view.set_hemingway(true);
    buffer.undo();
    let _ = WidgetExt::activate_action(&view, "text.undo", None);
    assert_eq!(view.text(), "абв");
    view.set_hemingway(false);
    buffer.undo();
    assert_eq!(view.text(), "", "the history outlives the mode");
    view.set_hemingway(true);
    buffer.redo();
    let _ = WidgetExt::activate_action(&view, "text.redo", None);
    assert_eq!(view.text(), "");
    view.set_hemingway(false);
    buffer.redo();
    assert_eq!(view.text(), "абв");
    // A blocked undo must leave autocorrect working (it is off during a replay).
    view.set_hemingway(true);
    buffer.undo();
    type_keys(&view, " --");
    assert_eq!(view.text(), "абв —");
}

fn hemingway_drops_the_selection() {
    let view = view_with("абвгд");
    let buffer = view.buffer();
    buffer.select_range(&buffer.iter_at_offset(1), &buffer.iter_at_offset(3));
    view.set_hemingway(true);
    assert!(!buffer.has_selection());
    assert_eq!(cursor(&view), 1);
    type_keys(&view, "ж");
    assert_eq!(view.text(), "ажбвгд", "typing must not replace what was selected");
}

fn hemingway_cannot_select_all() {
    let view = view_with("абвгд");
    let buffer = view.buffer();
    place(&view, 2);
    view.set_hemingway(true);
    view.emit_select_all(true);
    assert!(!buffer.has_selection());
    view.set_hemingway(false);
    view.emit_select_all(true);
    assert!(buffer.has_selection());
}

fn hemingway_cannot_overwrite() {
    let view = view_with("абв");
    view.set_overwrite(true);
    view.set_hemingway(true);
    assert!(!view.overwrites(), "overwrite mode ends with the mode's start");
    view.emit_toggle_overwrite();
    assert!(!view.overwrites(), "the Insert key must not bring it back");
}

/// What a pointer at the middle of the view lands on.
fn picked_at_middle(view: &OpalView) -> gtk::Widget {
    let scroller = view.parent().expect("the view is in a scrolled window");
    let (x, y) = (scroller.width() as f64 / 2.0, scroller.height() as f64 / 2.0);
    scroller.pick(x, y, gtk::PickFlags::DEFAULT).expect("the scrolled window itself at least")
}

fn hemingway_mouse_does_not_reach_the_text() {
    let view = view_with("текст");
    let window = common::show(&view);
    let on_view = |view: &OpalView| {
        let picked = picked_at_middle(view);
        picked == *view.upcast_ref::<gtk::Widget>() || picked.is_ancestor(view)
    };
    assert!(on_view(&view));
    view.set_hemingway(true);
    assert!(!on_view(&view), "clicks must not move the cursor");
    view.set_hemingway(false);
    assert!(on_view(&view));
    window.destroy();
}

/// The view's context menu, if it is open.
fn open_menu(view: &OpalView) -> Option<gtk::PopoverMenu> {
    let mut child = view.first_child();
    while let Some(widget) = child {
        if widget.is_visible() {
            if let Ok(menu) = widget.clone().downcast::<gtk::PopoverMenu>() {
                return Some(menu);
            }
        }
        child = widget.next_sibling();
    }
    None
}

fn hemingway_has_no_context_menu() {
    let view = view_with("текст");
    let window = common::show(&view);
    view.set_hemingway(true);
    let _ = WidgetExt::activate_action(&view, "menu.popup", None);
    assert!(open_menu(&view).is_none(), "its Select All and Delete would edit the text");
    view.set_hemingway(false);
    WidgetExt::activate_action(&view, "menu.popup", None).unwrap();
    open_menu(&view).expect("the menu is back").popdown();
    window.destroy();
}

fn hemingway_opens_documents_at_the_end() {
    let config = Config { typewriter: false, ..Config::default() };
    let view = OpalView::new(&config);
    let window = common::show(&view);
    view.set_hemingway(true);
    view.load_text("a **b**\n\nконец");
    assert_eq!(cursor(&view), 14);
    assert!(view.is_hidden_at(2), "the markup away from the cursor stays hidden");
    view.load_text(&prose(20_000));
    common::settle();
    let scroller = view.parent().and_downcast::<gtk::ScrolledWindow>().expect("a scrolled window");
    assert!(scroller.vadjustment().value() > 0.0, "the end of the document is on screen");
    view.set_hemingway(false);
    view.load_text("a **b**");
    assert_eq!(cursor(&view), 0);
    window.destroy();
}

fn hemingway_brings_typewriter_mode() {
    let text: String = (0..300).map(|i| format!("строка {i}\n")).collect();
    let view = OpalView::new(&Config { typewriter: false, ..Config::default() });
    view.load_text(&text);
    let window = common::show(&view);
    let buffer = view.buffer();
    buffer.place_cursor(&buffer.iter_at_line(150).expect("line exists"));
    common::settle();
    view.set_hemingway(true);
    common::settle();
    let loc = view.iter_location(&buffer.iter_at_offset(cursor(&view)));
    let visible = view.visible_rect();
    let (line_centre, view_centre) = (loc.y() + loc.height() / 2, visible.y() + visible.height() / 2);
    assert!(
        (line_centre - view_centre).abs() <= loc.height(),
        "line centre {line_centre}, view centre {view_centre}"
    );
    view.set_hemingway(false);
    assert!(!view.typewriter(), "it was off before the mode");
    view.set_typewriter(true);
    view.set_hemingway(true);
    view.set_hemingway(false);
    assert!(view.typewriter(), "it was on before the mode");
    window.destroy();
}
