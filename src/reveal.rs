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
