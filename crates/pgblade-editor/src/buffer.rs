use ropey::Rope;

use crate::selection::Position;

/// A text buffer backed by a rope data structure.
///
/// Provides O(log n) insert/delete operations and efficient line-based access.
/// Positions use 0-based line and column numbers.
pub struct EditBuffer {
    rope: Rope,
}

impl Default for EditBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl EditBuffer {
    pub fn new() -> Self {
        Self { rope: Rope::new() }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
        }
    }

    // --- Text accessors ---

    /// Total number of characters in the buffer.
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Total number of bytes in the buffer.
    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    /// Number of lines in the buffer (at least 1).
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Get the text of a specific line (0-based, without trailing newline).
    pub fn line(&self, line_idx: usize) -> String {
        if line_idx >= self.rope.len_lines() {
            return String::new();
        }
        let line = self.rope.line(line_idx);
        let mut s = line.to_string();
        // Remove trailing newline if present
        if s.ends_with('\n') {
            s.pop();
        }
        s
    }

    /// Get the length of a specific line in characters (without newline).
    pub fn line_len(&self, line_idx: usize) -> usize {
        if line_idx >= self.rope.len_lines() {
            return 0;
        }
        let line = self.rope.line(line_idx);
        let len = line.len_chars();
        // Subtract newline if present
        if len > 0 && line.char(len - 1) == '\n' {
            len - 1
        } else {
            len
        }
    }

    /// Get the full text content.
    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    /// Get a slice of text between two char offsets.
    pub fn slice(&self, start: usize, end: usize) -> String {
        self.rope.slice(start..end).to_string()
    }

    // --- Position conversion ---

    /// Convert a Position (line, column) to a char offset.
    /// Clamps to valid bounds.
    pub fn pos_to_char(&self, pos: Position) -> usize {
        let line = pos.line.min(self.rope.len_lines().saturating_sub(1));
        let line_start = self.rope.line_to_char(line);
        let line_len = self.line_len(line);
        let col = pos.column.min(line_len);
        line_start + col
    }

    /// Convert a char offset to a Position (line, column).
    pub fn char_to_pos(&self, char_idx: usize) -> Position {
        let char_idx = char_idx.min(self.rope.len_chars());
        let line = self.rope.char_to_line(char_idx);
        let line_start = self.rope.line_to_char(line);
        Position {
            line,
            column: char_idx - line_start,
        }
    }

    // --- Edit operations ---

    /// Insert text at a char offset. Returns the edit operation for undo tracking.
    pub fn insert(&mut self, char_offset: usize, text: &str) -> EditOp {
        let offset = char_offset.min(self.rope.len_chars());
        self.rope.insert(offset, text);
        EditOp {
            offset,
            old_text: String::new(),
            new_text: text.to_string(),
        }
    }

    /// Insert text at a Position.
    pub fn insert_at(&mut self, pos: Position, text: &str) -> EditOp {
        let offset = self.pos_to_char(pos);
        self.insert(offset, text)
    }

    /// Delete a range of characters. Returns the deleted text as an edit operation.
    pub fn delete(&mut self, start: usize, end: usize) -> EditOp {
        let start = start.min(self.rope.len_chars());
        let end = end.min(self.rope.len_chars());
        if start >= end {
            return EditOp {
                offset: start,
                old_text: String::new(),
                new_text: String::new(),
            };
        }
        let old_text = self.rope.slice(start..end).to_string();
        self.rope.remove(start..end);
        EditOp {
            offset: start,
            old_text,
            new_text: String::new(),
        }
    }

    /// Delete a range defined by two Positions.
    pub fn delete_range(&mut self, from: Position, to: Position) -> EditOp {
        let start = self.pos_to_char(from);
        let end = self.pos_to_char(to);
        if start <= end {
            self.delete(start, end)
        } else {
            self.delete(end, start)
        }
    }

    /// Replace text in a range. Returns the old text as an edit operation.
    pub fn replace(&mut self, start: usize, end: usize, text: &str) -> EditOp {
        let start = start.min(self.rope.len_chars());
        let end = end.min(self.rope.len_chars());
        let old_text = if start < end {
            let s = self.rope.slice(start..end).to_string();
            self.rope.remove(start..end);
            s
        } else {
            String::new()
        };
        self.rope.insert(start, text);
        EditOp {
            offset: start,
            old_text,
            new_text: text.to_string(),
        }
    }

    // --- Word boundary helpers ---

    /// Find the start of the word at or before the given char offset.
    pub fn word_start(&self, char_offset: usize) -> usize {
        if char_offset == 0 {
            return 0;
        }
        let mut idx = char_offset;
        // Skip whitespace backward
        while idx > 0 {
            let ch = self.rope.char(idx - 1);
            if !ch.is_whitespace() {
                break;
            }
            idx -= 1;
        }
        // Skip word chars backward
        while idx > 0 {
            let ch = self.rope.char(idx - 1);
            if !ch.is_alphanumeric() && ch != '_' {
                break;
            }
            idx -= 1;
        }
        idx
    }

    /// Find the end of the word at or after the given char offset.
    pub fn word_end(&self, char_offset: usize) -> usize {
        let len = self.rope.len_chars();
        if char_offset >= len {
            return len;
        }
        let mut idx = char_offset;
        // Skip word chars forward
        while idx < len {
            let ch = self.rope.char(idx);
            if !ch.is_alphanumeric() && ch != '_' {
                break;
            }
            idx += 1;
        }
        // Skip whitespace forward
        while idx < len {
            let ch = self.rope.char(idx);
            if !ch.is_whitespace() {
                break;
            }
            idx += 1;
        }
        idx
    }
}

/// A single edit operation for undo/redo tracking.
#[derive(Debug, Clone, PartialEq)]
pub struct EditOp {
    pub offset: usize,
    pub old_text: String,
    pub new_text: String,
}

impl EditOp {
    pub fn is_empty(&self) -> bool {
        self.old_text.is_empty() && self.new_text.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_buffer_is_empty() {
        let buf = EditBuffer::new();
        assert!(buf.is_empty());
        assert_eq!(buf.len_chars(), 0);
        assert_eq!(buf.len_bytes(), 0);
    }

    #[test]
    fn from_str_creates_content() {
        let buf = EditBuffer::from_str("hello world");
        assert!(!buf.is_empty());
        assert_eq!(buf.len_chars(), 11);
        assert_eq!(buf.text(), "hello world");
    }

    #[test]
    fn line_count() {
        let buf = EditBuffer::from_str("line1\nline2\nline3");
        assert_eq!(buf.line_count(), 3);

        let buf_trailing = EditBuffer::from_str("line1\nline2\n");
        // ropey counts the empty line after the trailing newline
        assert_eq!(buf_trailing.line_count(), 3);
    }

    #[test]
    fn line_content() {
        let buf = EditBuffer::from_str("hello\nworld\nfoo");
        assert_eq!(buf.line(0), "hello");
        assert_eq!(buf.line(1), "world");
        assert_eq!(buf.line(2), "foo");
        // Out of bounds returns empty
        assert_eq!(buf.line(99), "");
    }

    #[test]
    fn line_len() {
        let buf = EditBuffer::from_str("hello\nworld\n");
        assert_eq!(buf.line_len(0), 5); // "hello"
        assert_eq!(buf.line_len(1), 5); // "world"
        assert_eq!(buf.line_len(2), 0); // empty line after trailing newline
        assert_eq!(buf.line_len(99), 0); // out of bounds
    }

    #[test]
    fn pos_to_char_and_back() {
        let buf = EditBuffer::from_str("hello\nworld\nfoo");
        // (0, 0) -> 0
        let offset = buf.pos_to_char(Position::new(0, 0));
        assert_eq!(offset, 0);
        assert_eq!(buf.char_to_pos(offset), Position::new(0, 0));

        // (1, 3) -> 6 + 3 = 9 ("wor|ld")
        let offset = buf.pos_to_char(Position::new(1, 3));
        assert_eq!(offset, 9);
        assert_eq!(buf.char_to_pos(offset), Position::new(1, 3));

        // (2, 2) -> 12 + 2 = 14 ("fo|o")
        let offset = buf.pos_to_char(Position::new(2, 2));
        assert_eq!(offset, 14);
        assert_eq!(buf.char_to_pos(offset), Position::new(2, 2));
    }

    #[test]
    fn insert_at_beginning() {
        let mut buf = EditBuffer::from_str("world");
        let op = buf.insert(0, "hello ");
        assert_eq!(buf.text(), "hello world");
        assert_eq!(op.offset, 0);
        assert_eq!(op.new_text, "hello ");
        assert!(op.old_text.is_empty());
    }

    #[test]
    fn insert_in_middle() {
        let mut buf = EditBuffer::from_str("helo");
        let op = buf.insert(3, "l");
        assert_eq!(buf.text(), "hello");
        assert_eq!(op.offset, 3);
    }

    #[test]
    fn insert_at_end() {
        let mut buf = EditBuffer::from_str("hello");
        let op = buf.insert(100, " world"); // offset is clamped
        assert_eq!(buf.text(), "hello world");
        assert_eq!(op.offset, 5); // clamped to len_chars
    }

    #[test]
    fn delete_range() {
        let mut buf = EditBuffer::from_str("hello world");
        let op = buf.delete(5, 11);
        assert_eq!(buf.text(), "hello");
        assert_eq!(op.old_text, " world");
        assert!(op.new_text.is_empty());
    }

    #[test]
    fn delete_entire_line() {
        let mut buf = EditBuffer::from_str("line1\nline2\nline3");
        // Delete "line2\n" (chars 6..12)
        let op = buf.delete(6, 12);
        assert_eq!(buf.text(), "line1\nline3");
        assert_eq!(op.old_text, "line2\n");
    }

    #[test]
    fn replace_text() {
        let mut buf = EditBuffer::from_str("hello world");
        let op = buf.replace(6, 11, "rust");
        assert_eq!(buf.text(), "hello rust");
        assert_eq!(op.old_text, "world");
        assert_eq!(op.new_text, "rust");
    }

    #[test]
    fn word_start_and_end() {
        let buf = EditBuffer::from_str("hello world foo_bar");
        // word_start from middle of "hello" (offset 3)
        assert_eq!(buf.word_start(3), 0);
        // word_end from middle of "hello" (offset 3)
        assert_eq!(buf.word_end(3), 6); // end of "hello" + space

        // word_start from middle of "world" (offset 8)
        assert_eq!(buf.word_start(8), 6);
        // word_end from beginning of "world" (offset 6)
        assert_eq!(buf.word_end(6), 12); // end of "world" + space

        // word_start from middle of "foo_bar" (offset 15)
        assert_eq!(buf.word_start(15), 12);
        // word_end from beginning of "foo_bar" (offset 12)
        assert_eq!(buf.word_end(12), 19); // end of "foo_bar"
    }

    #[test]
    fn pos_clamps_to_bounds() {
        let buf = EditBuffer::from_str("hi\nthere");
        // Line out of bounds, column also out of bounds -> clamped to end of last line
        let offset = buf.pos_to_char(Position::new(100, 100));
        assert_eq!(buf.char_to_pos(offset), Position::new(1, 5)); // clamped to end of "there"

        // Line out of bounds, column 0 -> clamped to start of last line
        let offset = buf.pos_to_char(Position::new(100, 0));
        assert_eq!(buf.char_to_pos(offset), Position::new(1, 0));

        // Column out of bounds
        let offset = buf.pos_to_char(Position::new(0, 100));
        assert_eq!(buf.char_to_pos(offset), Position::new(0, 2)); // clamped to end of "hi"
    }

    #[test]
    fn insert_at_position() {
        let mut buf = EditBuffer::from_str("hello\nworld");
        let op = buf.insert_at(Position::new(1, 0), "cruel ");
        assert_eq!(buf.text(), "hello\ncruel world");
        assert_eq!(op.offset, 6);
    }

    #[test]
    fn delete_range_by_positions() {
        let mut buf = EditBuffer::from_str("hello\nworld");
        let op = buf.delete_range(Position::new(0, 3), Position::new(1, 2));
        assert_eq!(buf.text(), "helrld");
        assert_eq!(op.old_text, "lo\nwo");
    }

    #[test]
    fn edit_op_is_empty() {
        let empty_op = EditOp {
            offset: 0,
            old_text: String::new(),
            new_text: String::new(),
        };
        assert!(empty_op.is_empty());

        let insert_op = EditOp {
            offset: 0,
            old_text: String::new(),
            new_text: "x".to_string(),
        };
        assert!(!insert_op.is_empty());
    }

    #[test]
    fn slice_text() {
        let buf = EditBuffer::from_str("hello world");
        assert_eq!(buf.slice(0, 5), "hello");
        assert_eq!(buf.slice(6, 11), "world");
    }
}
