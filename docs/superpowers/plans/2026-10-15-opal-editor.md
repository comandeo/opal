# Opal Editor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `opal`, a minimalist native Markdown editor (Typora-style hybrid rendering, centred column, typewriter mode, autosave) in Rust + GTK4.

**Architecture:** All logic lives in GTK-free modules covered by `cargo test --lib`: `markdown` (pulldown-cmark events → elements with byte ranges of their markup), `reveal` (which markup is visible for a cursor), `restyle` (`Styler`: text/cursor change → minimal tag operations), `layout`, `config`, `document`. A thin GTK layer (`editor/` — a `GtkTextView` subclass; `window.rs`) executes tag operations, draws decorations and handles files. GTK behaviour is pinned by integration tests that open real windows (`tests/gtk_*.rs`, `harness = false`).

**Tech Stack:** Rust 2024, `gtk4` 0.11 (feature `v4_10`), `pulldown-cmark` 0.13 (no default features), `tempfile` 3 (dev only).

**Spec:** `docs/superpowers/specs/2026-10-14-opal-editor-design.md` — section 2 is the agreed behaviour, section 3 the technical design. Read both before starting.

## Global Constraints

- Runtime dependencies: only `gtk4` (renamed to `gtk` in Cargo.toml, version `0.11`, features `["v4_10"]`) and `pulldown-cmark` (`0.13`, `default-features = false`). Dev: `tempfile = "3"`. Nothing else.
- Minimum GTK 4.10. No Linux-only APIs in `src/` (no `/proc`, inotify, `libc`); tests may use unix-only std (`std::os::unix`).
- The file on disk gets exactly the buffer text *including hidden markup* (`buffer.text(.., .., true)`); the editor never rewrites markup.
- User-visible strings are Russian (window title, dialogs, config warnings). Window title: `имя.md — opal`, untitled: `Без имени — opal`, prefix `• ` when unsaved and (untitled, save failed or autosave paused).
- Config file `$XDG_CONFIG_HOME/opal/config` or `~/.config/opal/config`; keys `column_width` (default 70, allowed 20..=400), `font` (`Serif 14`), `mono_font` (`Monospace 12`), `typewriter` (`true`), `theme` (`light`|`dark`).
- Autosave 1 s after the last change; also on close, Ctrl+O, Ctrl+N. Keys: Ctrl+S, Ctrl+O, Ctrl+N, Ctrl+T (typewriter), Ctrl+Q (close).
- Performance targets: restyle ≤ 5 ms per keystroke on a 100 KB document (release build); startup to first frame ≲ 200 ms; RSS ≲ 60 MB.
- Work happens on branch `v1` (created in Task 1). Every commit message ends with the trailer `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` (as a separate `-m` paragraph, as shown in the commit steps).
- `cargo test --lib` runs pure tests only. `cargo test --test gtk_editor` / `--test gtk_window` open windows and need a display (`WAYLAND_DISPLAY`/`DISPLAY` — present on the dev machine, sway).
- gtk4-rs API names in this plan were written against gtk4-rs 0.11 without a compiler at hand; if one does not compile, use the equivalent from the 0.11 docs (`cargo doc -p gtk4 --open` or docs.rs/gtk4/0.11) and keep behaviour identical. The GTK-free code was compiled and tested as written.

## Review Focus

1. Text with 4-byte characters (emoji) right next to markup — markers must hide/show at the right chars (byte↔char mapping). Test: `multibyte_offsets` in Task 4.
2. CRLF and mixed line endings — the editor must return the text byte-for-byte and keep `\r\n` after edits. Test: `crlf_round_trip` in Task 4.
3. Undo of an edit that created markup — styling must match the text after undo (no stale bold, no hidden unpaired `**`). Test: `undo_restores_styling` in Task 4.
4. Saving into a location that cannot be written — an error, the original untouched, no temp file left. Test: `unwritable_directory_fails_cleanly` in Task 7.
5. Closing the window less than a second after typing — the last edit must reach the disk. Test: `close_flushes_pending_edit` in Task 8.

---

### Task 1: Project scaffold and config

**Files:**
- Create: `Cargo.toml`, `.gitignore`, `src/lib.rs`, `src/config.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `opal::config::{Config, Theme, parse, path_from, load}`:
  - `pub struct Config { pub column_width: u32, pub font: String, pub mono_font: String, pub typewriter: bool, pub theme: Theme }` (+ `Default`, `Clone`, `PartialEq`, `Debug`)
  - `pub enum Theme { Light, Dark }` (`Copy`, `PartialEq`)
  - `pub fn parse(text: &str) -> (Config, Vec<String>)`, `pub fn path_from(xdg_config_home: Option<&str>, home: Option<&str>) -> Option<PathBuf>`, `pub fn load() -> Config`

- [ ] **Step 1: Create the branch and the crate files**

```bash
cd /home/mitja/dev/opal && git checkout -b v1
```

`Cargo.toml`:

```toml
[package]
name = "opal"
version = "0.1.0"
edition = "2024"
description = "Минималистичный Markdown-редактор"

[dependencies]
pulldown-cmark = { version = "0.13", default-features = false }

[dev-dependencies]
tempfile = "3"

[profile.release]
lto = true
codegen-units = 1
strip = true
```

`.gitignore`:

```
/target
```

`src/lib.rs`:

```rust
//! opal — a minimalist Markdown editor.

pub mod config;
```

- [ ] **Step 2: Write the failing tests** — `src/config.rs` with stubs and tests:

```rust
//! `~/.config/opal/config`: `key = value` lines, `#` starts a comment.

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Column width in characters.
    pub column_width: u32,
    /// Pango font description, e.g. "Serif 14".
    pub font: String,
    pub mono_font: String,
    /// Typewriter mode at startup.
    pub typewriter: bool,
    pub theme: Theme,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            column_width: 70,
            font: "Serif 14".to_string(),
            mono_font: "Monospace 12".to_string(),
            typewriter: true,
            theme: Theme::Light,
        }
    }
}

pub fn parse(_text: &str) -> (Config, Vec<String>) {
    (Config::default(), Vec::new())
}

pub fn path_from(_xdg_config_home: Option<&str>, _home: Option<&str>) -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_gives_defaults() {
        assert_eq!(parse(""), (Config::default(), vec![]));
    }

    #[test]
    fn all_keys() {
        let text = "# comment\ncolumn_width = 80\nfont = Literata 15  # trailing comment\nmono_font=JetBrains Mono 11\ntypewriter = false\ntheme = dark\n";
        let (cfg, warnings) = parse(text);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            cfg,
            Config {
                column_width: 80,
                font: "Literata 15".into(),
                mono_font: "JetBrains Mono 11".into(),
                typewriter: false,
                theme: Theme::Dark,
            }
        );
    }

    #[test]
    fn bad_lines_warn_and_keep_defaults() {
        let (cfg, warnings) = parse("column_width = wide\ncolumn_width = 5\ntheme = blue\nfont =\nnonsense\ncolour = red\n");
        assert_eq!(cfg, Config::default());
        assert_eq!(warnings.len(), 6, "{warnings:?}");
        assert!(warnings[0].starts_with("строка 1:"));
        assert!(warnings[5].contains("colour"));
    }

    #[test]
    fn path_prefers_xdg() {
        assert_eq!(path_from(Some("/x"), Some("/h")), Some(PathBuf::from("/x/opal/config")));
        assert_eq!(path_from(None, Some("/h")), Some(PathBuf::from("/h/.config/opal/config")));
        assert_eq!(path_from(Some(""), Some("/h")), Some(PathBuf::from("/h/.config/opal/config")));
        assert_eq!(path_from(None, None), None);
    }
}
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `cargo test --lib config`
Expected: FAIL — `all_keys`, `bad_lines_warn_and_keep_defaults`, `path_prefers_xdg` fail; `empty_text_gives_defaults` passes.

- [ ] **Step 4: Implement** — replace everything above the `#[cfg(test)]` line of `src/config.rs` with:

```rust
//! `~/.config/opal/config`: `key = value` lines, `#` starts a comment.

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Column width in characters.
    pub column_width: u32,
    /// Pango font description, e.g. "Serif 14".
    pub font: String,
    pub mono_font: String,
    /// Typewriter mode at startup.
    pub typewriter: bool,
    pub theme: Theme,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            column_width: 70,
            font: "Serif 14".to_string(),
            mono_font: "Monospace 12".to_string(),
            typewriter: true,
            theme: Theme::Light,
        }
    }
}

/// Parses config text; returns the config and a warning per ignored line.
pub fn parse(text: &str) -> (Config, Vec<String>) {
    let mut cfg = Config::default();
    let mut warnings = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            warnings.push(format!("строка {n}: ожидается `ключ = значение`"));
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        let ok = match key {
            "column_width" => match value.parse::<u32>() {
                Ok(v) if (20..=400).contains(&v) => {
                    cfg.column_width = v;
                    true
                }
                _ => false,
            },
            "font" | "mono_font" if !value.is_empty() => {
                let slot = if key == "font" { &mut cfg.font } else { &mut cfg.mono_font };
                *slot = value.to_string();
                true
            }
            "typewriter" => match value {
                "true" => {
                    cfg.typewriter = true;
                    true
                }
                "false" => {
                    cfg.typewriter = false;
                    true
                }
                _ => false,
            },
            "theme" => match value {
                "light" => {
                    cfg.theme = Theme::Light;
                    true
                }
                "dark" => {
                    cfg.theme = Theme::Dark;
                    true
                }
                _ => false,
            },
            "font" | "mono_font" => false,
            _ => {
                warnings.push(format!("строка {n}: неизвестный ключ `{key}`"));
                continue;
            }
        };
        if !ok {
            warnings.push(format!("строка {n}: недопустимое значение `{value}` для `{key}`"));
        }
    }
    (cfg, warnings)
}

/// `$XDG_CONFIG_HOME/opal/config`, falling back to `$HOME/.config/opal/config`.
pub fn path_from(xdg_config_home: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
    let base = match xdg_config_home.filter(|v| !v.is_empty()) {
        Some(xdg) => PathBuf::from(xdg),
        None => PathBuf::from(home.filter(|v| !v.is_empty())?).join(".config"),
    };
    Some(base.join("opal").join("config"))
}

/// Reads the config file; problems are reported to stderr and never fatal.
pub fn load() -> Config {
    let xdg = std::env::var("XDG_CONFIG_HOME").ok();
    let home = std::env::var("HOME").ok();
    let Some(path) = path_from(xdg.as_deref(), home.as_deref()) else {
        return Config::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let (cfg, warnings) = parse(&text);
            for w in warnings {
                eprintln!("opal: {}: {w}", path.display());
            }
            cfg
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
        Err(e) => {
            eprintln!("opal: не удалось прочитать {}: {e}", path.display());
            Config::default()
        }
    }
}
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test --lib config`
Expected: PASS (4 tests), no warnings.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock .gitignore src/lib.rs src/config.rs
git commit -m "Add crate scaffold and config parsing" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Markdown elements

**Files:**
- Create: `src/markdown.rs`
- Modify: `src/lib.rs` (add `pub mod markdown;`)

**Interfaces:**
- Consumes: nothing.
- Produces: `opal::markdown::{Kind, Marker, Element, parse}`:
  - `pub enum Kind { Strong, Emphasis, Strike, Code, Link { url: String }, Heading(u8), Quote, ListItem { ordered: bool }, Task { checked: bool }, CodeBlock, Rule }`
  - `pub struct Marker { pub range: Range<usize>, pub scope: Range<usize> }` — byte range of markup chars and where the cursor must be for them to show.
  - `pub struct Element { pub kind: Kind, pub range: Range<usize>, pub markers: Vec<Marker> }` — all derive `Debug, Clone, PartialEq, Eq, Hash`.
  - `pub fn parse(text: &str) -> Vec<Element>` — ordered by start offset, outer before nested. Byte offsets.

Rules (spec 3.3) are encoded in the tests; the notable ones: setext underline and code fences are hidden *with* their line break (so the line collapses); a rule keeps its line (the line is drawn there); ordered list items have no markers; a task item has one marker covering `- [ ] `; quote markers are per line and per nesting level; unclosed markup is plain text.

- [ ] **Step 1: Write the failing tests** — `src/markdown.rs`:

```rust
//! Markdown text → styled elements with byte ranges of their markup.

use std::collections::HashMap;
use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, LinkType, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Kind {
    Strong,
    Emphasis,
    Strike,
    Code,
    Link { url: String },
    Heading(u8),
    Quote,
    ListItem { ordered: bool },
    Task { checked: bool },
    CodeBlock,
    Rule,
}

/// Markup characters (`range`) and where the cursor must be (`scope`)
/// for them to be shown. Byte offsets into the parsed text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Marker {
    pub range: Range<usize>,
    pub scope: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Element {
    pub kind: Kind,
    /// Bytes the element's style applies to.
    pub range: Range<usize>,
    pub markers: Vec<Marker>,
}


pub fn parse(_text: &str) -> Vec<Element> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (kind, styled text, [marker texts]) for compact assertions.
    fn view(text: &str) -> Vec<(Kind, &str, Vec<&str>)> {
        parse(text)
            .into_iter()
            .map(|e| {
                let markers = e.markers.iter().map(|m| &text[m.range.clone()]).collect();
                (e.kind, &text[e.range], markers)
            })
            .collect()
    }

    fn scopes(text: &str) -> Vec<Vec<&str>> {
        parse(text)
            .into_iter()
            .map(|e| e.markers.iter().map(|m| &text[m.scope.clone()]).collect())
            .collect()
    }

    #[test]
    fn strong_emphasis_strike() {
        assert_eq!(
            view("a **b** _c_ ~~d~~ ~e~"),
            vec![
                (Kind::Strong, "**b**", vec!["**", "**"]),
                (Kind::Emphasis, "_c_", vec!["_", "_"]),
                (Kind::Strike, "~~d~~", vec!["~~", "~~"]),
                (Kind::Strike, "~e~", vec!["~", "~"]),
            ]
        );
    }

    #[test]
    fn strong_inside_emphasis() {
        assert_eq!(
            view("***x***"),
            vec![
                (Kind::Emphasis, "***x***", vec!["*", "*"]),
                (Kind::Strong, "**x**", vec!["**", "**"]),
            ]
        );
    }

    #[test]
    fn inline_scope_is_the_element() {
        assert_eq!(scopes("a **b** c"), vec![vec!["**b**", "**b**"]]);
    }

    #[test]
    fn code_span_with_double_backticks() {
        assert_eq!(
            view("``a`b`` `c`"),
            vec![(Kind::Code, "``a`b``", vec!["``", "``"]), (Kind::Code, "`c`", vec!["`", "`"])]
        );
    }

    #[test]
    fn links() {
        let url = |u: &str| Kind::Link { url: u.to_string() };
        assert_eq!(
            view("[li **nk**](http://u \"t\") <https://a.b> [r][x] <me@x.org>\n\n[x]: http://x"),
            vec![
                (url("http://u"), "[li **nk**](http://u \"t\")", vec!["[", "](http://u \"t\")"]),
                (Kind::Strong, "**nk**", vec!["**", "**"]),
                (url("https://a.b"), "<https://a.b>", vec!["<", ">"]),
                (url("http://x"), "[r][x]", vec!["[", "][x]"]),
                (url("me@x.org"), "<me@x.org>", vec!["<", ">"]),
            ]
        );
    }

    #[test]
    fn empty_link_text() {
        assert_eq!(
            view("[](http://e)"),
            vec![(Kind::Link { url: "http://e".into() }, "[](http://e)", vec!["[", "](http://e)"])]
        );
    }

    #[test]
    fn unclosed_markup_is_plain_text() {
        assert_eq!(view("**unclosed and *open"), vec![]);
    }

    #[test]
    fn atx_headings() {
        assert_eq!(
            view("## Заголовок ##\n# Title\n#\n"),
            vec![
                (Kind::Heading(2), "## Заголовок ##\n", vec!["## ", " ##"]),
                (Kind::Heading(1), "# Title\n", vec!["# "]),
                (Kind::Heading(1), "#\n", vec!["#"]),
            ]
        );
        assert_eq!(scopes("# Title\n"), vec![vec!["# Title"]]);
    }

    #[test]
    fn setext_headings_hide_the_underline_line() {
        assert_eq!(view("Setext\n===\nnext"), vec![(Kind::Heading(1), "Setext\n===\n", vec!["===\n"])]);
        assert_eq!(view("Title\n---"), vec![(Kind::Heading(2), "Title\n---", vec!["\n---"])]);
    }

    #[test]
    fn quotes_have_one_marker_per_line_and_level() {
        let text = "> q1\n> q2\nlazy\n>\n> > nested\n";
        assert_eq!(
            view(text),
            vec![(Kind::Quote, text, vec!["> ", "> ", ">", "> "]), (Kind::Quote, "> nested\n", vec!["> "])]
        );
        assert_eq!(scopes(text), vec![vec!["> q1", "> q2", ">", "> > nested"], vec!["> > nested"]]);
    }

    #[test]
    fn quote_inside_list_item() {
        let text = "- > quote\n  > more\n";
        assert_eq!(
            view(text),
            vec![
                (Kind::ListItem { ordered: false }, text, vec!["- "]),
                (Kind::Quote, "> quote\n  > more\n", vec!["> ", "> "]),
            ]
        );
    }

    #[test]
    fn list_items() {
        assert_eq!(
            view("- a\n  - nested\n* star\n\n1. num\n- \n"),
            vec![
                (Kind::ListItem { ordered: false }, "- a\n  - nested\n", vec!["- "]),
                (Kind::ListItem { ordered: false }, "- nested\n", vec!["- "]),
                (Kind::ListItem { ordered: false }, "* star\n\n", vec!["* "]),
                (Kind::ListItem { ordered: true }, "1. num\n", vec![]),
                (Kind::ListItem { ordered: false }, "- \n", vec!["- "]),
            ]
        );
        assert_eq!(scopes("- a\n  - b\n")[0], vec!["- a"]);
    }

    #[test]
    fn tasks_hide_bullet_and_box_together() {
        assert_eq!(
            view("- [ ] todo\n- [x] done\n\n1. [ ] num\n"),
            vec![
                (Kind::Task { checked: false }, "- [ ] todo\n", vec!["- [ ] "]),
                (Kind::Task { checked: true }, "- [x] done\n\n", vec!["- [x] "]),
                (Kind::Task { checked: false }, "1. [ ] num\n", vec!["[ ] "]),
            ]
        );
    }

    #[test]
    fn fenced_code_blocks() {
        assert_eq!(
            view("```rust\ncode\n```\nafter"),
            vec![(Kind::CodeBlock, "```rust\ncode\n```", vec!["```rust\n", "```\n"])]
        );
        assert_eq!(view("```\ncode\n```"), vec![(Kind::CodeBlock, "```\ncode\n```", vec!["```\n", "\n```"])]);
        assert_eq!(view("```\nunclosed\n"), vec![(Kind::CodeBlock, "```\nunclosed\n", vec!["```\n"])]);
        assert_eq!(view("```\n```\n"), vec![(Kind::CodeBlock, "```\n```", vec!["```\n", "```\n"])]);
        assert_eq!(view("```"), vec![(Kind::CodeBlock, "```", vec!["```"])]);
        let text = "```\ncode\n```\n";
        assert_eq!(scopes(text), vec![vec!["```\ncode\n```", "```\ncode\n```"]]);
    }

    #[test]
    fn indented_code_block_has_no_markers() {
        assert_eq!(view("    code\n"), vec![(Kind::CodeBlock, "code\n", vec![])]);
    }

    #[test]
    fn rule_keeps_its_line() {
        assert_eq!(view("para\n\n---\nnext"), vec![(Kind::Rule, "---", vec!["---"])]);
    }

    #[test]
    fn crlf_line_breaks() {
        assert_eq!(
            view("# T\r\n\r\n```\r\nc\r\n```\r\n"),
            vec![
                (Kind::Heading(1), "# T\r\n", vec!["# "]),
                (Kind::CodeBlock, "```\r\nc\r\n```", vec!["```\r\n", "```\r\n"]),
            ]
        );
        assert_eq!(scopes("> a\r\n> b\r\n")[0], vec!["> a", "> b"]);
    }

    #[test]
    fn cyrillic_offsets_are_bytes() {
        let text = "Привет **мир**";
        let els = parse(text);
        assert_eq!(els[0].range, 13..23);
        assert_eq!(els[0].markers[0].range, 13..15);
        assert_eq!(&text[els[0].markers[1].range.clone()], "**");
    }
}
```

Add `pub mod markdown;` to `src/lib.rs` (keep modules in alphabetical order).

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test --lib markdown`
Expected: FAIL — every test except `unclosed_markup_is_plain_text`.

- [ ] **Step 3: Implement** — replace everything above the `#[cfg(test)]` line of `src/markdown.rs` with:

```rust
//! Markdown text → styled elements with byte ranges of their markup.

use std::collections::HashMap;
use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, LinkType, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Kind {
    Strong,
    Emphasis,
    Strike,
    Code,
    Link { url: String },
    Heading(u8),
    Quote,
    ListItem { ordered: bool },
    Task { checked: bool },
    CodeBlock,
    Rule,
}

/// Markup characters (`range`) and where the cursor must be (`scope`)
/// for them to be shown. Byte offsets into the parsed text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Marker {
    pub range: Range<usize>,
    pub scope: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Element {
    pub kind: Kind,
    /// Bytes the element's style applies to.
    pub range: Range<usize>,
    pub markers: Vec<Marker>,
}

/// An element whose Start event was seen but not its End yet.
struct Open {
    idx: usize,
    autolink: bool,
    fenced: bool,
    first_child: Option<usize>,
    last_child_end: Option<usize>,
}

/// Parses `text` and returns its elements ordered by start offset
/// (outer elements before the elements nested in them).
pub fn parse(text: &str) -> Vec<Element> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let mut out: Vec<Element> = Vec::new();
    let mut open: Vec<Open> = Vec::new();
    let mut lists: Vec<bool> = Vec::new();
    // Line start → byte after the `>` markers already claimed by outer quotes.
    let mut quote_consumed: HashMap<usize, usize> = HashMap::new();

    for (event, r) in Parser::new_ext(text, options).into_offset_iter() {
        let closes_open = matches!(&event, Event::End(end) if is_tracked(end));
        if !closes_open {
            if let Some(top) = open.last_mut() {
                top.first_child.get_or_insert(r.start);
                top.last_child_end = Some(top.last_child_end.map_or(r.end, |e| e.max(r.end)));
            }
        }

        match event {
            Event::Start(Tag::List(start)) => lists.push(start.is_some()),
            Event::End(TagEnd::List(_)) => {
                lists.pop();
            }
            Event::Start(tag) => {
                let (mut autolink, mut fenced) = (false, false);
                let kind = match tag {
                    Tag::Strong => Kind::Strong,
                    Tag::Emphasis => Kind::Emphasis,
                    Tag::Strikethrough => Kind::Strike,
                    Tag::Link { link_type, dest_url, .. } => {
                        autolink = matches!(link_type, LinkType::Autolink | LinkType::Email);
                        Kind::Link { url: dest_url.to_string() }
                    }
                    Tag::Heading { level, .. } => Kind::Heading(level as u8),
                    Tag::BlockQuote(_) => Kind::Quote,
                    Tag::Item => Kind::ListItem { ordered: lists.last().copied().unwrap_or(false) },
                    Tag::CodeBlock(kind) => {
                        fenced = matches!(kind, CodeBlockKind::Fenced(_));
                        Kind::CodeBlock
                    }
                    _ => continue,
                };
                let markers = match kind {
                    Kind::Quote => quote_markers(text, &r, &mut quote_consumed),
                    Kind::ListItem { ordered: false } => vec![bullet_marker(text, &r)],
                    _ => Vec::new(),
                };
                open.push(Open {
                    idx: out.len(),
                    autolink,
                    fenced,
                    first_child: None,
                    last_child_end: None,
                });
                out.push(Element { kind, range: r, markers });
            }
            Event::End(end) if is_tracked(&end) => {
                let o = open.pop().expect("pulldown-cmark keeps events nested");
                let el = &mut out[o.idx];
                let r = el.range.clone();
                match el.kind {
                    Kind::Strong => el.markers = delims(&r, 2),
                    Kind::Emphasis => el.markers = delims(&r, 1),
                    Kind::Strike => el.markers = delims(&r, run_len(text, r.start, b'~').min(2)),
                    Kind::Link { .. } if o.autolink => el.markers = delims(&r, 1),
                    Kind::Link { .. } => {
                        let tail = o.last_child_end.unwrap_or(r.start + 1);
                        el.markers = vec![
                            Marker { range: r.start..r.start + 1, scope: r.clone() },
                            Marker { range: tail..r.end, scope: r.clone() },
                        ];
                    }
                    Kind::Heading(_) => el.markers = heading_markers(text, &r, &o),
                    Kind::CodeBlock if o.fenced => el.markers = fence_markers(text, &r, &o),
                    _ => {}
                }
            }
            Event::Code(_) => {
                let n = run_len(text, r.start, b'`');
                out.push(Element { kind: Kind::Code, markers: delims(&r, n), range: r });
            }
            Event::Rule => {
                let line = r.start..trim_break(text, r.end);
                out.push(Element {
                    kind: Kind::Rule,
                    markers: vec![Marker { range: line.clone(), scope: line.clone() }],
                    range: line,
                });
            }
            Event::TaskListMarker(checked) => {
                let Some(top) = open.last() else { continue };
                let el = &mut out[top.idx];
                let Kind::ListItem { ordered } = el.kind else { continue };
                let end = if text.as_bytes().get(r.end) == Some(&b' ') { r.end + 1 } else { r.end };
                let start = if ordered { r.start } else { el.range.start };
                let scope = el.range.start..line_end(text, el.range.start);
                el.kind = Kind::Task { checked };
                el.markers = vec![Marker { range: start..end, scope }];
            }
            _ => {}
        }
    }
    out
}

fn is_tracked(end: &TagEnd) -> bool {
    matches!(
        end,
        TagEnd::Strong
            | TagEnd::Emphasis
            | TagEnd::Strikethrough
            | TagEnd::Link
            | TagEnd::Heading(_)
            | TagEnd::BlockQuote(_)
            | TagEnd::Item
            | TagEnd::CodeBlock
    )
}

/// Opening and closing delimiters of `n` bytes each.
fn delims(r: &Range<usize>, n: usize) -> Vec<Marker> {
    vec![
        Marker { range: r.start..r.start + n, scope: r.clone() },
        Marker { range: r.end - n..r.end, scope: r.clone() },
    ]
}

fn run_len(text: &str, at: usize, ch: u8) -> usize {
    text.as_bytes()[at..].iter().take_while(|&&b| b == ch).count()
}

fn is_blank(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

/// Start of the line containing `pos`.
fn line_start(text: &str, pos: usize) -> usize {
    text[..pos].rfind('\n').map_or(0, |i| i + 1)
}

/// End of the line content starting at or containing `pos`, before `\n` / `\r\n`.
fn line_end(text: &str, pos: usize) -> usize {
    let end = text[pos..].find('\n').map_or(text.len(), |i| pos + i);
    if end > pos && text.as_bytes()[end - 1] == b'\r' { end - 1 } else { end }
}

/// `end` moved back over one trailing line break, if there is one.
fn trim_break(text: &str, end: usize) -> usize {
    if text[..end].ends_with("\r\n") {
        end - 2
    } else if text[..end].ends_with('\n') {
        end - 1
    } else {
        end
    }
}

/// A whole-line marker extended by its line break (the following one, or the
/// preceding one on the last line) so that hiding it collapses the line.
fn with_break(text: &str, r: Range<usize>) -> Range<usize> {
    if text[r.end..].starts_with("\r\n") {
        r.start..r.end + 2
    } else if text[r.end..].starts_with('\n') {
        r.start..r.end + 1
    } else if text[..r.start].ends_with("\r\n") {
        r.start - 2..r.end
    } else if text[..r.start].ends_with('\n') {
        r.start - 1..r.end
    } else {
        r
    }
}

fn bullet_marker(text: &str, r: &Range<usize>) -> Marker {
    let le = line_end(text, r.start);
    let after = r.start + 1;
    let spaces = text.as_bytes()[after..le].iter().take_while(|&&b| is_blank(b)).count();
    Marker { range: r.start..after + spaces, scope: r.start..le }
}

fn quote_markers(text: &str, r: &Range<usize>, consumed: &mut HashMap<usize, usize>) -> Vec<Marker> {
    let bytes = text.as_bytes();
    let mut markers = Vec::new();
    let mut ls = line_start(text, r.start);
    let mut from = r.start;
    while ls < r.end {
        let le = line_end(text, ls);
        let from_here = from.max(consumed.get(&ls).copied().unwrap_or(ls));
        let p = from_here + bytes[from_here..le].iter().take_while(|&&b| is_blank(b)).count();
        if p < le && bytes[p] == b'>' {
            let end = if p + 1 < le && bytes[p + 1] == b' ' { p + 2 } else { p + 1 };
            markers.push(Marker { range: p..end, scope: ls..le });
            consumed.insert(ls, end);
        }
        ls = text[le..].find('\n').map_or(text.len(), |i| le + i + 1);
        from = ls;
    }
    markers
}

fn heading_markers(text: &str, r: &Range<usize>, o: &Open) -> Vec<Marker> {
    let content_end = trim_break(text, r.end);
    let scope = r.start..content_end;
    let mut ranges = Vec::new();
    if text.as_bytes()[r.start] == b'#' {
        match (o.first_child, o.last_child_end) {
            (Some(first), Some(last)) => {
                ranges.push(r.start..first);
                if last < content_end {
                    ranges.push(last..content_end);
                }
            }
            _ => ranges.push(r.start..content_end),
        }
    } else {
        let underline = line_start(text, content_end);
        ranges.push(with_break(text, underline..content_end));
    }
    ranges.into_iter().map(|range| Marker { range, scope: scope.clone() }).collect()
}

fn fence_markers(text: &str, r: &Range<usize>, o: &Open) -> Vec<Marker> {
    let open_end = match o.first_child {
        Some(first) => first,
        None => with_break(text, r.start..line_end(text, r.start)).end.min(r.end),
    };
    let mut markers = vec![Marker { range: r.start..open_end, scope: r.clone() }];
    let close_start = o.last_child_end.unwrap_or(open_end).max(open_end);
    if close_start < r.end {
        markers.push(Marker { range: with_break(text, close_start..r.end), scope: r.clone() });
    }
    markers
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --lib markdown`
Expected: PASS (18 tests), no warnings.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/markdown.rs
git commit -m "Parse Markdown into elements with markup ranges" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Reveal rules and the Styler

**Files:**
- Create: `src/reveal.rs`, `src/restyle.rs`
- Modify: `src/lib.rs` (add `pub mod restyle;`, `pub mod reveal;`)

**Interfaces:**
- Consumes: `markdown::{parse, Element, Kind}`.
- Produces:
  - `opal::reveal::visible_markers(elements: &[Element], cursor: Range<usize>) -> Vec<Range<usize>>` — markers whose scope touches the cursor/selection (inclusive; `cursor` may be reversed).
  - `opal::restyle::Style` — `Strong, Emphasis, Strike, Code, Link, Heading(u8), Quote, ListItem, CodeBlock, Markup, Hidden`; `Style::ALL: [Style; 16]` (in tag priority order: `Markup` and `Hidden` last).
  - `opal::restyle::TagOps { pub clear: Option<Range<usize>>, pub remove: Vec<(Style, Range<usize>)>, pub add: Vec<(Style, Range<usize>)> }` — byte offsets of the current text; apply `clear`, then `remove`, then `add`.
  - `opal::restyle::Styler` — `new()`, `text() -> &str`, `elements() -> &[Element]`, `is_visible(&Range<usize>) -> bool`, `reset(&mut self, text: &str, cursor: Range<usize>) -> TagOps`, `text_changed(&mut self, text: &str, cursor: Range<usize>) -> TagOps`, `cursor_moved(&mut self, cursor: Range<usize>) -> TagOps`.
  - `opal::restyle::{element_style, common_affixes}`.

How `text_changed` stays cheap (spec 3.4): the common prefix/suffix of old and new text locate the edit; elements are in document order, so the ones unchanged by the edit (after shifting) form a common head and tail of the old and new lists; only the span covering the edit and the elements between head and tail is cleared and re-tagged. Markers outside that span keep their tags; only their `Hidden` state is toggled where visibility changed. Measured: ~2 ms per edit on a 100 KB document in release, almost all of it `parse`.

- [ ] **Step 1: Write the failing tests** — `src/reveal.rs`:

```rust
//! Which markup is shown for a given cursor or selection.

use std::ops::Range;

use crate::markdown::Element;

pub fn visible_markers(_elements: &[Element], _cursor: Range<usize>) -> Vec<Range<usize>> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::parse;

    fn shown(text: &str, cursor: Range<usize>) -> Vec<&str> {
        visible_markers(&parse(text), cursor).into_iter().map(|r| &text[r]).collect()
    }

    #[test]
    fn cursor_outside_hides_everything() {
        assert!(shown("a **b** c", 0..0).is_empty());
        assert!(shown("a **b** c", 8..8).is_empty());
    }

    #[test]
    fn cursor_inside_or_on_boundary_reveals() {
        assert_eq!(shown("a **b** c", 4..4), vec!["**", "**"]);
        assert_eq!(shown("a **b** c", 2..2), vec!["**", "**"]);
        assert_eq!(shown("a **b** c", 7..7), vec!["**", "**"]);
    }

    #[test]
    fn nested_elements_reveal_together() {
        assert_eq!(shown("**a *b* c**", 5..5), vec!["**", "**", "*", "*"]);
        assert_eq!(shown("**a *b* c**", 9..9), vec!["**", "**"]);
    }

    #[test]
    fn adjacent_elements_are_independent() {
        assert_eq!(shown("**a** **b**", 5..5), vec!["**", "**"]);
    }

    #[test]
    fn selection_reveals_everything_it_touches() {
        assert_eq!(shown("**a** x _b_", 3..9).len(), 4);
        assert_eq!(shown("**a** x _b_", 9..3).len(), 4);
    }

    #[test]
    fn line_markers_follow_their_line() {
        let text = "> one\n> two\n";
        assert_eq!(visible_markers(&parse(text), 9..9), vec![6..8]);
        assert_eq!(visible_markers(&parse(text), 5..5), vec![0..2]);
    }

    #[test]
    fn heading_and_code_block_scopes() {
        assert_eq!(shown("# T\ntext", 3..3), vec!["# "]);
        assert!(shown("# T\ntext", 5..5).is_empty());
        let code = "```\nx\n```\n";
        assert_eq!(shown(code, 4..4), vec!["```\n", "```\n"]);
    }
}
```

and `src/restyle.rs`:

```rust
//! Turns text and cursor changes into the minimal set of tag operations.

use std::ops::Range;

use crate::markdown::{Element, Kind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Style {
    Strong,
    Emphasis,
    Strike,
    Code,
    Link,
    Heading(u8),
    Quote,
    ListItem,
    CodeBlock,
    /// Dimmed colour of markup characters.
    Markup,
    /// Markup that is currently not shown.
    Hidden,
}

#[derive(Debug, Default, PartialEq)]
pub struct TagOps {
    pub clear: Option<Range<usize>>,
    pub remove: Vec<(Style, Range<usize>)>,
    pub add: Vec<(Style, Range<usize>)>,
}

pub fn element_style(_kind: &Kind) -> Option<Style> {
    None
}

#[derive(Default)]
pub struct Styler {
    elements: Vec<Element>,
}

impl Styler {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn elements(&self) -> &[Element] {
        &self.elements
    }
    pub fn is_visible(&self, _marker: &Range<usize>) -> bool {
        todo!()
    }
    pub fn reset(&mut self, _text: &str, _cursor: Range<usize>) -> TagOps {
        todo!()
    }
    pub fn text_changed(&mut self, _text: &str, _cursor: Range<usize>) -> TagOps {
        todo!()
    }
    pub fn cursor_moved(&mut self, _cursor: Range<usize>) -> TagOps {
        todo!()
    }
}

pub fn common_affixes(_a: &str, _b: &str) -> (usize, usize) {
    (0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(mut v: Vec<(Style, Range<usize>)>) -> Vec<(Style, Range<usize>)> {
        v.sort_by_key(|(s, r)| (r.start, r.end, *s));
        v
    }

    #[test]
    fn affixes() {
        assert_eq!(common_affixes("abc", "abc"), (3, 0));
        assert_eq!(common_affixes("abc", "abXc"), (2, 1));
        assert_eq!(common_affixes("aaa", "aa"), (2, 0));
        assert_eq!(common_affixes("", "x"), (0, 0));
        // "м" and "и" share their first UTF-8 byte: the prefix must not split it.
        assert_eq!(common_affixes("мир", "иир"), (0, 4));
    }

    #[test]
    fn reset_styles_everything_and_hides_markers_away_from_cursor() {
        let mut s = Styler::new();
        let ops = s.reset("a **b** c", 0..0);
        assert_eq!(ops.clear, Some(0..9));
        assert!(ops.remove.is_empty());
        assert_eq!(
            sorted(ops.add),
            vec![
                (Style::Markup, 2..4),
                (Style::Hidden, 2..4),
                (Style::Strong, 2..7),
                (Style::Markup, 5..7),
                (Style::Hidden, 5..7),
            ]
        );
    }

    #[test]
    fn reset_with_cursor_inside_keeps_markers_visible() {
        let mut s = Styler::new();
        let ops = s.reset("a **b** c", 4..4);
        assert!(!ops.add.iter().any(|(st, _)| *st == Style::Hidden));
        assert!(s.is_visible(&(2..4)));
    }

    #[test]
    fn cursor_moves_toggle_only_hidden() {
        let mut s = Styler::new();
        s.reset("a **b** c", 0..0);
        let ops = s.cursor_moved(4..4);
        assert_eq!(ops.clear, None);
        assert!(ops.add.is_empty());
        assert_eq!(sorted(ops.remove), vec![(Style::Hidden, 2..4), (Style::Hidden, 5..7)]);
        let ops = s.cursor_moved(9..9);
        assert_eq!(sorted(ops.add), vec![(Style::Hidden, 2..4), (Style::Hidden, 5..7)]);
        assert!(s.cursor_moved(9..9) == TagOps::default());
    }

    #[test]
    fn typing_plain_text_restyles_only_the_typed_span() {
        let mut s = Styler::new();
        s.reset("**a** x", 7..7);
        let ops = s.text_changed("**a** xy", 8..8);
        assert_eq!(ops.clear, Some(7..8));
        assert!(ops.add.is_empty() && ops.remove.is_empty());
    }

    #[test]
    fn typing_inside_an_element_restyles_that_element() {
        let mut s = Styler::new();
        s.reset("x **ab** y", 5..5);
        let ops = s.text_changed("x **aXb** y", 6..6);
        // Only the typed char is cleared; the element's tags are re-applied over it.
        assert_eq!(ops.clear, Some(5..6));
        assert_eq!(
            sorted(ops.add),
            vec![(Style::Markup, 2..4), (Style::Strong, 2..9), (Style::Markup, 7..9)]
        );
    }

    #[test]
    fn deleting_a_closing_delimiter_clears_the_old_element() {
        let mut s = Styler::new();
        s.reset("**bold** rest", 8..8);
        let ops = s.text_changed("**bold rest", 6..6);
        assert_eq!(ops.clear, Some(0..6));
        assert!(ops.add.is_empty());
    }

    #[test]
    fn edits_shift_untouched_elements_without_restyling_them() {
        let mut s = Styler::new();
        s.reset("ab **c**", 0..0);
        let ops = s.text_changed("abX **c**", 3..3);
        assert_eq!(ops.clear, Some(2..3));
        assert!(ops.add.is_empty() && ops.remove.is_empty());
        assert_eq!(s.elements()[0].range, 4..9);
    }

    #[test]
    fn edit_reveals_markers_of_an_untouched_element_the_cursor_reached() {
        let mut s = Styler::new();
        s.reset("ab **c**", 0..0);
        // Deleting "ab" moves the cursor to the element's start boundary.
        let ops = s.text_changed(" **c**", 0..0);
        assert!(ops.add.is_empty());
        assert_eq!(ops.clear, None);
        // cursor 0 is next to the space, not touching **c** (starts at 1)
        let ops = s.text_changed("**c**", 0..0);
        assert_eq!(ops.clear, None);
        assert_eq!(sorted(ops.remove), vec![(Style::Hidden, 0..2), (Style::Hidden, 3..5)]);
    }

    #[test]
    fn link_definition_below_restyles_the_link_above() {
        let mut s = Styler::new();
        s.reset("[a][r]\n\ntext", 12..12);
        let ops = s.text_changed("[a][r]\n\ntext\n\n[r]: http://x", 27..27);
        assert_eq!(ops.clear.map(|r| r.start), Some(0));
        assert!(ops.add.contains(&(Style::Link, 0..6)));
    }

    #[test]
    fn opening_a_fence_restyles_to_the_end() {
        let mut s = Styler::new();
        s.reset("a\n\n**b**\n", 0..0);
        let ops = s.text_changed("```\na\n\n**b**\n", 3..3);
        assert_eq!(ops.clear, Some(0..13));
        assert!(ops.add.contains(&(Style::CodeBlock, 0..13)));
        assert!(!ops.add.iter().any(|(st, _)| *st == Style::Strong));
    }
}
```

Add `pub mod restyle;` and `pub mod reveal;` to `src/lib.rs`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test --lib -- reveal restyle`
Expected: FAIL — all reveal tests except `cursor_outside_hides_everything`; all restyle tests (panics on `todo!()` / wrong affixes).

- [ ] **Step 3: Implement** — replace everything above `#[cfg(test)]` in `src/reveal.rs` with:

```rust
//! Which markup is shown for a given cursor or selection.

use std::ops::Range;

use crate::markdown::Element;

/// Byte ranges of the markers that must be visible when the cursor or
/// selection covers `cursor` (an empty range is a plain cursor). A marker is
/// visible when `cursor` touches its scope, boundaries included.
pub fn visible_markers(elements: &[Element], cursor: Range<usize>) -> Vec<Range<usize>> {
    let lo = cursor.start.min(cursor.end);
    let hi = cursor.start.max(cursor.end);
    elements
        .iter()
        .flat_map(|e| &e.markers)
        .filter(|m| m.scope.start <= hi && lo <= m.scope.end)
        .map(|m| m.range.clone())
        .collect()
}
```

and everything above `#[cfg(test)]` in `src/restyle.rs` with:

```rust
//! Turns text and cursor changes into the minimal set of tag operations.

use std::collections::HashSet;
use std::ops::Range;

use crate::markdown::{Element, Kind, parse};
use crate::reveal::visible_markers;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Style {
    Strong,
    Emphasis,
    Strike,
    Code,
    Link,
    Heading(u8),
    Quote,
    ListItem,
    CodeBlock,
    /// Dimmed colour of markup characters.
    Markup,
    /// Markup that is currently not shown.
    Hidden,
}

impl Style {
    pub const ALL: [Style; 16] = [
        Style::Strong,
        Style::Emphasis,
        Style::Strike,
        Style::Code,
        Style::Link,
        Style::Heading(1),
        Style::Heading(2),
        Style::Heading(3),
        Style::Heading(4),
        Style::Heading(5),
        Style::Heading(6),
        Style::Quote,
        Style::ListItem,
        Style::CodeBlock,
        Style::Markup,
        Style::Hidden,
    ];
}

/// Tag changes in byte offsets of the current text. Apply in order:
/// `clear` (remove every style there), then `remove`, then `add`.
#[derive(Debug, Default, PartialEq)]
pub struct TagOps {
    pub clear: Option<Range<usize>>,
    pub remove: Vec<(Style, Range<usize>)>,
    pub add: Vec<(Style, Range<usize>)>,
}

pub fn element_style(kind: &Kind) -> Option<Style> {
    Some(match kind {
        Kind::Strong => Style::Strong,
        Kind::Emphasis => Style::Emphasis,
        Kind::Strike => Style::Strike,
        Kind::Code => Style::Code,
        Kind::Link { .. } => Style::Link,
        Kind::Heading(n) => Style::Heading(*n),
        Kind::Quote => Style::Quote,
        Kind::ListItem { .. } | Kind::Task { .. } => Style::ListItem,
        Kind::CodeBlock => Style::CodeBlock,
        Kind::Rule => return None,
    })
}

/// Remembers what is currently styled so that edits only touch what changed.
#[derive(Default)]
pub struct Styler {
    text: String,
    elements: Vec<Element>,
    visible: Vec<Range<usize>>,
}

impl Styler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn elements(&self) -> &[Element] {
        &self.elements
    }

    pub fn is_visible(&self, marker: &Range<usize>) -> bool {
        self.visible.contains(marker)
    }

    /// Styles a whole new document.
    pub fn reset(&mut self, text: &str, cursor: Range<usize>) -> TagOps {
        self.text = text.to_owned();
        self.elements = parse(text);
        self.visible = visible_markers(&self.elements, cursor);
        let all = 0..text.len();
        let mut ops = TagOps { clear: Some(all.clone()), ..TagOps::default() };
        self.add_styles(&mut ops, &all);
        ops
    }

    /// The text was edited; `cursor` is the cursor/selection after the edit.
    pub fn text_changed(&mut self, text: &str, cursor: Range<usize>) -> TagOps {
        let (prefix, suffix) = common_affixes(&self.text, text);
        let shift = Shift { prefix, old_tail: self.text.len() - suffix, new_tail: text.len() - suffix };
        let elements = parse(text);

        // Elements come in document order, so the unchanged ones form a common
        // head and tail of both lists; whatever lies between them changed.
        let old = &self.elements;
        let head = old.iter().zip(&elements).take_while(|(o, n)| shift.same(o, n)).count();
        let max_tail = old.len().min(elements.len()) - head;
        let tail = old
            .iter()
            .rev()
            .zip(elements.iter().rev())
            .take(max_tail)
            .take_while(|(o, n)| shift.same(o, n))
            .count();
        let mut dirty = prefix..shift.new_tail;
        let changed_old = old[head..old.len() - tail].iter().map(|e| shift.range(&e.range));
        let changed_new = elements[head..elements.len() - tail].iter().map(|e| e.range.clone());
        for r in changed_old.chain(changed_new) {
            dirty.start = dirty.start.min(r.start);
            dirty.end = dirty.end.max(r.end);
        }

        let old_visible: Vec<Range<usize>> = self
            .visible
            .iter()
            .filter(|r| r.end <= shift.prefix || r.start >= shift.old_tail)
            .map(|r| shift.range(r))
            .collect();
        self.text = text.to_owned();
        self.elements = elements;
        self.visible = visible_markers(&self.elements, cursor);

        let mut ops = TagOps::default();
        if !dirty.is_empty() {
            ops.clear = Some(dirty.clone());
            self.add_styles(&mut ops, &dirty);
        }
        // Markers outside the dirty span kept their tags: fix only their visibility.
        let outside = |r: &Range<usize>| dirty.is_empty() || r.end <= dirty.start || r.start >= dirty.end;
        let now: HashSet<&Range<usize>> = self.visible.iter().collect();
        let before: HashSet<&Range<usize>> = old_visible.iter().collect();
        for r in old_visible.iter().filter(|r| outside(r) && !now.contains(r)) {
            ops.add.push((Style::Hidden, r.clone()));
        }
        for r in self.visible.iter().filter(|r| outside(r) && !before.contains(r)) {
            ops.remove.push((Style::Hidden, r.clone()));
        }
        ops
    }

    /// The cursor or selection moved without a text change.
    pub fn cursor_moved(&mut self, cursor: Range<usize>) -> TagOps {
        let visible = visible_markers(&self.elements, cursor);
        let now: HashSet<&Range<usize>> = visible.iter().collect();
        let before: HashSet<&Range<usize>> = self.visible.iter().collect();
        let mut ops = TagOps::default();
        for r in self.visible.iter().filter(|r| !now.contains(r)) {
            ops.add.push((Style::Hidden, r.clone()));
        }
        for r in visible.iter().filter(|r| !before.contains(r)) {
            ops.remove.push((Style::Hidden, r.clone()));
        }
        self.visible = visible;
        ops
    }

    fn add_styles(&self, ops: &mut TagOps, within: &Range<usize>) {
        let visible: HashSet<&Range<usize>> = self.visible.iter().collect();
        let touches = |r: &Range<usize>| r.start < within.end && within.start < r.end;
        for e in self.elements.iter().filter(|e| touches(&e.range)) {
            if let Some(style) = element_style(&e.kind) {
                ops.add.push((style, e.range.clone()));
            }
            for m in &e.markers {
                ops.add.push((Style::Markup, m.range.clone()));
                if !visible.contains(&m.range) {
                    ops.add.push((Style::Hidden, m.range.clone()));
                }
            }
        }
    }
}

/// Maps offsets of the old text to the new one around an edit that replaced
/// `prefix..old_tail` with `prefix..new_tail`.
struct Shift {
    prefix: usize,
    old_tail: usize,
    new_tail: usize,
}

impl Shift {
    /// Positions inside the replaced span collapse to its new end.
    fn pos(&self, p: usize) -> usize {
        if p <= self.prefix {
            p
        } else if p >= self.old_tail {
            p - self.old_tail + self.new_tail
        } else {
            self.new_tail
        }
    }

    fn range(&self, r: &Range<usize>) -> Range<usize> {
        self.pos(r.start)..self.pos(r.end)
    }

    /// `old` (from the old text) is `new` moved by the edit.
    fn same(&self, old: &Element, new: &Element) -> bool {
        old.kind == new.kind
            && self.range(&old.range) == new.range
            && old.markers.len() == new.markers.len()
            && old
                .markers
                .iter()
                .zip(&new.markers)
                .all(|(a, b)| self.range(&a.range) == b.range && self.range(&a.scope) == b.scope)
    }
}

/// Lengths in bytes of the common prefix and suffix of `a` and `b`; they never
/// overlap and always end on char boundaries.
pub fn common_affixes(a: &str, b: &str) -> (usize, usize) {
    let (ab, bb) = (a.as_bytes(), b.as_bytes());
    let max = ab.len().min(bb.len());
    let mut prefix = ab.iter().zip(bb).take_while(|(x, y)| x == y).count();
    while !a.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let mut suffix = ab.iter().rev().zip(bb.iter().rev()).take(max - prefix).take_while(|(x, y)| x == y).count();
    while !a.is_char_boundary(a.len() - suffix) {
        suffix -= 1;
    }
    (prefix, suffix)
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --lib`
Expected: PASS (all config, markdown, reveal, restyle tests), no warnings.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/reveal.rs src/restyle.rs
git commit -m "Add reveal rules and incremental Styler" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Editor core in GTK — GO/NO-GO GATE

**Files:**
- Modify: `Cargo.toml` (gtk dependency, `[[test]] gtk_editor`), `src/lib.rs` (add `pub mod editor;`, `pub mod offsets;`)
- Create: `src/offsets.rs`, `src/editor/mod.rs`, `src/editor/imp.rs`, `src/editor/tags.rs`, `tests/common/mod.rs`, `tests/gtk_editor.rs`

**Interfaces:**
- Consumes: `config::{Config, Theme}`, `restyle::{Style, Styler, TagOps}`.
- Produces:
  - `opal::offsets::CharIndex` — `new(&str)`, `char_len() -> i32`, `char_at(byte: usize) -> i32`, `byte_at(char: i32) -> usize` (both clamp), `Default`.
  - `opal::editor::OpalView` (a `gtk::TextView`): `new(&Config) -> Self`, `load_text(&self, &str)` (no undo history, cursor at start, unmodified), `text(&self) -> String` (with hidden markup), `has_style_at(&self, Style, char_offset: i32) -> bool`, `is_hidden_at(&self, i32) -> bool`, `char_width(&self) -> f64`, `line_height(&self) -> i32`; private `recenter(&self)` used by later tasks.
  - `editor::imp::OpalView` fields used by Tasks 5–6: `side`, `vertical`, `typewriter`, `column_chars`, `char_width`, `line_height`, `checkboxes`, `tags`, `palette`, `styler`, `index`.
  - `tests/common` helpers: `run`, `pump_for`, `settle`, `wait_until`, `show`, `save_png`.

- [ ] **Step 1: Check the GTK development files**

Run: `pkg-config --modversion gtk4`
Expected: a version ≥ 4.10 (Debian testing: 4.24.x). If the package is missing, STOP and ask the user to run `! sudo apt install libgtk-4-dev` (needs sudo; the executor cannot do it).

- [ ] **Step 2: Add the dependency and the test target** — in `Cargo.toml` add under `[dependencies]`:

```toml
gtk = { package = "gtk4", version = "0.11", features = ["v4_10"] }
```

and at the end:

```toml
[[test]]
name = "gtk_editor"
harness = false
```

Add `pub mod editor;` and `pub mod offsets;` to `src/lib.rs`.

- [ ] **Step 3: Write `src/offsets.rs` test-first** — create it with stubs and tests:

```rust
//! Byte offsets (pulldown-cmark) ↔ char offsets (GtkTextBuffer).

pub struct CharIndex {
    map: Vec<u32>,
}

impl CharIndex {
    pub fn new(_text: &str) -> Self {
        CharIndex { map: vec![0] }
    }
    pub fn char_len(&self) -> i32 {
        0
    }
    pub fn char_at(&self, _b: usize) -> i32 {
        0
    }
    pub fn byte_at(&self, _c: i32) -> usize {
        self.map.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_is_identity() {
        let ix = CharIndex::new("abc");
        assert_eq!((ix.char_at(0), ix.char_at(3), ix.char_len()), (0, 3, 3));
        assert_eq!((ix.byte_at(0), ix.byte_at(3)), (0, 3));
    }

    #[test]
    fn cyrillic_bytes_map_to_chars() {
        let text = "Привет **мир**";
        let ix = CharIndex::new(text);
        assert_eq!(ix.char_at(13), 7); // first `*`
        assert_eq!(ix.char_at(text.len()), 14);
        assert_eq!(ix.byte_at(7), 13);
        assert_eq!(ix.byte_at(1), 2);
        assert_eq!(ix.char_len(), 14);
    }

    #[test]
    fn out_of_range_is_clamped() {
        let ix = CharIndex::new("ж");
        assert_eq!(ix.char_at(99), 1);
        assert_eq!(ix.byte_at(99), 2);
        assert_eq!(ix.byte_at(-5), 0);
    }

    #[test]
    fn empty_text() {
        let ix = CharIndex::new("");
        assert_eq!((ix.char_at(0), ix.byte_at(0), ix.char_len()), (0, 0, 0));
    }
}
```

Run: `cargo test --lib offsets` — Expected: FAIL (`ascii_is_identity`, `cyrillic_bytes_map_to_chars`, `out_of_range_is_clamped`).

Then replace everything above `#[cfg(test)]` with:

```rust
//! Byte offsets (pulldown-cmark) ↔ char offsets (GtkTextBuffer).

/// Char offset for every byte position of a text, plus one entry for its end.
pub struct CharIndex {
    map: Vec<u32>,
}

impl CharIndex {
    pub fn new(text: &str) -> Self {
        let mut map = Vec::with_capacity(text.len() + 1);
        let mut chars = 0u32;
        for ch in text.chars() {
            for _ in 0..ch.len_utf8() {
                map.push(chars);
            }
            chars += 1;
        }
        map.push(chars);
        CharIndex { map }
    }

    /// Number of chars in the text.
    pub fn char_len(&self) -> i32 {
        *self.map.last().expect("map always has the end entry") as i32
    }

    /// Char offset of byte `b` (clamped to the end of the text).
    pub fn char_at(&self, b: usize) -> i32 {
        self.map[b.min(self.map.len() - 1)] as i32
    }

    /// Byte offset where char `c` starts (clamped to the end of the text).
    pub fn byte_at(&self, c: i32) -> usize {
        let c = c.max(0) as u32;
        self.map.partition_point(|&x| x < c).min(self.map.len() - 1)
    }
}

impl Default for CharIndex {
    fn default() -> Self {
        CharIndex::new("")
    }
}
```

Run: `cargo test --lib offsets` — Expected: PASS (4 tests).

- [ ] **Step 4: Write the GTK test harness** — `tests/common/mod.rs`:

```rust
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
```

- [ ] **Step 5: Write the failing GTK tests** — `tests/gtk_editor.rs`:

```rust
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
```

- [ ] **Step 6: Write the tags module** — `src/editor/tags.rs`:

```rust
//! Text tags, colours and the body font.

use std::collections::HashMap;

use gtk::prelude::*;
use gtk::{gdk, pango};

use crate::config::{Config, Theme};
use crate::restyle::Style;

const HEADING_SCALE: [f64; 6] = [1.8, 1.5, 1.25, 1.1, 1.0, 1.0];

pub struct Palette {
    pub markup: gdk::RGBA,
    pub link: gdk::RGBA,
    pub code_bg: gdk::RGBA,
    /// Quote bars and rules.
    pub line: gdk::RGBA,
    /// Bullets and check boxes.
    pub glyph: gdk::RGBA,
}

impl Palette {
    pub fn for_theme(theme: Theme) -> Self {
        let c = |hex: &str| gdk::RGBA::parse(hex).expect("valid colour literal");
        match theme {
            Theme::Light => Palette {
                markup: c("#a3a3a3"),
                link: c("#2766b0"),
                code_bg: c("#f1f1f1"),
                line: c("#c9c9c9"),
                glyph: c("#7a7a7a"),
            },
            Theme::Dark => Palette {
                markup: c("#6e6e6e"),
                link: c("#79a9e6"),
                code_bg: c("#2b2b2b"),
                line: c("#4f4f4f"),
                glyph: c("#9a9a9a"),
            },
        }
    }
}

pub struct Tags(HashMap<Style, gtk::TextTag>);

impl Tags {
    /// One tag per style, added in `Style::ALL` order: later tags win, so
    /// `Markup` and `Hidden` override everything else.
    pub fn new(table: &gtk::TextTagTable, config: &Config, palette: &Palette) -> Self {
        let mono = pango::FontDescription::from_string(&config.mono_font);
        let mut map = HashMap::new();
        for style in Style::ALL {
            let b = gtk::TextTag::builder().name(tag_name(style));
            let tag = match style {
                Style::Strong => b.weight(700),
                Style::Emphasis => b.style(pango::Style::Italic),
                Style::Strike => b.strikethrough(true),
                Style::Code => b.font_desc(&mono).background_rgba(&palette.code_bg),
                Style::Link => b.foreground_rgba(&palette.link).underline(pango::Underline::Single),
                Style::Heading(n) => b.weight(700).scale(HEADING_SCALE[n as usize - 1]).pixels_above_lines(8),
                // Their left margin follows the column; see editor::margins.
                Style::Quote | Style::ListItem => b,
                Style::CodeBlock => b.font_desc(&mono),
                Style::Markup => b.foreground_rgba(&palette.markup),
                Style::Hidden => b.invisible(true),
            }
            .build();
            table.add(&tag);
            map.insert(style, tag);
        }
        Tags(map)
    }

    pub fn get(&self, style: Style) -> &gtk::TextTag {
        &self.0[&style]
    }
}

fn tag_name(style: Style) -> String {
    match style {
        Style::Strong => "strong".into(),
        Style::Emphasis => "em".into(),
        Style::Strike => "strike".into(),
        Style::Code => "code".into(),
        Style::Link => "link".into(),
        Style::Heading(n) => format!("h{n}"),
        Style::Quote => "quote".into(),
        Style::ListItem => "list-item".into(),
        Style::CodeBlock => "code-block".into(),
        Style::Markup => "markup".into(),
        Style::Hidden => "hidden".into(),
    }
}

/// Sets the body font of `textview.opal` widgets from a Pango description.
pub fn install_font_css(font: &pango::FontDescription) {
    let mut css = String::from("textview.opal {");
    if let Some(family) = font.family() {
        css.push_str(&format!(" font-family: \"{family}\";"));
    }
    if font.size() > 0 {
        let unit = if font.is_size_absolute() { "px" } else { "pt" };
        css.push_str(&format!(" font-size: {}{unit};", font.size() as f64 / pango::SCALE as f64));
    }
    css.push_str(" }");
    let provider = gtk::CssProvider::new();
    provider.load_from_data(&css);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
}
```

- [ ] **Step 7: Write the widget state** — `src/editor/imp.rs`:

```rust
//! State of the OpalView widget.

use std::cell::{Cell, OnceCell, RefCell};

use gtk::gdk;
use gtk::glib;
use gtk::subclass::prelude::*;

use super::tags::{Palette, Tags};
use crate::offsets::CharIndex;
use crate::restyle::Styler;

#[derive(Default)]
pub struct OpalView {
    pub styler: RefCell<Styler>,
    /// Byte ↔ char mapping of the current text.
    pub index: RefCell<CharIndex>,
    pub tags: OnceCell<Tags>,
    pub palette: OnceCell<Palette>,
    /// Column width in characters and the font metrics that turn it into pixels.
    pub column_chars: Cell<u32>,
    pub char_width: Cell<f64>,
    pub line_height: Cell<i32>,
    pub typewriter: Cell<bool>,
    /// Current side and top/bottom margins in pixels.
    pub side: Cell<i32>,
    pub vertical: Cell<i32>,
    /// Set while a document is loaded: the signal handlers stay quiet.
    pub loading: Cell<bool>,
    /// A mouse button is held (e.g. selecting): typewriter mode must not scroll.
    pub mouse_down: Cell<bool>,
    /// `OPAL_BENCH=restyle`: print how long every restyle took.
    pub bench: Cell<bool>,
    /// Check boxes drawn in the last frame: rectangle in buffer coordinates and
    /// byte offset of the character between the brackets.
    pub checkboxes: RefCell<Vec<(gdk::Rectangle, usize)>>,
}

#[glib::object_subclass]
impl ObjectSubclass for OpalView {
    const NAME: &'static str = "OpalView";
    type Type = super::OpalView;
    type ParentType = gtk::TextView;
}

impl ObjectImpl for OpalView {}
impl WidgetImpl for OpalView {}
impl TextViewImpl for OpalView {}
```

- [ ] **Step 8: Write the widget** — `src/editor/mod.rs`:

```rust
//! The Markdown text view: styling, hidden markup, typewriter scrolling.

mod imp;
mod tags;

use std::ops::Range;
use std::time::Instant;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, pango};

use crate::config::Config;
use crate::offsets::CharIndex;
use crate::restyle::{Style, TagOps};
use tags::{Palette, Tags};

glib::wrapper! {
    pub struct OpalView(ObjectSubclass<imp::OpalView>)
        @extends gtk::TextView, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Scrollable;
}

impl OpalView {
    pub fn new(config: &Config) -> Self {
        let view: Self = glib::Object::new();
        view.setup(config);
        view
    }

    fn setup(&self, config: &Config) {
        let imp = self.imp();
        self.set_wrap_mode(gtk::WrapMode::WordChar);
        self.add_css_class("opal");
        let font = pango::FontDescription::from_string(&config.font);
        tags::install_font_css(&font);
        let palette = Palette::for_theme(config.theme);
        let _ = imp.tags.set(Tags::new(&self.buffer().tag_table(), config, &palette));
        let _ = imp.palette.set(palette);
        let metrics = self.pango_context().metrics(Some(&font), None);
        imp.char_width.set(metrics.approximate_char_width() as f64 / pango::SCALE as f64);
        imp.line_height.set((metrics.ascent() + metrics.descent()) / pango::SCALE);
        imp.column_chars.set(config.column_width);
        imp.typewriter.set(config.typewriter);
        imp.bench.set(std::env::var("OPAL_BENCH").as_deref() == Ok("restyle"));
        self.connect_buffer_signals();
        self.connect_mouse_tracking();
    }

    /// Replaces the whole text: no undo history, cursor at the start, unmodified.
    pub fn load_text(&self, text: &str) {
        let imp = self.imp();
        let buffer = self.buffer();
        imp.loading.set(true);
        buffer.begin_irreversible_action();
        buffer.set_text(text);
        buffer.end_irreversible_action();
        buffer.place_cursor(&buffer.start_iter());
        imp.loading.set(false);
        let ops = imp.styler.borrow_mut().reset(text, 0..0);
        *imp.index.borrow_mut() = CharIndex::new(text);
        self.apply(&ops);
        buffer.set_modified(false);
        self.recenter();
    }

    /// The document text, including hidden markup.
    pub fn text(&self) -> String {
        let buffer = self.buffer();
        buffer.text(&buffer.start_iter(), &buffer.end_iter(), true).to_string()
    }

    pub fn has_style_at(&self, style: Style, char_offset: i32) -> bool {
        let tag = self.imp().tags.get().expect("tags are created in setup").get(style);
        self.buffer().iter_at_offset(char_offset).has_tag(tag)
    }

    pub fn is_hidden_at(&self, char_offset: i32) -> bool {
        self.has_style_at(Style::Hidden, char_offset)
    }

    pub fn char_width(&self) -> f64 {
        self.imp().char_width.get()
    }

    pub fn line_height(&self) -> i32 {
        self.imp().line_height.get()
    }

    fn connect_buffer_signals(&self) {
        let buffer = self.buffer();
        let weak = self.downgrade();
        buffer.connect_changed(move |_| {
            if let Some(view) = weak.upgrade() {
                view.on_text_changed();
            }
        });
        let weak = self.downgrade();
        buffer.connect_mark_set(move |buffer, _, mark| {
            if *mark == buffer.get_insert() || *mark == buffer.selection_bound() {
                if let Some(view) = weak.upgrade() {
                    view.on_cursor_moved();
                }
            }
        });
    }

    /// Tracks the mouse button so that typewriter mode does not scroll while
    /// the user selects with the mouse; it re-centres on release.
    fn connect_mouse_tracking(&self) {
        let legacy = gtk::EventControllerLegacy::new();
        legacy.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.downgrade();
        legacy.connect_event(move |_, event| {
            if let Some(view) = weak.upgrade() {
                match event.event_type() {
                    gdk::EventType::ButtonPress => view.imp().mouse_down.set(true),
                    gdk::EventType::ButtonRelease => {
                        view.imp().mouse_down.set(false);
                        view.recenter();
                    }
                    _ => {}
                }
            }
            glib::Propagation::Proceed
        });
        self.add_controller(legacy);
    }

    fn on_text_changed(&self) {
        let imp = self.imp();
        if imp.loading.get() {
            return;
        }
        let started = Instant::now();
        let buffer = self.buffer();
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), true);
        let index = CharIndex::new(&text);
        let cursor = cursor_bytes(&buffer, &index);
        let ops = imp.styler.borrow_mut().text_changed(&text, cursor);
        *imp.index.borrow_mut() = index;
        self.apply(&ops);
        if imp.bench.get() {
            eprintln!("opal: restyle {:?}", started.elapsed());
        }
        self.recenter();
    }

    fn on_cursor_moved(&self) {
        let imp = self.imp();
        if imp.loading.get() {
            return;
        }
        let buffer = self.buffer();
        // Inside an insertion the cursor can move before `changed` arrives;
        // the `changed` handler then takes care of the cursor too.
        if buffer.char_count() != imp.index.borrow().char_len() {
            return;
        }
        let cursor = cursor_bytes(&buffer, &imp.index.borrow());
        let ops = imp.styler.borrow_mut().cursor_moved(cursor);
        self.apply(&ops);
        if !imp.mouse_down.get() {
            self.recenter();
        }
    }

    fn apply(&self, ops: &TagOps) {
        let imp = self.imp();
        let tags = imp.tags.get().expect("tags are created in setup");
        let buffer = self.buffer();
        let index = imp.index.borrow();
        let iter = |b: usize| buffer.iter_at_offset(index.char_at(b));
        if let Some(r) = &ops.clear {
            for style in Style::ALL {
                buffer.remove_tag(tags.get(style), &iter(r.start), &iter(r.end));
            }
        }
        for (style, r) in &ops.remove {
            buffer.remove_tag(tags.get(*style), &iter(r.start), &iter(r.end));
        }
        for (style, r) in &ops.add {
            buffer.apply_tag(tags.get(*style), &iter(r.start), &iter(r.end));
        }
    }

    /// In typewriter mode, scrolls so that the cursor line is in the middle.
    fn recenter(&self) {
        if self.imp().typewriter.get() {
            self.scroll_to_mark(&self.buffer().get_insert(), 0.0, true, 0.0, 0.5);
        }
    }
}

/// Cursor and selection bound as a byte range (possibly reversed).
fn cursor_bytes(buffer: &gtk::TextBuffer, index: &CharIndex) -> Range<usize> {
    let insert = buffer.iter_at_mark(&buffer.get_insert()).offset();
    let bound = buffer.iter_at_mark(&buffer.selection_bound()).offset();
    index.byte_at(insert)..index.byte_at(bound)
}
```

- [ ] **Step 9: Build and run the pure tests**

Run: `cargo build && cargo test --lib`
Expected: builds without warnings; all lib tests PASS. Fix any gtk4-rs API name mismatches here (see Global Constraints).

- [ ] **Step 10: Run the GTK tests (debug, then release)**

Run: `cargo test --test gtk_editor` then `cargo test --release --test gtk_editor`
Expected: all 11 tests print `ok`; the release run prints a restyle median under 5 ms.

- [ ] **Step 11: GO/NO-GO decision**

GO if `arrows_walk_through_hidden_markup`, `vertical_move_lands_on_text`, `tag_changes_are_not_undoable`, `undo_restores_styling` and `restyle_is_fast` (release) pass. Note the printed `right:`/`left:` cursor sequences in the task report.

NO-GO if the cursor gets stuck or jumps over visible text near hidden markup, or tag changes enter the undo history, and a fix within GtkTextView is not found in about an hour (e.g. re-applying `Hidden` from an idle callback, or moving the cursor out of hidden runs in `on_cursor_moved`). On NO-GO: STOP, do not continue with Task 5, and report to the user with the evidence and the fallback from spec 3.10 — keep markup visible but dimmed (no `Hidden` tag) — and wait for their decision.

If `restyle_is_fast` fails only on time: check whether `parse` or GTK dominates (`OPAL_BENCH=restyle` timings plus a quick `Instant` around `apply`), report numbers; this is not a NO-GO by itself.

- [ ] **Step 12: Commit**

```bash
git add Cargo.toml Cargo.lock src/lib.rs src/offsets.rs src/editor tests
git commit -m "Add GTK editor view with hidden markup" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Centred column and typewriter mode

**Files:**
- Create: `src/layout.rs`, `src/editor/margins.rs`
- Modify: `src/lib.rs` (add `pub mod layout;`), `src/editor/mod.rs` (add `mod margins;`), `src/editor/imp.rs` (`size_allocate`), `tests/gtk_editor.rs` (two tests)

**Interfaces:**
- Consumes: `OpalView` internals from Task 4 (`imp` fields, private `recenter`), `Style::{Quote, ListItem}` tags.
- Produces:
  - `opal::layout::{MIN_SIDE_MARGIN, side_margin(view_width: i32, column_chars: u32, char_width: f64) -> i32, vertical_margin(view_height: i32, line_height: i32, typewriter: bool) -> i32}`.
  - `OpalView::typewriter(&self) -> bool`, `OpalView::set_typewriter(&self, on: bool)`; `pub(super) fn update_margins(&self, width: i32, height: i32)`; `pub(super) const LIST_INDENT: i32 = 24` (used by Task 6).

GTK detail: a tag's `left-margin` *replaces* the view's left margin for its lines, so the `quote`/`list-item` tags get `side + indent` whenever the side margin changes. Margins change the allocation, so they are applied from an idle callback and only when the value differs — no allocation loop.

- [ ] **Step 1: Write `src/layout.rs` test-first** — create it with stubs and tests:

```rust
//! Geometry of the centred column and typewriter mode, in pixels.

/// Smallest side margin when the window is narrower than the column.
pub const MIN_SIDE_MARGIN: i32 = 16;

pub fn side_margin(_view_width: i32, _column_chars: u32, _char_width: f64) -> i32 {
    0
}

pub fn vertical_margin(_view_height: i32, _line_height: i32, _typewriter: bool) -> i32 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_view_centres_the_column() {
        assert_eq!(side_margin(1000, 70, 10.0), 150);
    }

    #[test]
    fn narrow_view_keeps_a_minimal_margin() {
        assert_eq!(side_margin(500, 70, 10.0), MIN_SIDE_MARGIN);
    }

    #[test]
    fn fractional_char_width() {
        assert_eq!(side_margin(1000, 70, 8.4), 206); // column 588 px
    }

    #[test]
    fn typewriter_margin_centres_a_line() {
        assert_eq!(vertical_margin(800, 20, true), 390);
        assert_eq!(vertical_margin(10, 20, true), 0);
    }

    #[test]
    fn normal_margin_is_two_lines() {
        assert_eq!(vertical_margin(800, 20, false), 40);
    }
}
```

Add `pub mod layout;` to `src/lib.rs`. Run: `cargo test --lib layout` — Expected: FAIL (all 5 tests).

Replace everything above `#[cfg(test)]` with:

```rust
//! Geometry of the centred column and typewriter mode, in pixels.

/// Smallest side margin when the window is narrower than the column.
pub const MIN_SIDE_MARGIN: i32 = 16;

/// Left and right margin that centres a column of `column_chars` characters.
pub fn side_margin(view_width: i32, column_chars: u32, char_width: f64) -> i32 {
    let column = (column_chars as f64 * char_width).round() as i32;
    ((view_width - column) / 2).max(MIN_SIDE_MARGIN)
}

/// Top and bottom margin. In typewriter mode it lets the first and the last
/// line sit in the vertical centre of the view.
pub fn vertical_margin(view_height: i32, line_height: i32, typewriter: bool) -> i32 {
    if typewriter { ((view_height - line_height) / 2).max(0) } else { 2 * line_height }
}
```

Run: `cargo test --lib layout` — Expected: PASS (5 tests).

- [ ] **Step 2: Write the failing GTK tests** — in `tests/gtk_editor.rs` add to the `common::run` list:

```rust
        ("column_is_centred", column_is_centred),
        ("typewriter_keeps_cursor_line_centred", typewriter_keeps_cursor_line_centred),
```

and the functions:

```rust
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
```

Run: `cargo test --test gtk_editor column` — Expected: compile error (`set_typewriter` missing) — that is the failing state.

- [ ] **Step 3: Implement the margins** — `src/editor/margins.rs`:

```rust
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
```

In `src/editor/mod.rs` add `mod margins;` after `mod imp;`.

In `src/editor/imp.rs` replace `impl WidgetImpl for OpalView {}` with:

```rust
impl WidgetImpl for OpalView {
    fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
        self.parent_size_allocate(width, height, baseline);
        self.obj().update_margins(width, height);
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --lib && cargo test --test gtk_editor`
Expected: all PASS, including `column_is_centred` and `typewriter_keeps_cursor_line_centred`.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/layout.rs src/editor tests/gtk_editor.rs
git commit -m "Centre the text column and add typewriter mode" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Decorations, check boxes and links

**Files:**
- Create: `src/editor/decor.rs`
- Modify: `src/editor/mod.rs` (`mod decor;`, call `connect_clicks` in `setup`), `src/editor/imp.rs` (`snapshot_layer`), `tests/gtk_editor.rs` (three tests)

**Interfaces:**
- Consumes: `Styler::{elements, is_visible, text}`, `Kind`, `imp` fields `side`, `palette`, `checkboxes`, `index`, `styler`; `margins::LIST_INDENT` (only as documentation of the room the glyphs use).
- Produces: `OpalView::checkbox_rects(&self) -> Vec<(gdk::Rectangle, usize)>`, `OpalView::toggle_task(&self, byte: usize)`, `OpalView::link_at(&self, char_offset: i32) -> Option<String>`, `opal::editor::link_uri(url: &str) -> String`.

What is drawn (spec 2.2, 3.5), in buffer coordinates from `iter_location`/`line_yrange`: below the text — code block background within the column; above the text — quote bar (always), rule line, `•` for unordered items, ☐/☑ for tasks (only while their marker is hidden). Only elements intersecting the visible rectangle are visited.

Coordinates: this plan assumes `snapshot_layer` draws in buffer coordinates (GTK translates the snapshot by the scroll offset). The screenshot in Step 5 confirms it; if decorations are shifted by the scroll position or the margins, convert with `self.buffer_to_window_coords(gtk::TextWindowType::Widget, x, y)` before drawing.

- [ ] **Step 1: Write the failing tests** — in `tests/gtk_editor.rs` add to the `common::run` list:

```rust
        ("decorations_render", decorations_render),
        ("checkbox_toggles", checkbox_toggles),
        ("link_lookup", link_lookup),
```

and the functions:

```rust
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
    let text_x = view.iter_location(&view.buffer().iter_at_offset(6)).x(); // 'з'
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
```

Run: `cargo test --test gtk_editor decorations` — Expected: compile error (`checkbox_rects` missing).

- [ ] **Step 2: Implement** — `src/editor/decor.rs`:

```rust
//! Decorations drawn around the text, and clicks on check boxes and links.

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, graphene};

use super::OpalView;
use crate::markdown::Kind;

/// Space between a drawn glyph and the text it precedes.
const GLYPH_GAP: i32 = 6;

impl OpalView {
    pub(super) fn draw_decorations(&self, layer: gtk::TextViewLayer, snapshot: &gtk::Snapshot) {
        let imp = self.imp();
        let Some(palette) = imp.palette.get() else { return };
        let styler = imp.styler.borrow();
        let index = imp.index.borrow();
        let buffer = self.buffer();
        let rect = self.visible_rect();
        let (Some(top), Some(bottom)) = (
            self.iter_at_location(rect.x(), rect.y()),
            self.iter_at_location(rect.x(), rect.y() + rect.height()),
        ) else {
            return;
        };
        let (top, bottom) = (index.byte_at(top.offset()), index.byte_at(bottom.offset()));
        let side = imp.side.get() as f32;
        let column = (self.width() - 2 * imp.side.get()) as f32;
        let iter = |b: usize| buffer.iter_at_offset(index.char_at(b));
        // Top of the first line and bottom of the last line of a byte range.
        let span = |start: usize, end: usize| {
            let (y0, _) = self.line_yrange(&iter(start));
            let (y1, h1) = self.line_yrange(&iter(end.saturating_sub(1).max(start)));
            (y0 as f32, (y1 + h1 - y0) as f32)
        };
        let above = layer == gtk::TextViewLayer::AboveText;
        if above {
            imp.checkboxes.borrow_mut().clear();
        }
        for e in styler.elements().iter().filter(|e| e.range.start <= bottom && e.range.end >= top) {
            let hidden = e.markers.first().is_some_and(|m| !styler.is_visible(&m.range));
            match &e.kind {
                Kind::CodeBlock if !above => {
                    let (y, h) = span(e.range.start, e.range.end);
                    snapshot.append_color(&palette.code_bg, &graphene::Rect::new(side - 8.0, y, column + 16.0, h));
                }
                Kind::Quote if above => {
                    let (y, h) = span(e.range.start, e.range.end);
                    snapshot.append_color(&palette.line, &graphene::Rect::new(side + 2.0, y, 3.0, h));
                }
                Kind::Rule if above && hidden => {
                    let (y, h) = self.line_yrange(&iter(e.range.start));
                    let middle = (y + h / 2) as f32;
                    snapshot.append_color(&palette.line, &graphene::Rect::new(side, middle, column, 1.0));
                }
                Kind::ListItem { ordered: false } if above && hidden => {
                    self.draw_glyph(snapshot, "•", &iter(e.markers[0].range.end), &palette.glyph);
                }
                Kind::Task { checked } if above && hidden => {
                    let marker = &e.markers[0].range;
                    let glyph = if *checked { "☑" } else { "☐" };
                    let rect = self.draw_glyph(snapshot, glyph, &iter(marker.end), &palette.glyph);
                    let bracket = styler.text()[marker.clone()].find('[').unwrap_or(0);
                    imp.checkboxes.borrow_mut().push((rect, marker.start + bracket + 1));
                }
                _ => {}
            }
        }
    }

    /// Draws `glyph` left of the text at `at`; returns where it went.
    fn draw_glyph(&self, snapshot: &gtk::Snapshot, glyph: &str, at: &gtk::TextIter, color: &gdk::RGBA) -> gdk::Rectangle {
        let loc = self.iter_location(at);
        let layout = self.create_pango_layout(Some(glyph));
        let (w, h) = layout.pixel_size();
        let x = loc.x() - w - GLYPH_GAP;
        let y = loc.y() + (loc.height() - h) / 2;
        snapshot.save();
        snapshot.translate(&graphene::Point::new(x as f32, y as f32));
        snapshot.append_layout(&layout, color);
        snapshot.restore();
        gdk::Rectangle::new(x, y, w, h)
    }

    pub fn checkbox_rects(&self) -> Vec<(gdk::Rectangle, usize)> {
        self.imp().checkboxes.borrow().clone()
    }

    /// Flips `[ ]` ↔ `[x]`; `byte` is the offset of the char between the brackets.
    pub fn toggle_task(&self, byte: usize) {
        let buffer = self.buffer();
        let offset = self.imp().index.borrow().char_at(byte);
        let mut start = buffer.iter_at_offset(offset);
        let mut end = buffer.iter_at_offset(offset + 1);
        let replacement = if start.char() == ' ' { "x" } else { " " };
        buffer.begin_user_action();
        buffer.delete(&mut start, &mut end);
        buffer.insert(&mut start, replacement);
        buffer.end_user_action();
    }

    /// URL of the link at a char offset.
    pub fn link_at(&self, char_offset: i32) -> Option<String> {
        let imp = self.imp();
        let byte = imp.index.borrow().byte_at(char_offset);
        imp.styler.borrow().elements().iter().find_map(|e| match &e.kind {
            Kind::Link { url } if e.range.start <= byte && byte < e.range.end => Some(url.clone()),
            _ => None,
        })
    }

    /// Clicks on drawn check boxes toggle them; Ctrl+click opens links.
    pub(super) fn connect_clicks(&self) {
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_PRIMARY);
        click.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.downgrade();
        click.connect_pressed(move |gesture, _, x, y| {
            let Some(view) = weak.upgrade() else { return };
            let (bx, by) = view.window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);
            if gesture.current_event_state().contains(gdk::ModifierType::CONTROL_MASK) {
                let url = view.iter_at_location(bx, by).and_then(|it| view.link_at(it.offset()));
                if let Some(url) = url {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                    view.open_link(&url);
                    return;
                }
            }
            let hit = view.checkbox_rects().into_iter().find(|(r, _)| r.contains_point(bx, by));
            if let Some((_, byte)) = hit {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                view.toggle_task(byte);
            }
        });
        self.add_controller(click);
    }

    fn open_link(&self, url: &str) {
        let launcher = gtk::UriLauncher::builder().uri(link_uri(url)).build();
        let window = self.root().and_downcast::<gtk::Window>();
        launcher.launch(window.as_ref(), gio::Cancellable::NONE, |result| {
            if let Err(e) = result {
                eprintln!("opal: не удалось открыть ссылку: {e}");
            }
        });
    }
}

/// `<me@x.org>` autolinks carry a bare address; give it a scheme.
pub fn link_uri(url: &str) -> String {
    if !url.contains(':') && url.contains('@') { format!("mailto:{url}") } else { url.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_uris() {
        assert_eq!(link_uri("http://x"), "http://x");
        assert_eq!(link_uri("me@x.org"), "mailto:me@x.org");
        assert_eq!(link_uri("mailto:me@x.org"), "mailto:me@x.org");
        assert_eq!(link_uri("notes.md"), "notes.md");
    }
}
```

In `src/editor/mod.rs`: add `mod decor;` after `mod imp;`, add `pub use decor::link_uri;` after the `use` lines, and in `setup` add `self.connect_clicks();` after `self.connect_mouse_tracking();`.

In `src/editor/imp.rs` replace `impl TextViewImpl for OpalView {}` with:

```rust
impl TextViewImpl for OpalView {
    fn snapshot_layer(&self, layer: gtk::TextViewLayer, snapshot: gtk::Snapshot) {
        self.parent_snapshot_layer(layer, snapshot.clone());
        self.obj().draw_decorations(layer, &snapshot);
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --lib && cargo test --test gtk_editor`
Expected: all PASS, including `link_uris`, `decorations_render`, `checkbox_toggles`, `link_lookup`.

- [ ] **Step 4: Check the screenshot**

Open the PNG printed by `decorations_render` (`target/tmp/opal-decorations.png`) with the Read tool. Expected: large heading without `#`; a grey bar left of both quote lines with no `>`; `•` left of «пункт»; ☐ before «задача» and ☑ before «готово» with no `- [ ]`; a thin horizontal line where `---` was; a light background behind «код» within the column and no visible fences. If decorations are shifted, apply the coordinate fix described above and repeat.

- [ ] **Step 5: Commit**

```bash
git add src/editor tests/gtk_editor.rs
git commit -m "Draw bullets, check boxes, quote bars and rules" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Document files

**Files:**
- Create: `src/document.rs`
- Modify: `src/lib.rs` (add `pub mod document;`)

**Interfaces:**
- Consumes: nothing.
- Produces: `opal::document::{Stamp, LoadError, SaveOutcome, resolve, stamp, load, save}`:
  - `pub struct Stamp { pub modified: Option<SystemTime>, pub len: u64 }` (`Copy`, `PartialEq`)
  - `pub enum LoadError { NotUtf8, Io(io::Error) }` with a Russian `Display`
  - `pub enum SaveOutcome { Saved(Stamp), Conflict }`
  - `resolve(&Path) -> PathBuf`, `stamp(&Path) -> io::Result<Option<Stamp>>`, `load(&Path) -> Result<(String, Option<Stamp>), LoadError>`, `save(path: &Path, text: &str, expected: Option<Stamp>, force: bool) -> io::Result<SaveOutcome>`

- [ ] **Step 1: Write the failing tests** — `src/document.rs`:

```rust
//! Reading and safely writing the document file.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub modified: Option<SystemTime>,
    pub len: u64,
}

#[derive(Debug)]
pub enum LoadError {
    NotUtf8,
    Io(io::Error),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SaveOutcome {
    Saved(Stamp),
    Conflict,
}

pub fn resolve(_path: &Path) -> PathBuf {
    todo!()
}

pub fn stamp(_path: &Path) -> io::Result<Option<Stamp>> {
    todo!()
}

pub fn load(_path: &Path) -> Result<(String, Option<Stamp>), LoadError> {
    todo!()
}

pub fn save(_path: &Path, _text: &str, _expected: Option<Stamp>, _force: bool) -> io::Result<SaveOutcome> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn saved(outcome: SaveOutcome) -> Stamp {
        match outcome {
            SaveOutcome::Saved(st) => st,
            SaveOutcome::Conflict => panic!("unexpected conflict"),
        }
    }

    #[test]
    fn missing_file_is_empty_document() {
        let dir = tempfile::tempdir().unwrap();
        let (text, st) = load(&dir.path().join("new.md")).unwrap();
        assert_eq!((text.as_str(), st), ("", None));
    }

    #[test]
    fn non_utf8_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bad.md");
        fs::write(&p, [0xff, 0xfe, b'a']).unwrap();
        assert!(matches!(load(&p), Err(LoadError::NotUtf8)));
    }

    #[test]
    fn save_then_load_roundtrips_bytes_exactly() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.md");
        let text = "# Привет\r\n\r\n**мир**\n";
        let st = saved(save(&p, text, None, false).unwrap());
        let (back, st2) = load(&p).unwrap();
        assert_eq!(back, text);
        assert_eq!(st2, Some(st));
        let leftovers: Vec<_> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(leftovers, vec![std::ffi::OsString::from("a.md")]);
    }

    #[test]
    fn save_keeps_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.md");
        fs::write(&p, "x").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
        let st = stamp(&p).unwrap();
        saved(save(&p, "y", st, false).unwrap());
        assert_eq!(fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn saving_through_a_symlink_writes_the_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real.md");
        let link = dir.path().join("link.md");
        fs::write(&target, "old").unwrap();
        symlink(&target, &link).unwrap();
        let p = resolve(&link);
        let st = stamp(&p).unwrap();
        saved(save(&p, "new", st, false).unwrap());
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
    }

    #[test]
    fn external_change_is_a_conflict_unless_forced() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.md");
        let st = saved(save(&p, "mine", None, false).unwrap());
        fs::write(&p, "theirs, longer").unwrap();
        assert_eq!(save(&p, "mine 2", Some(st), false).unwrap(), SaveOutcome::Conflict);
        assert_eq!(fs::read_to_string(&p).unwrap(), "theirs, longer");
        saved(save(&p, "mine 2", Some(st), true).unwrap());
        assert_eq!(fs::read_to_string(&p).unwrap(), "mine 2");
    }

    #[test]
    fn file_appearing_for_a_new_document_is_a_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.md");
        fs::write(&p, "someone else's").unwrap();
        assert_eq!(save(&p, "mine", None, false).unwrap(), SaveOutcome::Conflict);
    }

    #[test]
    fn deleted_file_is_recreated() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.md");
        let st = saved(save(&p, "v1", None, false).unwrap());
        fs::remove_file(&p).unwrap();
        saved(save(&p, "v2", Some(st), false).unwrap());
        assert_eq!(fs::read_to_string(&p).unwrap(), "v2");
    }

    #[test]
    fn unwritable_directory_fails_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.md");
        fs::write(&p, "original").unwrap();
        let st = stamp(&p).unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
        let probe = dir.path().join("probe");
        if fs::write(&probe, "").is_ok() {
            // Running as root: directory permissions are not enforced.
            let _ = fs::remove_file(&probe);
            fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
            return;
        }
        assert!(save(&p, "new", st, false).is_err());
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "original");
        let names: Vec<_> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, vec![std::ffi::OsString::from("a.md")]);
    }

    #[test]
    fn resolve_makes_relative_paths_absolute() {
        let p = resolve(Path::new("surely-missing-opal-file.md"));
        assert!(p.is_absolute());
        assert!(p.ends_with("surely-missing-opal-file.md"));
    }
}
```

Add `pub mod document;` to `src/lib.rs`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test --lib document`
Expected: FAIL — every test panics with `not yet implemented`.

- [ ] **Step 3: Implement** — replace everything above `#[cfg(test)]` in `src/document.rs` with:

```rust
//! Reading and safely writing the document file.

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// What we last saw on disk; a different stamp means someone else wrote the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub modified: Option<SystemTime>,
    pub len: u64,
}

#[derive(Debug)]
pub enum LoadError {
    NotUtf8,
    Io(io::Error),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::NotUtf8 => write!(f, "файл не в кодировке UTF-8"),
            LoadError::Io(e) => write!(f, "{e}"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SaveOutcome {
    Saved(Stamp),
    /// The file changed on disk since `expected`; nothing was written.
    Conflict,
}

/// Absolute path to read and write: symlinks are resolved so that saving
/// replaces the target file, not the link.
pub fn resolve(path: &Path) -> PathBuf {
    if let Ok(p) = fs::canonicalize(path) {
        return p;
    }
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir().map(|d| d.join(path)).unwrap_or_else(|_| path.to_path_buf())
}

/// Stamp of the file, or `None` if it does not exist.
pub fn stamp(path: &Path) -> io::Result<Option<Stamp>> {
    match fs::metadata(path) {
        Ok(m) => Ok(Some(Stamp { modified: m.modified().ok(), len: m.len() })),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Reads the file. A missing file is an empty document (stamp `None`).
pub fn load(path: &Path) -> Result<(String, Option<Stamp>), LoadError> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((String::new(), None)),
        Err(e) => return Err(LoadError::Io(e)),
    };
    let st = stamp(path).map_err(LoadError::Io)?;
    let text = String::from_utf8(bytes).map_err(|_| LoadError::NotUtf8)?;
    Ok((text, st))
}

/// Writes `text` atomically (temp file + rename). Unless `force`, refuses with
/// `Conflict` when the file on disk does not match `expected`. A file that was
/// deleted meanwhile is simply created again.
pub fn save(path: &Path, text: &str, expected: Option<Stamp>, force: bool) -> io::Result<SaveOutcome> {
    let current = stamp(path)?;
    if !force && current.is_some() && current != expected {
        return Ok(SaveOutcome::Conflict);
    }
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = path.file_name().ok_or_else(|| io::Error::other("в пути нет имени файла"))?;
    let tmp = dir.join(format!(".{}.opal-tmp", name.to_string_lossy()));
    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        if current.is_some() {
            fs::set_permissions(&tmp, fs::metadata(path)?.permissions())?;
        }
        fs::rename(&tmp, path)
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    if let Ok(d) = fs::File::open(dir) {
        let _ = d.sync_all();
    }
    let st = stamp(path)?.ok_or_else(|| io::Error::other("файл исчез сразу после записи"))?;
    Ok(SaveOutcome::Saved(st))
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --lib document`
Expected: PASS (10 tests).

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/document.rs
git commit -m "Load and atomically save documents with conflict detection" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Window, autosave and the application

**Files:**
- Create: `src/window.rs`, `src/main.rs`, `tests/gtk_window.rs`
- Modify: `src/lib.rs` (add `pub mod window;` and `run()`), `Cargo.toml` (`[[test]] gtk_window`)

**Interfaces:**
- Consumes: `OpalView::{new, load_text, text, typewriter, set_typewriter}`, `document::*`, `config::{Config, Theme, load}`.
- Produces: `opal::window::build(app: &gtk::Application, config: &Config, path: Option<PathBuf>) -> gtk::ApplicationWindow` (window → `gtk::ScrolledWindow` → `OpalView`); window actions `win.save`, `win.open`, `win.new`, `win.typewriter`, `win.close`; `opal::run() -> glib::ExitCode`; the `opal` binary.

Behaviour (spec 2.5–2.6): autosave 1 s after the last change when the document has a path and autosave is not paused; Ctrl+S saves at once (untitled → «Сохранить как» dialog). Before close / Ctrl+O / Ctrl+N the current document is finished: saved if it has a path; if untitled and non-empty, a «Сохранить / Не сохранять / Отмена» dialog. An external change pauses autosave and asks «Загрузить с диска / Перезаписать». A failed save shows an alert once per series and `• ` in the title. `OPAL_BENCH=startup` quits after the first frame.

- [ ] **Step 1: Write the failing tests** — add to `Cargo.toml`:

```toml
[[test]]
name = "gtk_window"
harness = false
```

and create `tests/gtk_window.rs`:

```rust
//! GTK-level tests of the window: opening, autosave, conflicts.
//! Run: `cargo test --test gtk_window [name-filter]` (needs a display).

mod common;

use std::fs;
use std::path::Path;
use std::time::Duration;

use gtk::gio;
use gtk::prelude::*;
use opal::config::Config;
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
        ("typewriter_action_toggles", typewriter_action_toggles),
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
    window.activate_action("win.save", None).unwrap();
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
    window.activate_action("win.save", None).unwrap();
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

fn typewriter_action_toggles() {
    let (window, view) = open(None);
    let before = view.typewriter();
    window.activate_action("win.typewriter", None).unwrap();
    assert_ne!(view.typewriter(), before);
    window.destroy();
}
```

Run: `cargo test --test gtk_window` — Expected: compile error (`opal::window` missing).

- [ ] **Step 2: Implement the window** — `src/window.rs`:

```rust
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
        settings.set_gtk_application_prefer_dark_theme(config.theme == Theme::Dark);
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
```

- [ ] **Step 3: Add the entry point** — append to `src/lib.rs` (after the module list; also add `pub mod window;` to it):

```rust
use std::path::PathBuf;

use gtk::prelude::*;
use gtk::{gio, glib};

/// Runs the editor: `opal [file]`.
pub fn run() -> glib::ExitCode {
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
```

and create `src/main.rs`:

```rust
fn main() -> gtk::glib::ExitCode {
    opal::run()
}
```

- [ ] **Step 4: Run all tests**

Run: `cargo test --lib && cargo test --test gtk_editor && cargo test --test gtk_window`
Expected: all PASS. (Alert dialogs from the conflict and non-UTF-8 tests appear briefly; that is expected.)

- [ ] **Step 5: Smoke-run the binary**

Run: `printf '# Проба\n\n**жирный** и *курсив*\n' > /tmp/claude-1000/opal-smoke.md && OPAL_BENCH=startup cargo run --release -- /tmp/claude-1000/opal-smoke.md; echo "exit $?"`
Expected: a window flashes and the command exits with `exit 0` (it quits after the first frame).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/lib.rs src/main.rs src/window.rs tests/gtk_window.rs
git commit -m "Add window with autosave, dialogs and the opal binary" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Measurements, README and final verification

**Files:**
- Create: `README.md`, `scripts/measure.py`
- Modify: nothing else unless a measurement fails

**Interfaces:**
- Consumes: the `opal` binary, `OPAL_BENCH=startup`.
- Produces: measured startup time and RSS (reported to the user), README.

- [ ] **Step 1: Write the measurement script** — `scripts/measure.py`:

```python
#!/usr/bin/env python3
"""Startup time (median of 10, to the first frame) and resident memory of opal.

Usage: scripts/measure.py target/release/opal
Linux-only (reads /proc); a development tool, not part of the editor.
"""
import os
import statistics
import subprocess
import sys
import tempfile
import time

binary = sys.argv[1]
block = ("Это **жирный** текст и *курсив*, а ещё `код` и [ссылка](http://x). "
         "Обычные слова, много слов.\n\n> цитата\n> ещё\n\n- пункт\n- [ ] задача\n\n")
with tempfile.NamedTemporaryFile("w", suffix=".md", delete=False) as f:
    text = "# Заголовок\n\n"
    while len(text.encode()) < 100_000:
        text += block
    f.write(text)
    doc = f.name

runs = []
for _ in range(10):
    started = time.perf_counter()
    subprocess.run([binary, doc], env=dict(os.environ, OPAL_BENCH="startup"), check=True)
    runs.append((time.perf_counter() - started) * 1000)
print(f"startup: median {statistics.median(runs):.0f} ms, min {min(runs):.0f} ms (100 KB document)")

proc = subprocess.Popen([binary, doc])
time.sleep(1.5)
with open(f"/proc/{proc.pid}/status") as status:
    rss = next(line for line in status if line.startswith("VmRSS"))
proc.terminate()
proc.wait()
print(f"memory: {rss.split(':')[1].strip()}")
os.unlink(doc)
```

- [ ] **Step 2: Measure**

Run: `cargo build --release && python3 scripts/measure.py target/release/opal`
Expected: startup median ≲ 200 ms, memory ≲ 60 MB. If startup is slower, run again with `GSK_RENDERER=cairo` and with `GSK_RENDERER=ngl` prepended and record all numbers; do not change code for this — report the numbers to the user (spec 3.10).

- [ ] **Step 3: Check portability**

Run: `grep -rnE '/proc|inotify|epoll|libc::|target_os' src/ || echo clean`
Expected: `clean`.

- [ ] **Step 4: Write `README.md`**

````markdown
# opal

Минималистичный Markdown-редактор: разметка видна только у элемента под курсором, колонка текста по центру, typewriter mode, автосохранение. GTK4, без веб-движка.

## Сборка

Debian: `sudo apt install libgtk-4-dev pkg-config` · FreeBSD: `pkg install gtk4 rust pkgconf`

```
cargo build --release
cp target/release/opal ~/.local/bin/
```

## Использование

`opal файл.md` — открыть (если файла нет, он появится при первом сохранении); `opal` — новый документ.

| Клавиши | |
|---|---|
| Ctrl+S | сохранить сейчас (для нового документа — выбрать имя) |
| Ctrl+O / Ctrl+N | открыть / новый документ |
| Ctrl+T | typewriter mode вкл/выкл |
| Ctrl+Q | закрыть |
| Ctrl+клик | открыть ссылку |

Сохранение автоматическое: через секунду после паузы и при закрытии.

## Настройки

`~/.config/opal/config`:

```
column_width = 70        # ширина колонки в символах
font = Serif 14
mono_font = Monospace 12
typewriter = true
theme = light            # light | dark
```

## Тесты

`cargo test --lib` — логика; `cargo test --test gtk_editor` и `cargo test --test gtk_window` открывают окна и требуют дисплей.
````

- [ ] **Step 5: Full verification**

Run: `cargo build --release 2>&1 | grep -c warning; cargo test --lib && cargo test --release --test gtk_editor && cargo test --test gtk_window`
Expected: `0` warnings; all tests PASS.

- [ ] **Step 6: Commit**

```bash
git add README.md scripts/measure.py
git commit -m "Add README and measurement script" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 7: Report to the user** — in Russian, short: what works, measured startup/memory/restyle numbers, anything that deviated from the spec, and this checklist for them to try by hand:

1. `opal ~/заметка.md` — окно появляется сразу.
2. Набрать `**жирный**`, увести курсор — звёздочки исчезают; вернуть курсор стрелками — появляются.
3. Набор длинного текста — строка с курсором остаётся по центру; Ctrl+T выключает и включает режим.
4. Изменить ширину окна — колонка остаётся по центру.
5. Напечатать слово и сразу закрыть окно — изменение сохранено.
6. Ctrl+N, напечатать, закрыть — редактор спрашивает, сохранить ли.
7. Открыть тот же файл в другом редакторе, изменить, вернуться и напечатать — появляется вопрос о конфликте.
8. Ctrl+клик по ссылке открывает браузер; клик по ☐ ставит галочку.
