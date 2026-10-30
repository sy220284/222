//! Vim cursor navigation over a text buffer, ported from
//! `src/utils/Cursor.ts` word/line/find semantics (2.1.88).
//!
//! The original walks Intl grapheme clusters; the Rust line editor is
//! `char`-oriented, so this port navigates Unicode scalar values — visually
//! identical for CJK, close for emoji ZWJ sequences (documented deviation).

use super::types::FindType;

/// Character classification shared with text objects (`isVimWordChar` etc.).
pub fn is_vim_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}
pub fn is_vim_whitespace(ch: char) -> bool {
    ch.is_whitespace()
}
pub fn is_vim_punctuation(ch: char) -> bool {
    !is_vim_whitespace(ch) && !is_vim_word_char(ch)
}

/// Read-only navigation surface over a `&str` at a byte offset (kept on char
/// boundaries by construction).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VimCursor<'a> {
    pub text: &'a str,
    pub offset: usize,
}

fn char_at(text: &str, offset: usize) -> Option<(char, usize)> {
    let rest = text.get(offset..)?;
    rest.chars().next().map(|c| (c, c.len_utf8()))
}

fn prev_boundary(text: &str, offset: usize) -> usize {
    text[..offset]
        .char_indices()
        .next_back()
        .map(|(i, _)| i)
        .unwrap_or(offset)
}

impl<'a> VimCursor<'a> {
    pub fn new(text: &'a str, offset: usize) -> Self {
        let mut offset = offset.min(text.len());
        while !text.is_char_boundary(offset) {
            offset -= 1;
        }
        Self { text, offset }
    }

    pub fn equals(&self, other: &VimCursor<'_>) -> bool {
        self.offset == other.offset
    }

    pub fn is_at_start(&self) -> bool {
        self.offset == 0
    }
    pub fn is_at_end(&self) -> bool {
        self.offset >= self.text.len()
    }
    pub fn is_over_whitespace(&self) -> bool {
        char_at(self.text, self.offset)
            .map(|(c, _)| is_vim_whitespace(c))
            .unwrap_or(false)
    }
    fn is_over_word_char(&self) -> bool {
        char_at(self.text, self.offset)
            .map(|(c, _)| is_vim_word_char(c))
            .unwrap_or(false)
    }

    pub fn left(&self) -> VimCursor<'a> {
        if self.offset == 0 {
            return self.clone();
        }
        VimCursor::new(self.text, prev_boundary(self.text, self.offset))
    }
    pub fn right(&self) -> VimCursor<'a> {
        match char_at(self.text, self.offset) {
            Some((_, len)) => VimCursor::new(self.text, self.offset + len),
            None => self.clone(),
        }
    }
    /// Advance `count` characters forward without word semantics.
    fn advance(&self, count: usize) -> VimCursor<'a> {
        let mut cursor = self.clone();
        for _ in 0..count {
            cursor = cursor.right();
        }
        cursor
    }

    // ----- logical lines -------------------------------------------------
    fn line_bounds(&self) -> (usize, usize) {
        let start = self.text[..self.offset]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let end = self.text[self.offset..]
            .find('\n')
            .map(|i| self.offset + i)
            .unwrap_or(self.text.len());
        (start, end)
    }
    pub fn start_of_logical_line(&self) -> VimCursor<'a> {
        VimCursor::new(self.text, self.line_bounds().0)
    }
    pub fn end_of_logical_line(&self) -> VimCursor<'a> {
        VimCursor::new(self.text, self.line_bounds().1)
    }
    pub fn first_non_blank_in_logical_line(&self) -> VimCursor<'a> {
        let (start, end) = self.line_bounds();
        let line = &self.text[start..end];
        let col = line
            .char_indices()
            .find(|(_, c)| !c.is_whitespace())
            .map(|(i, _)| i)
            .unwrap_or(0);
        VimCursor::new(self.text, start + col)
    }
    pub fn line_index(&self) -> usize {
        self.text[..self.offset]
            .bytes()
            .filter(|b| *b == b'\n')
            .count()
    }
    /// Preserve the current column while moving one logical line up/down,
    /// clamped to the target line length (Cursor.upLogicalLine/downLogicalLine).
    fn vertical(&self, down: bool) -> VimCursor<'a> {
        let (start, end) = self.line_bounds();
        let column = self.text[start..self.offset].chars().count();
        let (other_start, other_end) = if down {
            if end >= self.text.len() {
                return self.clone();
            }
            let next_start = end + 1;
            let next_end = self.text[next_start..]
                .find('\n')
                .map(|i| next_start + i)
                .unwrap_or(self.text.len());
            (next_start, next_end)
        } else {
            if start == 0 {
                return self.clone();
            }
            let prev_start = self.text[..start - 1]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            (prev_start, start - 1)
        };
        let clamped = self.text[other_start..other_end]
            .chars()
            .take(column)
            .map(char::len_utf8)
            .sum::<usize>();
        VimCursor::new(self.text, other_start + clamped)
    }
    pub fn up_logical_line(&self) -> VimCursor<'a> {
        self.vertical(false)
    }
    pub fn down_logical_line(&self) -> VimCursor<'a> {
        self.vertical(true)
    }
    pub fn start_of_first_line(&self) -> VimCursor<'a> {
        VimCursor::new(self.text, 0)
    }
    pub fn start_of_last_line(&self) -> VimCursor<'a> {
        let offset = self.text.rfind('\n').map(|i| i + 1).unwrap_or(0);
        VimCursor::new(self.text, offset)
    }
    /// Move to column 0 of line `line_number` (1-based, clamped; goToLine).
    pub fn go_to_line(&self, line_number: usize) -> VimCursor<'a> {
        let total = self.text.bytes().filter(|b| *b == b'\n').count() + 1;
        let target = line_number.clamp(1, total) - 1;
        let mut offset = 0;
        for _ in 0..target {
            match self.text[offset..].find('\n') {
                Some(i) => offset += i + 1,
                None => break,
            }
        }
        VimCursor::new(self.text, offset)
    }

    // ----- vim word motions ----------------------------------------------
    fn class(c: Option<char>) -> u8 {
        match c {
            Some(c) if is_vim_whitespace(c) => 0,
            Some(c) if is_vim_word_char(c) => 1,
            Some(_) => 2,
            None => 3,
        }
    }
    fn is_word(&self) -> bool {
        matches!(
            Self::class(char_at(self.text, self.offset).map(|(ch, _)| ch)),
            1 | 2
        )
    }
    /// `w` — start of next vim word (words are word/punct runs; whitespace skipped).
    pub fn next_vim_word(&self) -> VimCursor<'a> {
        let mut cursor = self.clone();
        if cursor.is_at_end() {
            return cursor;
        }
        let start_class = Self::class(char_at(cursor.text, cursor.offset).map(|(ch, _)| ch));
        if start_class != 0 {
            let class = start_class;
            while !cursor.is_at_end()
                && Self::class(char_at(cursor.text, cursor.offset).map(|(ch, _)| ch)) == class
            {
                cursor = cursor.right();
            }
        }
        while !cursor.is_at_end()
            && Self::class(char_at(cursor.text, cursor.offset).map(|(ch, _)| ch)) == 0
        {
            cursor = cursor.right();
        }
        cursor
    }
    /// `e` — end of current/next vim word.
    pub fn end_of_vim_word(&self) -> VimCursor<'a> {
        let mut cursor = self.clone();
        if !cursor.is_at_end() {
            cursor = cursor.right();
        }
        while !cursor.is_at_end() && !cursor.is_word() {
            cursor = cursor.right();
        }
        if cursor.is_at_end() {
            return VimCursor::new(cursor.text, cursor.text.len());
        }
        while !cursor.is_at_end() {
            let next = cursor.right();
            if next.is_at_end() || !next.is_word() {
                break;
            }
            cursor = next;
        }
        cursor
    }
    /// `b` — start of previous vim word.
    pub fn prev_vim_word(&self) -> VimCursor<'a> {
        let mut cursor = self.clone();
        cursor = cursor.left();
        while !cursor.is_at_start() && !cursor.is_word() {
            cursor = cursor.left();
        }
        while !cursor.is_at_start() {
            let prev = cursor.left();
            if !prev.is_word() {
                break;
            }
            cursor = prev;
        }
        cursor
    }
    fn class_word(c: Option<char>) -> u8 {
        match c {
            Some(c) if is_vim_whitespace(c) => 0,
            Some(_) => 1,
            None => 2,
        }
    }
    fn is_word_big(&self) -> bool {
        matches!(
            Self::class_word(char_at(self.text, self.offset).map(|(ch, _)| ch)),
            1
        )
    }
    /// `W` — start of next WORD (runs of non-whitespace).
    pub fn next_word_big(&self) -> VimCursor<'a> {
        let mut cursor = self.clone();
        if cursor.is_at_end() {
            return cursor;
        }
        let start_class = Self::class_word(char_at(cursor.text, cursor.offset).map(|(ch, _)| ch));
        if start_class != 0 {
            while !cursor.is_at_end()
                && Self::class_word(char_at(cursor.text, cursor.offset).map(|(ch, _)| ch))
                    == start_class
            {
                cursor = cursor.right();
            }
        }
        while !cursor.is_at_end()
            && Self::class_word(char_at(cursor.text, cursor.offset).map(|(ch, _)| ch)) == 0
        {
            cursor = cursor.right();
        }
        cursor
    }
    /// `E` — end of current/next WORD.
    pub fn end_of_word_big(&self) -> VimCursor<'a> {
        let mut cursor = self.clone();
        if !cursor.is_at_end() {
            cursor = cursor.right();
        }
        while !cursor.is_at_end() && !cursor.is_word_big() {
            cursor = cursor.right();
        }
        if cursor.is_at_end() {
            return VimCursor::new(cursor.text, cursor.text.len());
        }
        while !cursor.is_at_end() {
            let next = cursor.right();
            if next.is_at_end() || !next.is_word_big() {
                break;
            }
            cursor = next;
        }
        cursor
    }
    /// `B` — start of previous WORD.
    pub fn prev_word_big(&self) -> VimCursor<'a> {
        let mut cursor = self.clone();
        cursor = cursor.left();
        while !cursor.is_at_start() && !cursor.is_word_big() {
            cursor = cursor.left();
        }
        while !cursor.is_at_start() {
            let prev = cursor.left();
            if !prev.is_word_big() {
                break;
            }
            cursor = prev;
        }
        cursor
    }

    // ----- f/F/t/T --------------------------------------------------------
    /// Vim find semantics with occurrence `count`; `t`/`T` stop one char
    /// short of the target.
    pub fn find_character(&self, ch: char, find: FindType, count: usize) -> Option<usize> {
        let forward = matches!(find, FindType::FForward | FindType::TTill);
        let till = matches!(find, FindType::TTill | FindType::TBackTill);
        let count = count.max(1);
        let mut found = 0;
        if forward {
            let mut pos = match char_at(self.text, self.offset) {
                Some((_, len)) => self.offset + len,
                None => return None,
            };
            while pos < self.text.len() {
                let (c, len) = char_at(self.text, pos)?;
                if c == ch {
                    found += 1;
                    if found == count {
                        return Some(if till {
                            self.offset.max(prev_boundary(self.text, pos))
                        } else {
                            pos
                        });
                    }
                }
                pos += len;
            }
        } else {
            let mut pos = prev_boundary(self.text, self.offset);
            loop {
                let (c, _) = char_at(self.text, pos)?;
                if c == ch {
                    found += 1;
                    if found == count {
                        return Some(if till {
                            self.offset.min(match char_at(self.text, pos) {
                                Some((_, len)) => pos + len,
                                None => pos,
                            })
                        } else {
                            pos
                        });
                    }
                }
                if pos == 0 {
                    return None;
                }
                pos = prev_boundary(self.text, pos);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cursor(text: &str, offset: usize) -> VimCursor<'_> {
        VimCursor::new(text, offset)
    }
    #[test]
    fn word_motions_follow_vim_semantics() {
        let text = "foo bar-baz qux";
        assert_eq!(cursor(text, 0).next_vim_word().offset, 4);
        assert_eq!(cursor(text, 4).next_vim_word().offset, 8);
        // b goes back to word start
        assert_eq!(cursor(text, 5).prev_vim_word().offset, 4);
        assert_eq!(cursor(text, 8).prev_vim_word().offset, 4);
        // e lands on last char of the word
        assert_eq!(cursor(text, 4).end_of_vim_word().offset, 6);
        assert_eq!(cursor(text, 4).end_of_vim_word().right().offset, 8);
        // CJK counts as word characters
        let cjk = "你好 world";
        assert_eq!(cursor(cjk, 0).next_vim_word().offset, 7);
    }
    #[test]
    fn word_big_ignores_punctuation() {
        let text = "foo, bar baz";
        assert_eq!(cursor(text, 0).next_word_big().offset, 5);
        assert_eq!(cursor(text, 5).prev_word_big().offset, 0);
        assert_eq!(cursor(text, 0).end_of_word_big().offset, 3);
    }
    #[test]
    fn line_motions_preserve_column() {
        let text = "alpha\nbeta\ngamma";
        let c = cursor(text, 0).down_logical_line();
        assert_eq!(c.offset, 6);
        let c = cursor(text, 2).down_logical_line();
        assert_eq!(c.offset, 8);
        assert_eq!(cursor(text, 8).up_logical_line().offset, 2);
        assert_eq!(cursor(text, 8).end_of_logical_line().offset, 10);
        assert_eq!(cursor(text, 8).start_of_logical_line().offset, 6);
        assert_eq!(cursor(text, 6).first_non_blank_in_logical_line().offset, 6);
        assert_eq!(cursor(text, 6).go_to_line(3).offset, 11);
        assert_eq!(cursor(text, 6).go_to_line(99).offset, 11);
        assert_eq!(cursor(text, 6).start_of_last_line().offset, 11);
    }
    #[test]
    fn find_character_supports_f_t_and_counts() {
        let text = "a1b2c3d4";
        assert_eq!(
            cursor(text, 0).find_character('3', FindType::FForward, 1),
            Some(6)
        );
        assert_eq!(
            cursor(text, 6).find_character('a', FindType::FBackward, 1),
            Some(0)
        );
        // t stops one before
        assert_eq!(
            cursor(text, 0).find_character('3', FindType::TTill, 1),
            Some(5)
        );
        // T stops one after (moving backwards)
        assert_eq!(
            cursor(text, 6).find_character('a', FindType::TBackTill, 1),
            Some(1)
        );
        assert_eq!(
            cursor(text, 0).find_character('z', FindType::FForward, 1),
            None
        );
    }
}
