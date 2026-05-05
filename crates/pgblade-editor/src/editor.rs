use gpui::*;

use pgblade_core::highlighter::TokenKind;

use crate::buffer::EditBuffer;
use crate::display::{DisplayLine, LineHighlight, compute_display_lines};
use crate::history::UndoHistory;
use crate::selection::{Position, Selection, SelectionGoal};

const LINE_HEIGHT: f32 = 20.0;
const GUTTER_BASE_WIDTH: f32 = 40.0;

/// Events emitted by the SQL editor.
#[derive(Debug, Clone)]
pub enum SqlEditorEvent {
    /// Buffer content changed.
    Changed,
    /// User pressed Cmd+Enter (execute query).
    Execute(String),
}

impl EventEmitter<SqlEditorEvent> for SqlEditor {}

pub struct SqlEditor {
    buffer: EditBuffer,
    selection: Selection,
    history: UndoHistory,
    focus_handle: FocusHandle,
    scroll_offset: usize,
    dragging: bool,
    gutter_width: f32,
    interactive: bool,
    // Find bar state
    find_query: String,
    find_matches: Vec<(usize, usize)>, // (start_char, end_char) pairs
    find_current: usize,
    find_visible: bool,
    find_focused: bool,
    // Auto-completion state
    completions: Vec<String>,
    completion_visible: bool,
    completion_selected: usize,
    completion_prefix: String,
}

impl SqlEditor {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            buffer: EditBuffer::new(),
            selection: Selection::default(),
            history: UndoHistory::new(),
            focus_handle: cx.focus_handle(),
            scroll_offset: 0,
            dragging: false,
            gutter_width: GUTTER_BASE_WIDTH,
            interactive: true,
            find_query: String::new(),
            find_matches: Vec::new(),
            find_current: 0,
            find_visible: false,
            find_focused: false,
            completions: Vec::new(),
            completion_visible: false,
            completion_selected: 0,
            completion_prefix: String::new(),
        }
    }

    pub fn text(&self) -> String {
        self.buffer.text()
    }

    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.buffer = EditBuffer::from_str(text);
        self.selection = Selection::cursor(Position::new(0, 0));
        self.history.clear();
        cx.notify();
    }

    pub fn focus(&self, window: &mut Window) {
        self.focus_handle.focus(window);
    }

    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    pub fn set_interactive(&mut self, interactive: bool, cx: &mut Context<Self>) {
        self.interactive = interactive;
        cx.notify();
    }

    // --- Edit operations (with undo) ---

    fn insert_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.history.begin_transaction(self.selection);

        // Delete selection if non-empty
        if !self.selection.is_empty() {
            let start = self.buffer.pos_to_char(self.selection.start);
            let end = self.buffer.pos_to_char(self.selection.end);
            let edit = self.buffer.delete(start, end);
            self.history.record_edit(edit);
            self.selection.collapse_to(self.selection.start);
        }

        let offset = self.buffer.pos_to_char(self.selection.head());
        let edit = self.buffer.insert(offset, text);
        self.history.record_edit(edit);

        // Move cursor after inserted text
        let new_offset = offset + text.chars().count();
        let new_pos = self.buffer.char_to_pos(new_offset);
        self.selection.collapse_to(new_pos);

        self.history.end_transaction(self.selection);
        self.update_completions();
        cx.emit(SqlEditorEvent::Changed);
        cx.notify();
    }

    fn backspace(&mut self, cx: &mut Context<Self>) {
        self.history.begin_transaction(self.selection);

        if !self.selection.is_empty() {
            let start = self.buffer.pos_to_char(self.selection.start);
            let end = self.buffer.pos_to_char(self.selection.end);
            let edit = self.buffer.delete(start, end);
            self.history.record_edit(edit);
            self.selection.collapse_to(self.selection.start);
        } else {
            let offset = self.buffer.pos_to_char(self.selection.head());
            if offset > 0 {
                let edit = self.buffer.delete(offset - 1, offset);
                self.history.record_edit(edit);
                let new_pos = self.buffer.char_to_pos(offset - 1);
                self.selection.collapse_to(new_pos);
            }
        }

        self.history.end_transaction(self.selection);
        cx.emit(SqlEditorEvent::Changed);
        cx.notify();
    }

    fn delete_forward(&mut self, cx: &mut Context<Self>) {
        self.history.begin_transaction(self.selection);

        if !self.selection.is_empty() {
            let start = self.buffer.pos_to_char(self.selection.start);
            let end = self.buffer.pos_to_char(self.selection.end);
            let edit = self.buffer.delete(start, end);
            self.history.record_edit(edit);
            self.selection.collapse_to(self.selection.start);
        } else {
            let offset = self.buffer.pos_to_char(self.selection.head());
            if offset < self.buffer.len_chars() {
                let edit = self.buffer.delete(offset, offset + 1);
                self.history.record_edit(edit);
            }
        }

        self.history.end_transaction(self.selection);
        cx.emit(SqlEditorEvent::Changed);
        cx.notify();
    }

    fn delete_word_backward(&mut self, cx: &mut Context<Self>) {
        self.history.begin_transaction(self.selection);
        let offset = self.buffer.pos_to_char(self.selection.head());
        let word_start = self.buffer.word_start(offset);
        if word_start < offset {
            let edit = self.buffer.delete(word_start, offset);
            self.history.record_edit(edit);
            let new_pos = self.buffer.char_to_pos(word_start);
            self.selection.collapse_to(new_pos);
        }
        self.history.end_transaction(self.selection);
        cx.emit(SqlEditorEvent::Changed);
        cx.notify();
    }

    fn delete_word_forward(&mut self, cx: &mut Context<Self>) {
        self.history.begin_transaction(self.selection);
        let offset = self.buffer.pos_to_char(self.selection.head());
        let word_end = self.buffer.word_end(offset);
        if word_end > offset {
            let edit = self.buffer.delete(offset, word_end);
            self.history.record_edit(edit);
        }
        self.history.end_transaction(self.selection);
        cx.emit(SqlEditorEvent::Changed);
        cx.notify();
    }

    fn undo(&mut self, cx: &mut Context<Self>) {
        if let Some(transaction) = self.history.undo() {
            let selection_before = transaction.selection_before;
            let edits: Vec<_> = transaction.edits.clone();
            // Apply inverse edits in reverse order
            for edit in edits.iter().rev() {
                self.buffer.replace(
                    edit.offset,
                    edit.offset + edit.new_text.chars().count(),
                    &edit.old_text,
                );
            }
            self.selection = selection_before;
            cx.emit(SqlEditorEvent::Changed);
            cx.notify();
        }
    }

    fn redo(&mut self, cx: &mut Context<Self>) {
        if let Some(transaction) = self.history.redo() {
            let selection_after = transaction.selection_after;
            let edits: Vec<_> = transaction.edits.clone();
            // Apply edits in order
            for edit in &edits {
                self.buffer.replace(
                    edit.offset,
                    edit.offset + edit.old_text.chars().count(),
                    &edit.new_text,
                );
            }
            self.selection = selection_after;
            cx.emit(SqlEditorEvent::Changed);
            cx.notify();
        }
    }

    // --- Movement ---

    fn move_cursor(&mut self, pos: Position, extend_selection: bool, cx: &mut Context<Self>) {
        if extend_selection {
            self.selection.set_head(pos);
        } else {
            self.selection.collapse_to(pos);
        }
        self.ensure_cursor_visible();
        cx.notify();
    }

    fn move_left(&mut self, extend: bool, cx: &mut Context<Self>) {
        let head = self.selection.head();
        if !extend && !self.selection.is_empty() {
            self.selection.collapse_to(self.selection.start);
            cx.notify();
            return;
        }
        let offset = self.buffer.pos_to_char(head);
        if offset > 0 {
            let new_pos = self.buffer.char_to_pos(offset - 1);
            self.move_cursor(new_pos, extend, cx);
        }
    }

    fn move_right(&mut self, extend: bool, cx: &mut Context<Self>) {
        let head = self.selection.head();
        if !extend && !self.selection.is_empty() {
            self.selection.collapse_to(self.selection.end);
            cx.notify();
            return;
        }
        let offset = self.buffer.pos_to_char(head);
        if offset < self.buffer.len_chars() {
            let new_pos = self.buffer.char_to_pos(offset + 1);
            self.move_cursor(new_pos, extend, cx);
        }
    }

    fn move_up(&mut self, extend: bool, cx: &mut Context<Self>) {
        let head = self.selection.head();
        if head.line == 0 {
            return;
        }
        let goal_col = match self.selection.goal {
            SelectionGoal::Column(c) => c,
            SelectionGoal::None => head.column,
        };
        let prev_line_len = self.buffer.line_len(head.line - 1);
        let new_pos = Position::new(head.line - 1, goal_col.min(prev_line_len));
        self.selection.goal = SelectionGoal::Column(goal_col);
        self.move_cursor(new_pos, extend, cx);
    }

    fn move_down(&mut self, extend: bool, cx: &mut Context<Self>) {
        let head = self.selection.head();
        if head.line + 1 >= self.buffer.line_count() {
            return;
        }
        let goal_col = match self.selection.goal {
            SelectionGoal::Column(c) => c,
            SelectionGoal::None => head.column,
        };
        let next_line_len = self.buffer.line_len(head.line + 1);
        let new_pos = Position::new(head.line + 1, goal_col.min(next_line_len));
        self.selection.goal = SelectionGoal::Column(goal_col);
        self.move_cursor(new_pos, extend, cx);
    }

    fn move_word_left(&mut self, extend: bool, cx: &mut Context<Self>) {
        let offset = self.buffer.pos_to_char(self.selection.head());
        let new_offset = self.buffer.word_start(offset);
        let new_pos = self.buffer.char_to_pos(new_offset);
        self.move_cursor(new_pos, extend, cx);
    }

    fn move_word_right(&mut self, extend: bool, cx: &mut Context<Self>) {
        let offset = self.buffer.pos_to_char(self.selection.head());
        let new_offset = self.buffer.word_end(offset);
        let new_pos = self.buffer.char_to_pos(new_offset);
        self.move_cursor(new_pos, extend, cx);
    }

    fn move_to_line_start(&mut self, extend: bool, cx: &mut Context<Self>) {
        let head = self.selection.head();
        let line_text = self.buffer.line(head.line);
        let first_non_ws = line_text
            .chars()
            .position(|c| !c.is_whitespace())
            .unwrap_or(0);
        // Smart home: toggle between first non-whitespace and column 0
        let target_col = if head.column == first_non_ws && first_non_ws > 0 {
            0
        } else {
            first_non_ws
        };
        let new_pos = Position::new(head.line, target_col);
        self.move_cursor(new_pos, extend, cx);
    }

    fn move_to_line_end(&mut self, extend: bool, cx: &mut Context<Self>) {
        let head = self.selection.head();
        let line_len = self.buffer.line_len(head.line);
        let new_pos = Position::new(head.line, line_len);
        self.move_cursor(new_pos, extend, cx);
    }

    fn select_all(&mut self, cx: &mut Context<Self>) {
        let last_line = self.buffer.line_count().saturating_sub(1);
        let last_col = self.buffer.line_len(last_line);
        self.selection = Selection::range(Position::new(0, 0), Position::new(last_line, last_col));
        cx.notify();
    }

    // --- Line manipulation ---

    fn delete_line(&mut self, cx: &mut Context<Self>) {
        self.history.begin_transaction(self.selection);
        let line = self.selection.head().line;
        let del_start = self.buffer.pos_to_char(Position::new(line, 0));
        let del_end = if line + 1 < self.buffer.line_count() {
            self.buffer.pos_to_char(Position::new(line + 1, 0))
        } else {
            self.buffer.len_chars()
        };
        if del_start < del_end {
            let edit = self.buffer.delete(del_start, del_end);
            self.history.record_edit(edit);
        }
        let new_line = line.min(self.buffer.line_count().saturating_sub(1));
        self.selection.collapse_to(Position::new(new_line, 0));
        self.history.end_transaction(self.selection);
        cx.emit(SqlEditorEvent::Changed);
        cx.notify();
    }

    fn duplicate_line(&mut self, cx: &mut Context<Self>) {
        self.history.begin_transaction(self.selection);
        let line = self.selection.head().line;
        let text = self.buffer.line(line);
        let line_start = self.buffer.pos_to_char(Position::new(line, 0));
        let edit = self.buffer.insert(line_start, &format!("{text}\n"));
        self.history.record_edit(edit);
        let col = self.selection.head().column;
        self.selection.collapse_to(Position::new(line + 1, col));
        self.history.end_transaction(self.selection);
        cx.emit(SqlEditorEvent::Changed);
        cx.notify();
    }

    fn move_line_up(&mut self, cx: &mut Context<Self>) {
        let line = self.selection.head().line;
        if line == 0 {
            return;
        }
        self.history.begin_transaction(self.selection);
        let current = self.buffer.line(line);
        let above = self.buffer.line(line - 1);
        let start = self.buffer.pos_to_char(Position::new(line - 1, 0));
        let end = if line + 1 < self.buffer.line_count() {
            self.buffer.pos_to_char(Position::new(line + 1, 0))
        } else {
            self.buffer.len_chars()
        };
        let edit = self.buffer.delete(start, end);
        self.history.record_edit(edit);
        let swapped = if line < self.buffer.line_count() {
            format!("{current}\n{above}\n")
        } else {
            format!("{current}\n{above}")
        };
        let edit = self.buffer.insert(start, &swapped);
        self.history.record_edit(edit);
        let col = self.selection.head().column.min(current.len());
        self.selection.collapse_to(Position::new(line - 1, col));
        self.history.end_transaction(self.selection);
        cx.emit(SqlEditorEvent::Changed);
        cx.notify();
    }

    fn move_line_down(&mut self, cx: &mut Context<Self>) {
        let line = self.selection.head().line;
        if line + 1 >= self.buffer.line_count() {
            return;
        }
        self.history.begin_transaction(self.selection);
        let current = self.buffer.line(line);
        let below = self.buffer.line(line + 1);
        let start = self.buffer.pos_to_char(Position::new(line, 0));
        let end = if line + 2 < self.buffer.line_count() {
            self.buffer.pos_to_char(Position::new(line + 2, 0))
        } else {
            self.buffer.len_chars()
        };
        let edit = self.buffer.delete(start, end);
        self.history.record_edit(edit);
        let swapped = if line + 2 <= self.buffer.line_count() {
            format!("{below}\n{current}\n")
        } else {
            format!("{below}\n{current}")
        };
        let edit = self.buffer.insert(start, &swapped);
        self.history.record_edit(edit);
        let col = self.selection.head().column.min(current.len());
        self.selection.collapse_to(Position::new(line + 1, col));
        self.history.end_transaction(self.selection);
        cx.emit(SqlEditorEvent::Changed);
        cx.notify();
    }

    // --- Indent/Outdent ---

    fn indent_selection(&mut self, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            self.insert_text("    ", cx);
            return;
        }
        self.history.begin_transaction(self.selection);
        let start_line = self.selection.start.line;
        let end_line = self.selection.end.line;
        // Insert 4 spaces at the start of each selected line (in reverse to keep offsets valid)
        for line in (start_line..=end_line).rev() {
            let offset = self.buffer.pos_to_char(Position::new(line, 0));
            let edit = self.buffer.insert(offset, "    ");
            self.history.record_edit(edit);
        }
        self.selection.start.column = self.selection.start.column.saturating_add(4);
        self.selection.end.column = self.selection.end.column.saturating_add(4);
        self.history.end_transaction(self.selection);
        cx.emit(SqlEditorEvent::Changed);
        cx.notify();
    }

    fn outdent_selection(&mut self, cx: &mut Context<Self>) {
        self.history.begin_transaction(self.selection);
        let start_line = self.selection.start.line;
        let end_line = self.selection.end.line;
        for line in (start_line..=end_line).rev() {
            let line_text = self.buffer.line(line);
            let spaces = line_text.chars().take(4).take_while(|c| *c == ' ').count();
            if spaces > 0 {
                let offset = self.buffer.pos_to_char(Position::new(line, 0));
                let edit = self.buffer.delete(offset, offset + spaces);
                self.history.record_edit(edit);
            }
        }
        self.history.end_transaction(self.selection);
        cx.emit(SqlEditorEvent::Changed);
        cx.notify();
    }

    // --- Scroll ---

    fn handle_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta_lines = match &event.delta {
            ScrollDelta::Lines(delta) => -delta.y,
            ScrollDelta::Pixels(delta) => {
                let y: f32 = delta.y.into();
                -(y / LINE_HEIGHT)
            }
        };
        let max_offset = self.buffer.line_count().saturating_sub(1);
        let new_offset = (self.scroll_offset as f32 + delta_lines).clamp(0.0, max_offset as f32);
        self.scroll_offset = new_offset as usize;
        cx.notify();
    }

    // --- Clipboard ---

    fn copy(&self, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            // Copy entire current line
            let line = self.buffer.line(self.selection.head().line);
            cx.write_to_clipboard(ClipboardItem::new_string(line + "\n"));
        } else {
            let start = self.buffer.pos_to_char(self.selection.start);
            let end = self.buffer.pos_to_char(self.selection.end);
            let text = self.buffer.slice(start, end);
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn cut(&mut self, cx: &mut Context<Self>) {
        self.copy(cx);
        if !self.selection.is_empty() {
            self.backspace(cx);
        }
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        if let Some(item) = cx.read_from_clipboard()
            && let Some(text) = item.text()
        {
            self.insert_text(&text, cx);
        }
    }

    // --- Find bar ---

    fn toggle_find(&mut self, cx: &mut Context<Self>) {
        self.find_visible = !self.find_visible;
        if self.find_visible {
            self.find_focused = true;
        } else {
            self.find_focused = false;
            self.find_matches.clear();
            self.find_current = 0;
        }
        cx.notify();
    }

    fn update_find_matches(&mut self) {
        self.find_matches.clear();
        if self.find_query.is_empty() {
            return;
        }
        let text = self.buffer.text();
        let query_lower = self.find_query.to_lowercase();
        let text_lower = text.to_lowercase();
        let mut start = 0;
        while let Some(pos) = text_lower[start..].find(&query_lower) {
            let match_start = start + pos;
            let match_end = match_start + self.find_query.len();
            // Convert byte offsets to char offsets
            let char_start = text[..match_start].chars().count();
            let char_end = text[..match_end].chars().count();
            self.find_matches.push((char_start, char_end));
            start = match_end;
        }
        if self.find_current >= self.find_matches.len() {
            self.find_current = 0;
        }
    }

    fn find_next(&mut self, cx: &mut Context<Self>) {
        if self.find_matches.is_empty() {
            return;
        }
        self.find_current = (self.find_current + 1) % self.find_matches.len();
        self.jump_to_current_match(cx);
    }

    fn find_prev(&mut self, cx: &mut Context<Self>) {
        if self.find_matches.is_empty() {
            return;
        }
        self.find_current = if self.find_current == 0 {
            self.find_matches.len() - 1
        } else {
            self.find_current - 1
        };
        self.jump_to_current_match(cx);
    }

    fn jump_to_current_match(&mut self, cx: &mut Context<Self>) {
        if let Some(&(start, end)) = self.find_matches.get(self.find_current) {
            let start_pos = self.buffer.char_to_pos(start);
            let end_pos = self.buffer.char_to_pos(end);
            self.selection = Selection::range(start_pos, end_pos);
            self.ensure_cursor_visible();
            cx.notify();
        }
    }

    // --- Auto-completion ---

    pub fn set_completion_items(&mut self, items: Vec<String>, cx: &mut Context<Self>) {
        self.completions = items;
        cx.notify();
    }

    fn update_completions(&mut self) {
        let offset = self.buffer.pos_to_char(self.selection.head());
        let text = self.buffer.text();

        // Find byte position of cursor
        let byte_pos = text
            .char_indices()
            .take(offset)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        let before_cursor = &text[..byte_pos];

        let word_start = before_cursor
            .rfind(|c: char| c.is_whitespace() || "(),;.".contains(c))
            .map(|i| i + 1)
            .unwrap_or(0);
        let prefix = &before_cursor[word_start..];

        if prefix.len() < 2 {
            self.completion_visible = false;
            self.completion_prefix.clear();
            return;
        }

        self.completion_prefix = prefix.to_lowercase();
        let has_matches = self
            .completions
            .iter()
            .any(|item| item.to_lowercase().starts_with(&self.completion_prefix));

        self.completion_visible = has_matches;
        self.completion_selected = 0;
    }

    fn filtered_completions(&self) -> Vec<String> {
        if self.completion_prefix.is_empty() {
            return Vec::new();
        }
        self.completions
            .iter()
            .filter(|item| item.to_lowercase().starts_with(&self.completion_prefix))
            .take(10)
            .cloned()
            .collect()
    }

    fn accept_completion(&mut self, cx: &mut Context<Self>) {
        if !self.completion_visible {
            return;
        }
        let filtered = self.filtered_completions();
        if let Some(item) = filtered.get(self.completion_selected) {
            let offset = self.buffer.pos_to_char(self.selection.head());
            let prefix_char_len = self.completion_prefix.chars().count();
            let start = offset.saturating_sub(prefix_char_len);
            self.history.begin_transaction(self.selection);
            let edit = self.buffer.replace(start, offset, item);
            self.history.record_edit(edit);
            let new_pos = self.buffer.char_to_pos(start + item.chars().count());
            self.selection.collapse_to(new_pos);
            self.history.end_transaction(self.selection);
            cx.emit(SqlEditorEvent::Changed);
        }
        self.completion_visible = false;
        cx.notify();
    }

    // --- Scroll ---

    fn ensure_cursor_visible(&mut self) {
        let cursor_line = self.selection.head().line;
        // Keep at least 2 lines of context above/below
        if cursor_line < self.scroll_offset + 2 {
            self.scroll_offset = cursor_line.saturating_sub(2);
        }
        // Approximate visible lines (will be refined when we have viewport height)
        let visible_lines = 30;
        if cursor_line >= self.scroll_offset + visible_lines - 2 {
            self.scroll_offset = cursor_line.saturating_sub(visible_lines - 3);
        }
    }

    // --- Mouse handling ---

    /// Convert a pixel position (relative to the editor element) to a text Position.
    fn pixel_to_position(&self, point: Point<Pixels>) -> Position {
        let y: f32 = point.y.into();
        let x: f32 = point.x.into();

        let line = ((y / LINE_HEIGHT) as usize + self.scroll_offset)
            .min(self.buffer.line_count().saturating_sub(1));

        // Approximate column from x position (subtract gutter + padding)
        let text_x = (x - self.gutter_width - 8.0).max(0.0);
        let char_width = 8.4_f32;
        let col = (text_x / char_width).round() as usize;
        let col = col.min(self.buffer.line_len(line));

        Position::new(line, col)
    }

    fn handle_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window);
        let pos = self.pixel_to_position(event.position);

        if event.click_count == 2 {
            // Double-click: select word
            let offset = self.buffer.pos_to_char(pos);
            let word_start = self.buffer.word_start(offset);
            let word_end = self.buffer.word_end(offset);
            // If word_start == word_end (e.g., on whitespace), select at least one char
            let start_pos = self.buffer.char_to_pos(word_start);
            let end_pos = self.buffer.char_to_pos(word_end);
            self.selection = Selection::range(start_pos, end_pos);
        } else {
            // Single click: place cursor
            self.selection.collapse_to(pos);
            self.dragging = true;
        }
        cx.notify();
    }

    fn handle_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.dragging && event.pressed_button == Some(MouseButton::Left) {
            let pos = self.pixel_to_position(event.position);
            self.selection.set_head(pos);
            self.ensure_cursor_visible();
            cx.notify();
        }
    }

    fn handle_mouse_up(
        &mut self,
        _event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dragging = false;
        cx.notify();
    }

    // --- Key handling ---

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ks = &event.keystroke;
        let cmd = ks.modifiers.platform;
        let alt = ks.modifiers.alt;
        let shift = ks.modifiers.shift;

        // Handle find bar key input when focused
        if self.find_focused {
            match ks.key.as_str() {
                "escape" => {
                    self.find_visible = false;
                    self.find_focused = false;
                    self.find_matches.clear();
                    cx.notify();
                }
                "enter" if shift => self.find_prev(cx),
                "enter" => self.find_next(cx),
                "backspace" => {
                    self.find_query.pop();
                    self.update_find_matches();
                    cx.notify();
                }
                "f" if cmd => {
                    // Cmd+F while find is open: toggle focus back to editor
                    self.find_focused = false;
                    cx.notify();
                }
                _ => {
                    if let Some(ref ch) = ks.key_char
                        && !cmd
                        && !ks.modifiers.control
                    {
                        self.find_query.push_str(ch);
                        self.update_find_matches();
                        if !self.find_matches.is_empty() {
                            self.find_current = 0;
                            self.jump_to_current_match(cx);
                        }
                        cx.notify();
                    }
                }
            }
            return;
        }

        // Handle completion navigation when visible
        if self.completion_visible {
            match ks.key.as_str() {
                "up" => {
                    if self.completion_selected > 0 {
                        self.completion_selected -= 1;
                    }
                    cx.notify();
                    return;
                }
                "down" => {
                    let count = self.filtered_completions().len();
                    if self.completion_selected + 1 < count {
                        self.completion_selected += 1;
                    }
                    cx.notify();
                    return;
                }
                "enter" | "tab" => {
                    self.accept_completion(cx);
                    return;
                }
                "escape" => {
                    self.completion_visible = false;
                    cx.notify();
                    return;
                }
                _ => {
                    // Fall through to normal handling; completions update after insert
                }
            }
        }

        match ks.key.as_str() {
            // Movement
            "left" if cmd => self.move_to_line_start(shift, cx),
            "left" if alt => self.move_word_left(shift, cx),
            "left" => self.move_left(shift, cx),
            "right" if cmd => self.move_to_line_end(shift, cx),
            "right" if alt => self.move_word_right(shift, cx),
            "right" => self.move_right(shift, cx),
            "up" if alt => self.move_line_up(cx),
            "down" if alt => self.move_line_down(cx),
            "up" => self.move_up(shift, cx),
            "down" => self.move_down(shift, cx),
            "home" => self.move_to_line_start(shift, cx),
            "end" => self.move_to_line_end(shift, cx),

            // Editing
            "backspace" if alt => self.delete_word_backward(cx),
            "backspace" => self.backspace(cx),
            "delete" if alt => self.delete_word_forward(cx),
            "delete" => self.delete_forward(cx),
            "enter" if cmd => {
                cx.emit(SqlEditorEvent::Execute(self.buffer.text()));
            }
            "enter" => {
                // Auto-indent: preserve leading whitespace from current line
                let line = self.selection.head().line;
                let line_text = self.buffer.line(line);
                let indent: String = line_text
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .collect();
                self.insert_text(&format!("\n{indent}"), cx);
            }
            "tab" if shift => self.outdent_selection(cx),
            "tab" => self.indent_selection(cx),

            // Line manipulation
            "k" if cmd && shift => self.delete_line(cx),
            "d" if cmd && shift => self.duplicate_line(cx),

            // Selection + clipboard
            "a" if cmd => self.select_all(cx),
            "c" if cmd => self.copy(cx),
            "x" if cmd => self.cut(cx),
            "v" if cmd => self.paste(cx),
            "z" if cmd && shift => self.redo(cx),
            "z" if cmd => self.undo(cx),

            // Find
            "f" if cmd => self.toggle_find(cx),

            _ => {
                // Auto-close brackets
                if let Some(ref ch) = ks.key_char
                    && !cmd
                    && !ks.modifiers.control
                {
                    let closer = match ch.as_str() {
                        "(" => Some(")"),
                        "[" => Some("]"),
                        "{" => Some("}"),
                        _ => None,
                    };
                    if let Some(close) = closer {
                        if self.selection.is_empty() {
                            self.insert_text(&format!("{ch}{close}"), cx);
                            // Move cursor back between the brackets
                            let offset = self.buffer.pos_to_char(self.selection.head());
                            if offset > 0 {
                                let pos = self.buffer.char_to_pos(offset - 1);
                                self.selection.collapse_to(pos);
                                cx.notify();
                            }
                        } else {
                            self.insert_text(ch, cx);
                        }
                    } else {
                        self.insert_text(ch, cx);
                    }
                }
            }
        }
    }

    // --- Rendering ---

    fn render_gutter(&self, display_lines: &[DisplayLine]) -> Div {
        let line_count = self.buffer.line_count();
        let width = if line_count >= 1000 {
            56.0
        } else if line_count >= 100 {
            48.0
        } else {
            GUTTER_BASE_WIDTH
        };

        let mut gutter = div()
            .w(px(width))
            .flex_shrink_0()
            .border_r_1()
            .border_color(rgb(0x333333))
            .bg(rgb(0x1a1a1a))
            .flex()
            .flex_col();

        for dl in display_lines {
            gutter = gutter.child(
                div()
                    .h(px(LINE_HEIGHT))
                    .pr_2()
                    .flex()
                    .items_center()
                    .justify_end()
                    .text_xs()
                    .text_color(rgb(0x555555))
                    .child(dl.line_number.to_string()),
            );
        }

        gutter
    }

    fn render_text_area(&self, display_lines: &[DisplayLine]) -> Div {
        let cursor = self.selection.head();
        let sel_start = self.selection.start;
        let sel_end = self.selection.end;
        let has_selection = !self.selection.is_empty();

        let mut area = div().flex_1().flex().flex_col().pl_2();

        for dl in display_lines {
            let line_idx = dl.line_number - 1; // 0-based
            let is_cursor_line = line_idx == cursor.line;

            let mut line_div = div()
                .h(px(LINE_HEIGHT))
                .flex()
                .flex_row()
                .items_center()
                .text_xs()
                .font_family("Monaco");

            // Current line highlight
            if is_cursor_line && !has_selection {
                line_div = line_div.bg(rgb(0x222222));
            }

            // Render text with highlights
            if dl.text.is_empty() {
                // Empty line -- show cursor if this is cursor line
                if is_cursor_line {
                    line_div = line_div.child(
                        div()
                            .w(px(1.5))
                            .h(px(16.0))
                            .bg(rgb(0x4fc1ff))
                            .flex_shrink_0(),
                    );
                }
            } else {
                line_div = line_div.child(self.render_line_content(
                    dl,
                    line_idx,
                    cursor,
                    sel_start,
                    sel_end,
                    has_selection,
                ));
            }

            area = area.child(line_div);
        }

        area
    }

    fn render_line_content(
        &self,
        dl: &DisplayLine,
        line_idx: usize,
        cursor: Position,
        sel_start: Position,
        sel_end: Position,
        has_selection: bool,
    ) -> Div {
        let is_cursor_line = line_idx == cursor.line;

        // If this is the cursor line and no selection, split at cursor column
        if is_cursor_line && !has_selection {
            let col = cursor.column.min(dl.text.len());
            let before = &dl.text[..col];
            let after = &dl.text[col..];

            return div()
                .flex()
                .flex_row()
                .child(self.render_highlighted_span(before, &dl.highlights, 0))
                .child(
                    div()
                        .w(px(1.5))
                        .h(px(16.0))
                        .bg(rgb(0x4fc1ff))
                        .flex_shrink_0(),
                )
                .child(self.render_highlighted_span(after, &dl.highlights, col));
        }

        // Line with possible selection highlight
        if has_selection && line_idx >= sel_start.line && line_idx <= sel_end.line {
            let line_len = dl.text.len();
            let sel_col_start = if line_idx == sel_start.line {
                sel_start.column.min(line_len)
            } else {
                0
            };
            let sel_col_end = if line_idx == sel_end.line {
                sel_end.column.min(line_len)
            } else {
                line_len
            };

            let before = &dl.text[..sel_col_start];
            let selected = &dl.text[sel_col_start..sel_col_end];
            let after = &dl.text[sel_col_end..];

            let mut row = div()
                .flex()
                .flex_row()
                .child(self.render_highlighted_span(before, &dl.highlights, 0))
                .child(div().bg(rgb(0x264f78)).child(self.render_highlighted_span(
                    selected,
                    &dl.highlights,
                    sel_col_start,
                )));

            if is_cursor_line {
                row = row.child(
                    div()
                        .w(px(1.5))
                        .h(px(16.0))
                        .bg(rgb(0x4fc1ff))
                        .flex_shrink_0(),
                );
            }

            row = row.child(self.render_highlighted_span(after, &dl.highlights, sel_col_end));
            return row;
        }

        // Plain line -- just render with highlights
        self.render_highlighted_span(&dl.text, &dl.highlights, 0)
    }

    fn render_highlighted_span(
        &self,
        text: &str,
        highlights: &[LineHighlight],
        col_offset: usize,
    ) -> Div {
        if text.is_empty() {
            return div();
        }

        let mut container = div().flex().flex_row();
        let start = col_offset;
        let end = col_offset + text.len();
        let mut pos = start;

        for hl in highlights {
            if hl.end <= start || hl.start >= end {
                continue;
            }
            let h_start = hl.start.max(start);
            let h_end = hl.end.min(end);

            if pos < h_start {
                container = container.child(
                    div()
                        .text_color(rgb(0xd4d4d4))
                        .child(text[pos - start..h_start - start].to_string()),
                );
            }

            let color = token_color(hl.kind);

            container = container.child(
                div()
                    .text_color(color)
                    .child(text[h_start - start..h_end - start].to_string()),
            );
            pos = h_end;
        }

        if pos < end {
            container = container.child(
                div()
                    .text_color(rgb(0xd4d4d4))
                    .child(text[pos - start..].to_string()),
            );
        }

        container
    }

    fn render_find_bar(&self) -> Div {
        let match_info = if self.find_matches.is_empty() {
            if self.find_query.is_empty() {
                String::new()
            } else {
                "No matches".to_string()
            }
        } else {
            format!("{}/{}", self.find_current + 1, self.find_matches.len())
        };

        let query_display = if self.find_query.is_empty() {
            "Search...".to_string()
        } else {
            self.find_query.clone()
        };

        let border_color = if self.find_focused {
            rgb(0x4fc1ff)
        } else {
            rgb(0x3e3e3e)
        };

        div()
            .h(px(32.0))
            .w_full()
            .px_3()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .bg(rgb(0x252525))
            .border_b_1()
            .border_color(rgb(0x333333))
            .flex_shrink_0()
            .child(div().text_xs().text_color(rgb(0x888888)).child("Find:"))
            .child(
                div()
                    .flex_1()
                    .px_2()
                    .py_1()
                    .bg(rgb(0x1e1e1e))
                    .border_1()
                    .border_color(border_color)
                    .rounded_sm()
                    .text_xs()
                    .text_color(rgb(0xd4d4d4))
                    .child(query_display),
            )
            .child(div().text_xs().text_color(rgb(0x888888)).child(match_info))
    }

    fn render_completion_popup(&self) -> Div {
        let filtered = self.filtered_completions();
        let cursor = self.selection.head();
        let cursor_y = (cursor.line.saturating_sub(self.scroll_offset) + 1) as f32 * LINE_HEIGHT
            + if self.find_visible { 32.0 } else { 0.0 };
        let cursor_x = self.gutter_width + 8.0 + cursor.column as f32 * 8.4;

        let mut popup = div()
            .absolute()
            .top(px(cursor_y))
            .left(px(cursor_x))
            .w(px(220.0))
            .bg(rgb(0x252525))
            .border_1()
            .border_color(rgb(0x444444))
            .rounded_sm()
            .overflow_hidden();

        for (i, item) in filtered.iter().enumerate() {
            let is_selected = i == self.completion_selected;
            let bg = if is_selected {
                rgb(0x333333)
            } else {
                rgb(0x252525)
            };
            popup = popup.child(
                div()
                    .h(px(24.0))
                    .px_2()
                    .flex()
                    .items_center()
                    .text_xs()
                    .text_color(rgb(0xd4d4d4))
                    .bg(bg)
                    .child(item.clone()),
            );
        }

        popup
    }
}

impl Render for SqlEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Compute visible lines
        let visible_line_count = 50; // will be dynamic later
        let display_lines =
            compute_display_lines(&self.buffer, self.scroll_offset, visible_line_count);

        // Update gutter width based on line count
        let line_count = self.buffer.line_count();
        self.gutter_width = if line_count >= 1000 {
            56.0
        } else if line_count >= 100 {
            48.0
        } else {
            GUTTER_BASE_WIDTH
        };

        let find_visible = self.find_visible;
        let completion_visible = self.completion_visible && !self.filtered_completions().is_empty();

        // Outer container: flex_col to stack find bar above editor content
        let mut outer = div()
            .id("sql-editor")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xd4d4d4))
            .font_family("Monaco")
            .text_sm()
            .overflow_hidden()
            .relative();

        if self.interactive {
            outer = outer
                .track_focus(&self.focus_handle)
                .on_key_down(cx.listener(Self::handle_key_down))
                .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_mouse_down))
                .on_mouse_move(cx.listener(Self::handle_mouse_move))
                .on_mouse_up(MouseButton::Left, cx.listener(Self::handle_mouse_up))
                .on_scroll_wheel(cx.listener(Self::handle_scroll_wheel))
                .cursor_text();
        }

        // Find bar (if visible)
        if find_visible {
            outer = outer.child(self.render_find_bar());
        }

        // Editor content: gutter + text area in a flex_row
        outer = outer.child(
            div()
                .flex_1()
                .flex()
                .flex_row()
                .overflow_hidden()
                .child(self.render_gutter(&display_lines))
                .child(self.render_text_area(&display_lines)),
        );

        // Completion popup (if visible)
        if completion_visible {
            outer = outer.child(self.render_completion_popup());
        }

        outer
    }
}

/// Map a token kind to its display color (VS Code dark theme inspired).
fn token_color(kind: TokenKind) -> Hsla {
    match kind {
        TokenKind::Keyword => rgb(0x569cd6).into(),
        TokenKind::String => rgb(0xce9178).into(),
        TokenKind::Number => rgb(0xb5cea8).into(),
        TokenKind::Comment => rgb(0x6a9955).into(),
        TokenKind::Operator => rgb(0xd4d4d4).into(),
        TokenKind::Identifier => rgb(0x9cdcfe).into(),
    }
}
