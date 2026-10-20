//! Typographic replacements while typing Russian prose: `--` → `—`,
//! `"` → «ёлочки», with „лапки“ inside them.

/// Replace the last `len` bytes before the cursor with `with`.
#[derive(Debug, PartialEq, Eq)]
pub struct Replace {
    pub len: usize,
    pub with: &'static str,
}

/// Called after a char was typed; `before` is the text up to the cursor,
/// ending with that char. Finished code spans and code blocks are the caller's
/// business; a code span still being typed on this line is recognised here.
pub fn after_typing(before: &str) -> Option<Replace> {
    let line = &before[before.rfind('\n').map_or(0, |i| i + 1)..];
    if line.matches('`').count() % 2 == 1 {
        return None;
    }
    if before.ends_with("--") && !before.ends_with("---") {
        return Some(Replace { len: 2, with: "—" });
    }
    let quoted = before.strip_suffix('"')?;
    let paragraph = &quoted[paragraph_start(quoted)..];
    // Innermost quote still open in this paragraph.
    let mut open = Vec::new();
    for c in paragraph.chars() {
        match c {
            '«' | '„' => open.push(c),
            '»' | '“' => {
                open.pop();
            }
            _ => {}
        }
    }
    let inner = open.last() == Some(&'«');
    let opening = quoted.chars().next_back().is_none_or(|c| c.is_whitespace() || "([{<«„—–-/".contains(c));
    let with = match (opening, inner) {
        (true, false) => "«",
        (true, true) => "„",
        (false, _) if open.last() == Some(&'„') => "“",
        (false, _) => "»",
    };
    Some(Replace { len: 1, with })
}

/// Start of the paragraph that ends `text`: after the last blank line.
fn paragraph_start(text: &str) -> usize {
    let lf = text.rfind("\n\n").map(|i| i + 2);
    let crlf = text.rfind("\n\r\n").map(|i| i + 3);
    lf.max(crlf).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(before: &str) -> Option<(usize, &'static str)> {
        after_typing(before).map(|r| (r.len, r.with))
    }

    #[test]
    fn double_hyphen_becomes_em_dash() {
        assert_eq!(typed("слово --"), Some((2, "—")));
        assert_eq!(typed("--"), Some((2, "—")));
        assert_eq!(typed("слово--"), Some((2, "—")));
        assert_eq!(typed("из-за"), None);
        assert_eq!(typed("—-"), None);
    }

    #[test]
    fn third_hyphen_keeps_a_restored_double_hyphen() {
        // After Ctrl+Z brought `--` back, a third `-` makes a `---` rule.
        assert_eq!(typed("---"), None);
        assert_eq!(typed("----"), None);
    }

    #[test]
    fn quotes_open_at_the_start_and_after_spaces() {
        assert_eq!(typed("\""), Some((1, "«")));
        assert_eq!(typed("он сказал \""), Some((1, "«")));
        assert_eq!(typed("строка\n\""), Some((1, "«")));
        assert_eq!(typed("(\""), Some((1, "«")));
        assert_eq!(typed("— \""), Some((1, "«")));
    }

    #[test]
    fn quotes_close_after_text() {
        assert_eq!(typed("он сказал «да\""), Some((1, "»")));
        assert_eq!(typed("«да!\""), Some((1, "»")));
        assert_eq!(typed("да\""), Some((1, "»")));
    }

    #[test]
    fn quotes_inside_quotes_are_lapki() {
        assert_eq!(typed("«он сказал \""), Some((1, "„")));
        assert_eq!(typed("«он сказал „да\""), Some((1, "“")));
        assert_eq!(typed("«он сказал „да“\""), Some((1, "»")));
        // Right after an opening quote another one opens the next level.
        assert_eq!(typed("«„\""), Some((1, "«")));
    }

    #[test]
    fn open_quotes_of_an_earlier_paragraph_do_not_count() {
        assert_eq!(typed("«не закрыта\n\nновый \""), Some((1, "«")));
        assert_eq!(typed("«не закрыта\r\n\r\nновый \""), Some((1, "«")));
        // A single line break stays inside the paragraph.
        assert_eq!(typed("«первая строка\nвторая \""), Some((1, "„")));
    }

    #[test]
    fn code_span_being_typed_is_left_alone() {
        assert_eq!(typed("запусти `ls --"), None);
        assert_eq!(typed("echo `\""), None);
        assert_eq!(typed("`a` и --"), Some((2, "—")));
    }

    #[test]
    fn other_text_is_left_alone() {
        assert_eq!(typed(""), None);
        assert_eq!(typed("слово"), None);
        assert_eq!(typed("-"), None);
    }
}
