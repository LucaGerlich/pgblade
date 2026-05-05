/// A position in the document as line + column (both 0-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// Tracks the desired column for vertical cursor movement.
/// When moving up/down, the cursor should try to stay at the original column
/// even if intermediate lines are shorter.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SelectionGoal {
    #[default]
    None,
    Column(usize),
}

/// A selection range in the document.
///
/// `start` is always <= `end` in document order.
/// `reversed` indicates whether the cursor (head) is at `start` or `end`:
/// - `reversed = false`: head at `end`, tail at `start` (forward selection)
/// - `reversed = true`: head at `start`, tail at `end` (backward selection)
///
/// When `start == end`, this is a cursor with no selected text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Selection {
    pub start: Position,
    pub end: Position,
    pub reversed: bool,
    pub goal: SelectionGoal,
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            start: Position::default(),
            end: Position::default(),
            reversed: false,
            goal: SelectionGoal::None,
        }
    }
}

impl Selection {
    /// Create a cursor (empty selection) at the given position.
    pub fn cursor(pos: Position) -> Self {
        Self {
            start: pos,
            end: pos,
            reversed: false,
            goal: SelectionGoal::None,
        }
    }

    /// Create a selection from start to end.
    /// Normalizes so start <= end.
    pub fn range(from: Position, to: Position) -> Self {
        if from <= to {
            Self {
                start: from,
                end: to,
                reversed: false,
                goal: SelectionGoal::None,
            }
        } else {
            Self {
                start: to,
                end: from,
                reversed: true,
                goal: SelectionGoal::None,
            }
        }
    }

    /// The position where the cursor is displayed (the "moving" end).
    pub fn head(&self) -> Position {
        if self.reversed { self.start } else { self.end }
    }

    /// The anchored end of the selection (where the selection started).
    pub fn tail(&self) -> Position {
        if self.reversed { self.end } else { self.start }
    }

    /// Whether this is a cursor (no selected text).
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Collapse selection to a cursor at the given position.
    pub fn collapse_to(&mut self, pos: Position) {
        self.start = pos;
        self.end = pos;
        self.reversed = false;
        self.goal = SelectionGoal::None;
    }

    /// Move the head of the selection to a new position.
    /// Updates start/end and reversed flag to maintain the invariant start <= end.
    pub fn set_head(&mut self, head: Position) {
        let tail = self.tail();
        if head < tail {
            self.start = head;
            self.end = tail;
            self.reversed = true;
        } else {
            self.start = tail;
            self.end = head;
            self.reversed = false;
        }
    }

    /// The selected text range as (start, end) positions.
    pub fn ordered_range(&self) -> (Position, Position) {
        (self.start, self.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_creation() {
        let pos = Position::new(3, 5);
        let sel = Selection::cursor(pos);
        assert_eq!(sel.start, pos);
        assert_eq!(sel.end, pos);
        assert!(!sel.reversed);
        assert!(sel.is_empty());
    }

    #[test]
    fn range_creation() {
        let from = Position::new(1, 0);
        let to = Position::new(2, 5);
        let sel = Selection::range(from, to);
        assert_eq!(sel.start, from);
        assert_eq!(sel.end, to);
        assert!(!sel.reversed);
        assert!(!sel.is_empty());
    }

    #[test]
    fn range_auto_normalizes() {
        let from = Position::new(5, 10);
        let to = Position::new(2, 3);
        let sel = Selection::range(from, to);
        // start should be the smaller position
        assert_eq!(sel.start, to);
        assert_eq!(sel.end, from);
        assert!(sel.reversed);
    }

    #[test]
    fn head_and_tail() {
        let from = Position::new(1, 0);
        let to = Position::new(3, 7);
        let sel = Selection::range(from, to);
        // Forward selection: head at end, tail at start
        assert_eq!(sel.head(), to);
        assert_eq!(sel.tail(), from);
    }

    #[test]
    fn reversed_head_and_tail() {
        let from = Position::new(5, 10);
        let to = Position::new(2, 3);
        let sel = Selection::range(from, to);
        // Reversed selection: head at start (smaller pos), tail at end (larger pos)
        assert_eq!(sel.head(), to);
        assert_eq!(sel.tail(), from);
    }

    #[test]
    fn is_empty() {
        let pos = Position::new(0, 0);
        let cursor = Selection::cursor(pos);
        assert!(cursor.is_empty());

        let range = Selection::range(Position::new(0, 0), Position::new(0, 5));
        assert!(!range.is_empty());
    }

    #[test]
    fn collapse_to() {
        let mut sel = Selection::range(Position::new(1, 0), Position::new(3, 5));
        let target = Position::new(2, 2);
        sel.collapse_to(target);
        assert_eq!(sel.start, target);
        assert_eq!(sel.end, target);
        assert!(!sel.reversed);
        assert!(sel.is_empty());
        assert_eq!(sel.goal, SelectionGoal::None);
    }

    #[test]
    fn set_head_forward() {
        // Start with a cursor at (1, 0), then extend forward
        let mut sel = Selection::cursor(Position::new(1, 0));
        sel.set_head(Position::new(3, 5));
        assert_eq!(sel.start, Position::new(1, 0));
        assert_eq!(sel.end, Position::new(3, 5));
        assert!(!sel.reversed);
        assert_eq!(sel.head(), Position::new(3, 5));
        assert_eq!(sel.tail(), Position::new(1, 0));
    }

    #[test]
    fn set_head_backward() {
        // Start with a cursor at (3, 5), then extend backward
        let mut sel = Selection::cursor(Position::new(3, 5));
        sel.set_head(Position::new(1, 0));
        assert_eq!(sel.start, Position::new(1, 0));
        assert_eq!(sel.end, Position::new(3, 5));
        assert!(sel.reversed);
        assert_eq!(sel.head(), Position::new(1, 0));
        assert_eq!(sel.tail(), Position::new(3, 5));
    }

    #[test]
    fn ordered_range_always_returns_start_end() {
        let sel = Selection::range(Position::new(5, 0), Position::new(1, 0));
        let (start, end) = sel.ordered_range();
        assert!(start <= end);
        assert_eq!(start, Position::new(1, 0));
        assert_eq!(end, Position::new(5, 0));
    }

    #[test]
    fn default_selection() {
        let sel = Selection::default();
        assert_eq!(sel.start, Position::new(0, 0));
        assert_eq!(sel.end, Position::new(0, 0));
        assert!(!sel.reversed);
        assert!(sel.is_empty());
    }
}
