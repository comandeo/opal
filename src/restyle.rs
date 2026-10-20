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
