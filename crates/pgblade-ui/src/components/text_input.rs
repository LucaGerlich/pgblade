use gpui::*;
use pgblade_core::highlighter::{TokenKind, highlight_sql};

/// A reusable text input component with cursor movement and basic editing.
///
/// Uses GPUI's focus and keyboard system for text input.
/// Supports both single-line (form fields) and multi-line (SQL editor) modes.
pub struct TextInput {
    text: String,
    cursor: usize,
    focus_handle: FocusHandle,
    multiline: bool,
    placeholder: SharedString,
    masked: bool,
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>, multiline: bool) -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            focus_handle: cx.focus_handle(),
            multiline,
            placeholder: "".into(),
            masked: false,
        }
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn with_masked(mut self, masked: bool) -> Self {
        self.masked = masked;
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.text = text;
        self.cursor = self.text.len();
        cx.notify();
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    pub fn focus(&self, window: &mut Window) {
        self.focus_handle.focus(window);
    }

    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    fn insert_char(&mut self, ch: &str, cx: &mut Context<Self>) {
        self.text.insert_str(self.cursor, ch);
        self.cursor += ch.len();
        cx.emit(TextInputEvent::Changed(self.text.clone()));
        cx.notify();
    }

    fn backspace(&mut self, cx: &mut Context<Self>) {
        if self.cursor > 0 {
            // Find the previous character boundary
            let prev = self.text[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.text.drain(prev..self.cursor);
            self.cursor = prev;
            cx.emit(TextInputEvent::Changed(self.text.clone()));
            cx.notify();
        }
    }

    fn delete_forward(&mut self, cx: &mut Context<Self>) {
        if self.cursor < self.text.len() {
            let next = self.text[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.text.len());
            self.text.drain(self.cursor..next);
            cx.emit(TextInputEvent::Changed(self.text.clone()));
            cx.notify();
        }
    }

    fn move_left(&mut self, cx: &mut Context<Self>) {
        if self.cursor > 0 {
            self.cursor = self.text[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            cx.notify();
        }
    }

    fn move_right(&mut self, cx: &mut Context<Self>) {
        if self.cursor < self.text.len() {
            self.cursor = self.text[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.text.len());
            cx.notify();
        }
    }

    fn move_to_start(&mut self, cx: &mut Context<Self>) {
        if self.multiline {
            // Move to start of current line
            let line_start = self.text[..self.cursor]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            self.cursor = line_start;
        } else {
            self.cursor = 0;
        }
        cx.notify();
    }

    fn move_to_end(&mut self, cx: &mut Context<Self>) {
        if self.multiline {
            // Move to end of current line
            let line_end = self.text[self.cursor..]
                .find('\n')
                .map(|i| self.cursor + i)
                .unwrap_or(self.text.len());
            self.cursor = line_end;
        } else {
            self.cursor = self.text.len();
        }
        cx.notify();
    }

    fn move_up(&mut self, cx: &mut Context<Self>) {
        if !self.multiline {
            return;
        }
        let col = self.current_column();
        if let Some(prev_line_start) = self.prev_line_start() {
            let prev_line_len = self.text[prev_line_start..self.cursor]
                .find('\n')
                .unwrap_or(self.cursor - prev_line_start);
            self.cursor = prev_line_start + col.min(prev_line_len);
            cx.notify();
        }
    }

    fn move_down(&mut self, cx: &mut Context<Self>) {
        if !self.multiline {
            return;
        }
        let col = self.current_column();
        if let Some(next_line_start) = self.next_line_start() {
            let next_line_len = self.text[next_line_start..]
                .find('\n')
                .unwrap_or(self.text.len() - next_line_start);
            self.cursor = next_line_start + col.min(next_line_len);
            cx.notify();
        }
    }

    fn current_column(&self) -> usize {
        let line_start = self.text[..self.cursor]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        self.cursor - line_start
    }

    fn prev_line_start(&self) -> Option<usize> {
        let current_line_start = self.text[..self.cursor]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        if current_line_start == 0 {
            return None;
        }
        let prev_line_start = self.text[..current_line_start - 1]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        Some(prev_line_start)
    }

    fn next_line_start(&self) -> Option<usize> {
        self.text[self.cursor..]
            .find('\n')
            .map(|i| self.cursor + i + 1)
            .filter(|&pos| pos <= self.text.len())
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;

        // Handle special keys first
        match keystroke.key.as_str() {
            "backspace" => {
                self.backspace(cx);
                return;
            }
            "delete" => {
                self.delete_forward(cx);
                return;
            }
            "left" => {
                self.move_left(cx);
                return;
            }
            "right" => {
                self.move_right(cx);
                return;
            }
            "up" => {
                self.move_up(cx);
                return;
            }
            "down" => {
                self.move_down(cx);
                return;
            }
            "home" => {
                self.move_to_start(cx);
                return;
            }
            "end" => {
                self.move_to_end(cx);
                return;
            }
            "enter" => {
                if self.multiline && !keystroke.modifiers.platform {
                    self.insert_char("\n", cx);
                } else if !self.multiline {
                    cx.emit(TextInputEvent::Submit(self.text.clone()));
                }
                // cmd+enter is handled by the parent via actions
                return;
            }
            "tab" => {
                if self.multiline {
                    self.insert_char("    ", cx);
                    return;
                }
                // In single-line mode, let tab propagate for focus cycling
                return;
            }
            "a" if keystroke.modifiers.platform => {
                // Cmd+A: select all (move cursor to end for now)
                self.cursor = self.text.len();
                cx.notify();
                return;
            }
            "v" if keystroke.modifiers.platform => {
                // Cmd+V: paste from clipboard
                if let Some(item) = cx.read_from_clipboard()
                    && let Some(text) = item.text()
                {
                    self.insert_char(&text, cx);
                }
                return;
            }
            _ => {}
        }

        // Insert the character if available
        if let Some(ref key_char) = keystroke.key_char
            && !keystroke.modifiers.platform
            && !keystroke.modifiers.control
        {
            self.insert_char(key_char, cx);
        }
    }

    /// Render the line numbers gutter for multiline mode.
    fn render_line_numbers(&self) -> impl IntoElement {
        let line_count = self.text.matches('\n').count() + 1;
        let width = if line_count >= 100 { 48.0 } else { 36.0 };

        div()
            .w(px(width))
            .flex_shrink_0()
            .pt_2()
            .pr_2()
            .border_r_1()
            .border_color(rgb(0x333333))
            .bg(rgb(0x1a1a1a))
            .flex()
            .flex_col()
            .items_end()
            .children((1..=line_count).map(|n| {
                div()
                    .text_xs()
                    .text_color(rgb(0x555555))
                    .child(n.to_string())
            }))
    }

    /// Render the text content with cursor visualization and syntax highlighting.
    fn render_content(&self, window: &Window) -> impl IntoElement {
        let is_focused = self.focus_handle.is_focused(window);

        if self.text.is_empty() {
            return div()
                .text_color(rgb(0x555555))
                .child(self.placeholder.clone());
        }

        // For masked or single-line inputs, no syntax highlighting
        if self.masked || !self.multiline {
            let (before_cursor, after_cursor) = if self.masked {
                let chars_before = self.text[..self.cursor].chars().count();
                let chars_after = self.text[self.cursor..].chars().count();
                ("*".repeat(chars_before), "*".repeat(chars_after))
            } else {
                (
                    self.text[..self.cursor].to_string(),
                    self.text[self.cursor..].to_string(),
                )
            };

            if !is_focused {
                return div().child(format!("{before_cursor}{after_cursor}"));
            }

            return div()
                .flex()
                .flex_row()
                .child(div().child(before_cursor))
                .child(
                    div()
                        .w(px(1.5))
                        .h(px(16.0))
                        .bg(rgb(0x4fc1ff))
                        .flex_shrink_0(),
                )
                .child(div().child(after_cursor));
        }

        // Multiline mode with syntax highlighting
        if !is_focused {
            return self.render_highlighted_text(&self.text, 0);
        }

        // Focused: highlighted before + cursor + highlighted after
        let before = &self.text[..self.cursor];
        let after = &self.text[self.cursor..];

        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .child(self.render_highlighted_text(before, 0))
            .child(
                div()
                    .w(px(1.5))
                    .h(px(16.0))
                    .bg(rgb(0x4fc1ff))
                    .flex_shrink_0(),
            )
            .child(self.render_highlighted_text(after, self.cursor))
    }

    /// Render a text slice with SQL syntax highlighting.
    ///
    /// Highlights are computed on the full `self.text` and then filtered
    /// to the byte range `[offset, offset + text.len())`.
    fn render_highlighted_text(&self, text: &str, offset: usize) -> Div {
        if text.is_empty() {
            return div();
        }

        let ranges = highlight_sql(&self.text);
        let start = offset;
        let end = offset + text.len();

        let mut container = div().flex().flex_row().flex_wrap();
        let mut pos = start;

        for range in &ranges {
            if range.end <= start || range.start >= end {
                continue;
            }
            let r_start = range.start.max(start);
            let r_end = range.end.min(end);

            // Gap before this highlight (default text color)
            if pos < r_start {
                container = container.child(
                    div()
                        .text_color(rgb(0xd4d4d4))
                        .child(self.text[pos..r_start].to_string()),
                );
            }

            let color = token_color(range.kind);
            container = container.child(
                div()
                    .text_color(color)
                    .child(self.text[r_start..r_end].to_string()),
            );
            pos = r_end;
        }

        // Remaining text after last highlight
        if pos < end {
            container = container.child(
                div()
                    .text_color(rgb(0xd4d4d4))
                    .child(self.text[pos..end].to_string()),
            );
        }

        container
    }
}

/// Map a token kind to its display color (VS Code dark theme inspired).
fn token_color(kind: TokenKind) -> Hsla {
    match kind {
        TokenKind::Keyword => rgb(0x569cd6).into(),    // Blue
        TokenKind::String => rgb(0xce9178).into(),     // Orange/brown
        TokenKind::Number => rgb(0xb5cea8).into(),     // Light green
        TokenKind::Comment => rgb(0x6a9955).into(),    // Green
        TokenKind::Operator => rgb(0xd4d4d4).into(),   // Default
        TokenKind::Identifier => rgb(0x9cdcfe).into(), // Light blue
    }
}

/// Events emitted by TextInput.
#[derive(Debug, Clone)]
pub enum TextInputEvent {
    /// Text content changed.
    Changed(String),
    /// Enter pressed in single-line mode.
    Submit(String),
}

impl EventEmitter<TextInputEvent> for TextInput {}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_focused = self.focus_handle.is_focused(window);

        let mut container = div()
            .id("text-input")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
            .size_full()
            .font_family("Monaco")
            .text_sm()
            .text_color(rgb(0xd4d4d4))
            .bg(if is_focused {
                rgb(0x1e1e1e)
            } else {
                rgb(0x252525)
            })
            .overflow_hidden();

        if self.multiline {
            // Multiline: flex row with gutter + content
            container = container
                .flex()
                .flex_row()
                .child(self.render_line_numbers())
                .child(div().flex_1().p_2().child(self.render_content(window)));
        } else {
            // Single-line: border + content
            container = container
                .p_2()
                .border_1()
                .border_color(if is_focused {
                    rgb(0x4fc1ff)
                } else {
                    rgb(0x3e3e3e)
                })
                .rounded_sm()
                .child(self.render_content(window));
        }

        container
    }
}
