use crate::buffer::EditOp;
use crate::selection::Selection;

/// A group of edits that should be undone/redone together.
#[derive(Debug, Clone)]
pub struct Transaction {
    pub edits: Vec<EditOp>,
    pub selection_before: Selection,
    pub selection_after: Selection,
}

/// Transaction-based undo/redo history.
///
/// Edits are grouped into transactions. Each transaction represents
/// a logical unit of work (e.g., typing a word, deleting a selection).
pub struct UndoHistory {
    undo_stack: Vec<Transaction>,
    redo_stack: Vec<Transaction>,
    current_edits: Vec<EditOp>,
    current_selection_before: Option<Selection>,
}

impl Default for UndoHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoHistory {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current_edits: Vec::new(),
            current_selection_before: None,
        }
    }

    /// Begin recording a new transaction. Call this before edits.
    pub fn begin_transaction(&mut self, selection: Selection) {
        if self.current_selection_before.is_none() {
            self.current_selection_before = Some(selection);
        }
    }

    /// Record an edit as part of the current transaction.
    pub fn record_edit(&mut self, edit: EditOp) {
        if !edit.is_empty() {
            self.current_edits.push(edit);
        }
    }

    /// End the current transaction. Call this after edits.
    pub fn end_transaction(&mut self, selection: Selection) {
        if !self.current_edits.is_empty() {
            let transaction = Transaction {
                edits: std::mem::take(&mut self.current_edits),
                selection_before: self.current_selection_before.take().unwrap_or_default(),
                selection_after: selection,
            };
            self.undo_stack.push(transaction);
            // New edit invalidates redo stack
            self.redo_stack.clear();
        }
        self.current_selection_before = None;
    }

    /// Get the next transaction to undo. Returns None if nothing to undo.
    /// The caller must apply the inverse edits to the buffer.
    pub fn undo(&mut self) -> Option<&Transaction> {
        let transaction = self.undo_stack.pop()?;
        self.redo_stack.push(transaction);
        self.redo_stack.last()
    }

    /// Get the next transaction to redo. Returns None if nothing to redo.
    /// The caller must apply the edits to the buffer.
    pub fn redo(&mut self) -> Option<&Transaction> {
        let transaction = self.redo_stack.pop()?;
        self.undo_stack.push(transaction);
        self.undo_stack.last()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.current_edits.clear();
        self.current_selection_before = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selection::Position;

    fn make_edit(offset: usize, old: &str, new: &str) -> EditOp {
        EditOp {
            offset,
            old_text: old.to_string(),
            new_text: new.to_string(),
        }
    }

    fn cursor_at(line: usize, col: usize) -> Selection {
        Selection::cursor(Position::new(line, col))
    }

    #[test]
    fn new_history_is_empty() {
        let history = UndoHistory::new();
        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn undo_after_edit() {
        let mut history = UndoHistory::new();

        history.begin_transaction(cursor_at(0, 0));
        history.record_edit(make_edit(0, "", "hello"));
        history.end_transaction(cursor_at(0, 5));

        assert!(history.can_undo());
        let tx = history.undo().unwrap();
        assert_eq!(tx.edits.len(), 1);
        assert_eq!(tx.edits[0].new_text, "hello");
        assert_eq!(tx.selection_before, cursor_at(0, 0));
        assert_eq!(tx.selection_after, cursor_at(0, 5));
    }

    #[test]
    fn redo_after_undo() {
        let mut history = UndoHistory::new();

        history.begin_transaction(cursor_at(0, 0));
        history.record_edit(make_edit(0, "", "hello"));
        history.end_transaction(cursor_at(0, 5));

        assert!(!history.can_redo());
        history.undo();
        assert!(history.can_redo());

        let tx = history.redo().unwrap();
        assert_eq!(tx.edits.len(), 1);
        assert_eq!(tx.edits[0].new_text, "hello");
    }

    #[test]
    fn redo_cleared_on_new_edit() {
        let mut history = UndoHistory::new();

        // First edit
        history.begin_transaction(cursor_at(0, 0));
        history.record_edit(make_edit(0, "", "hello"));
        history.end_transaction(cursor_at(0, 5));

        // Undo it
        history.undo();
        assert!(history.can_redo());

        // New edit should clear redo
        history.begin_transaction(cursor_at(0, 0));
        history.record_edit(make_edit(0, "", "world"));
        history.end_transaction(cursor_at(0, 5));

        assert!(!history.can_redo());
    }

    #[test]
    fn multiple_transactions() {
        let mut history = UndoHistory::new();

        // Transaction 1
        history.begin_transaction(cursor_at(0, 0));
        history.record_edit(make_edit(0, "", "hello"));
        history.end_transaction(cursor_at(0, 5));

        // Transaction 2
        history.begin_transaction(cursor_at(0, 5));
        history.record_edit(make_edit(5, "", " world"));
        history.end_transaction(cursor_at(0, 11));

        // Undo transaction 2
        let tx2 = history.undo().unwrap();
        assert_eq!(tx2.edits[0].new_text, " world");

        // Undo transaction 1
        let tx1 = history.undo().unwrap();
        assert_eq!(tx1.edits[0].new_text, "hello");

        assert!(!history.can_undo());
    }

    #[test]
    fn empty_transaction_not_recorded() {
        let mut history = UndoHistory::new();

        history.begin_transaction(cursor_at(0, 0));
        // No edits recorded
        history.end_transaction(cursor_at(0, 0));

        assert!(!history.can_undo());
    }

    #[test]
    fn selection_restored_on_undo() {
        let mut history = UndoHistory::new();

        let sel_before = Selection::range(Position::new(0, 0), Position::new(0, 5));
        let sel_after = cursor_at(0, 3);

        history.begin_transaction(sel_before);
        history.record_edit(make_edit(0, "hello", "foo"));
        history.end_transaction(sel_after);

        let tx = history.undo().unwrap();
        assert_eq!(tx.selection_before, sel_before);
        assert_eq!(tx.selection_after, sel_after);
    }

    #[test]
    fn empty_edit_op_not_recorded() {
        let mut history = UndoHistory::new();

        history.begin_transaction(cursor_at(0, 0));
        // Record an empty edit (should be ignored)
        history.record_edit(EditOp {
            offset: 0,
            old_text: String::new(),
            new_text: String::new(),
        });
        history.end_transaction(cursor_at(0, 0));

        assert!(!history.can_undo());
    }

    #[test]
    fn clear_removes_all_history() {
        let mut history = UndoHistory::new();

        history.begin_transaction(cursor_at(0, 0));
        history.record_edit(make_edit(0, "", "hello"));
        history.end_transaction(cursor_at(0, 5));

        history.undo();
        assert!(history.can_redo());

        history.clear();
        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }
}
