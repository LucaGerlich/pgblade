use pgblade_core::highlighter::{TokenKind, highlight_sql};

use crate::buffer::EditBuffer;

/// A single display line with its content and highlight information.
#[derive(Debug, Clone)]
pub struct DisplayLine {
    pub line_number: usize, // 1-based
    pub text: String,
    pub highlights: Vec<LineHighlight>,
}

/// A highlight within a single display line (byte offsets relative to line start).
#[derive(Debug, Clone)]
pub struct LineHighlight {
    pub start: usize,
    pub end: usize,
    pub kind: TokenKind,
}

/// Compute display lines for the given range of the buffer.
/// Applies syntax highlighting from the full text for correctness.
pub fn compute_display_lines(
    buffer: &EditBuffer,
    first_line: usize,
    line_count: usize,
) -> Vec<DisplayLine> {
    let full_text = buffer.text();
    let highlights = highlight_sql(&full_text);
    let total_lines = buffer.line_count();

    let end_line = (first_line + line_count).min(total_lines);

    // Compute byte offset of each line start
    let mut line_byte_offsets: Vec<usize> = Vec::with_capacity(total_lines);
    let mut offset = 0;
    for line_idx in 0..total_lines {
        line_byte_offsets.push(offset);
        let line_text = buffer.line(line_idx);
        offset += line_text.len() + 1; // +1 for \n
    }

    (first_line..end_line)
        .map(|line_idx| {
            let line_text = buffer.line(line_idx);
            let line_byte_start = line_byte_offsets.get(line_idx).copied().unwrap_or(0);
            let line_byte_end = line_byte_start + line_text.len();

            // Filter highlights that overlap this line
            let line_highlights: Vec<LineHighlight> = highlights
                .iter()
                .filter(|h| h.end > line_byte_start && h.start < line_byte_end)
                .map(|h| LineHighlight {
                    start: h.start.saturating_sub(line_byte_start),
                    end: (h.end - line_byte_start).min(line_text.len()),
                    kind: h.kind,
                })
                .collect();

            DisplayLine {
                line_number: line_idx + 1,
                text: line_text,
                highlights: line_highlights,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer_returns_no_lines() {
        let buffer = EditBuffer::new();
        let lines = compute_display_lines(&buffer, 0, 10);
        // An empty buffer has 1 line (the empty line) in ropey
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "");
    }

    #[test]
    fn single_line_produces_one_display_line() {
        let buffer = EditBuffer::from_str("SELECT 1");
        let lines = compute_display_lines(&buffer, 0, 10);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].line_number, 1);
        assert_eq!(lines[0].text, "SELECT 1");
        assert!(!lines[0].highlights.is_empty());
    }

    #[test]
    fn multiline_buffer_with_offset() {
        let buffer = EditBuffer::from_str("line1\nline2\nline3\nline4");
        let lines = compute_display_lines(&buffer, 1, 2);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].line_number, 2);
        assert_eq!(lines[0].text, "line2");
        assert_eq!(lines[1].line_number, 3);
        assert_eq!(lines[1].text, "line3");
    }

    #[test]
    fn line_count_clamped_to_total() {
        let buffer = EditBuffer::from_str("a\nb");
        let lines = compute_display_lines(&buffer, 0, 100);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn highlights_are_relative_to_line() {
        let buffer = EditBuffer::from_str("SELECT 1\nFROM t");
        let lines = compute_display_lines(&buffer, 0, 10);
        // Second line "FROM t" - FROM starts at byte 0 of line
        let from_highlights: Vec<_> = lines[1]
            .highlights
            .iter()
            .filter(|h| h.kind == TokenKind::Keyword)
            .collect();
        assert!(!from_highlights.is_empty());
        assert_eq!(from_highlights[0].start, 0);
        assert_eq!(from_highlights[0].end, 4);
    }
}
