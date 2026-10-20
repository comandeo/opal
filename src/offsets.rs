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
