use gpui::actions;

// Movement actions
actions!(
    pgblade_editor,
    [
        MoveLeft,
        MoveRight,
        MoveUp,
        MoveDown,
        MoveWordLeft,
        MoveWordRight,
        MoveToLineStart,
        MoveToLineEnd,
        MoveToBufferStart,
        MoveToBufferEnd,
        // Selection variants
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectWordLeft,
        SelectWordRight,
        SelectToLineStart,
        SelectToLineEnd,
        SelectAll,
        // Editing
        Backspace,
        Delete,
        DeleteWordBackward,
        DeleteWordForward,
        DeleteLine,
        Newline,
        Tab,
        // Clipboard
        Cut,
        Copy,
        Paste,
        // History
        Undo,
        Redo,
    ]
);
