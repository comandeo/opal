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
                let r = if matches!(kind, Kind::ListItem { .. }) { item_start(text, &r)..r.end } else { r };
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

/// pulldown-cmark starts a tab-indented nested list item at the line break
/// before it; the item really starts at its bullet or number.
fn item_start(text: &str, r: &Range<usize>) -> usize {
    let skip = text.as_bytes()[r.clone()].iter().take_while(|&&b| matches!(b, b'\r' | b'\n' | b' ' | b'\t'));
    (r.start + skip.count()).min(r.end)
}

fn bullet_marker(text: &str, r: &Range<usize>) -> Marker {
    let le = line_end(text, r.start);
    let after = (r.start + 1).min(le);
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
    fn tab_indented_nested_items() {
        // pulldown-cmark starts these items at the line break before them.
        assert_eq!(
            view("- a\n\t- b"),
            vec![
                (Kind::ListItem { ordered: false }, "- a\n\t- b", vec!["- "]),
                (Kind::ListItem { ordered: false }, "- b", vec!["- "]),
            ]
        );
        assert_eq!(view("* a\n\t* b")[1], (Kind::ListItem { ordered: false }, "* b", vec!["* "]));
        assert_eq!(view("- a\n\t- [ ] b")[1], (Kind::Task { checked: false }, "- [ ] b", vec!["- [ ] "]));
        assert_eq!(scopes("- a\n\t- b")[1], vec!["- b"]);
        assert!(parse("*\n\t+").iter().all(|e| !e.markers.is_empty()));
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
