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
    scroll_offset: usize, // first visible line (0-based)
    dragging: bool,       // mouse is being dragged for selection
    gutter_width: f32,    // computed gutter width in pixels
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
        let new_pos = Position::new(head.line, 0);
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

        match ks.key.as_str() {
            "left" if cmd => self.move_to_line_start(shift, cx),
            "left" if alt => self.move_word_left(shift, cx),
            "left" => self.move_left(shift, cx),

            "right" if cmd => self.move_to_line_end(shift, cx),
            "right" if alt => self.move_word_right(shift, cx),
            "right" => self.move_right(shift, cx),

            "up" => self.move_up(shift, cx),
            "down" => self.move_down(shift, cx),

            "home" => self.move_to_line_start(shift, cx),
            "end" => self.move_to_line_end(shift, cx),

            "backspace" if alt => self.delete_word_backward(cx),
            "backspace" => self.backspace(cx),

            "delete" if alt => self.delete_word_forward(cx),
            "delete" => self.delete_forward(cx),

            "enter" if cmd => {
                cx.emit(SqlEditorEvent::Execute(self.buffer.text()));
            }
            "enter" => self.insert_text("\n", cx),

            "tab" => self.insert_text("    ", cx),

            "a" if cmd => self.select_all(cx),
            "c" if cmd => self.copy(cx),
            "x" if cmd => self.cut(cx),
            "v" if cmd => self.paste(cx),
            "z" if cmd && shift => self.redo(cx),
            "z" if cmd => self.undo(cx),

            _ => {
                // Insert printable character
                if let Some(ref ch) = ks.key_char
                    && !cmd
                    && !ks.modifiers.control
                {
                    self.insert_text(ch, cx);
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

        div()
            .id("sql-editor")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_mouse_down))
            .on_mouse_move(cx.listener(Self::handle_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::handle_mouse_up))
            .size_full()
            .flex()
            .flex_row()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xd4d4d4))
            .font_family("Monaco")
            .text_sm()
            .cursor_text()
            .overflow_hidden()
            .child(self.render_gutter(&display_lines))
            .child(self.render_text_area(&display_lines))
    }
}
