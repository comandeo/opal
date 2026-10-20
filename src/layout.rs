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
