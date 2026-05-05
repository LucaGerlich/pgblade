use gpui::prelude::FluentBuilder;
use gpui::*;

/// A command entry in the palette.
#[derive(Debug, Clone)]
pub struct CommandItem {
    pub id: &'static str,
    pub label: &'static str,
    pub shortcut: &'static str,
}

/// Events emitted by the command palette.
#[derive(Debug, Clone)]
pub enum CommandPaletteEvent {
    /// A command was selected by the user.
    Selected(String),
    /// The palette was dismissed without selection.
    Dismissed,
}

impl EventEmitter<CommandPaletteEvent> for CommandPalette {}

/// A searchable command palette overlay (Cmd+K).
pub struct CommandPalette {
    query: String,
    commands: Vec<CommandItem>,
    filtered: Vec<usize>,
    selected_index: usize,
    focus_handle: FocusHandle,
}

impl CommandPalette {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let commands = vec![
            CommandItem {
                id: "execute_query",
                label: "Execute Query",
                shortcut: "Cmd+Enter",
            },
            CommandItem {
                id: "new_connection",
                label: "New Connection",
                shortcut: "Cmd+N",
            },
            CommandItem {
                id: "disconnect",
                label: "Disconnect",
                shortcut: "",
            },
            CommandItem {
                id: "toggle_sidebar",
                label: "Toggle Sidebar",
                shortcut: "Cmd+B",
            },
            CommandItem {
                id: "toggle_read_only",
                label: "Toggle Read-Only Mode",
                shortcut: "",
            },
            CommandItem {
                id: "cancel_query",
                label: "Cancel Query",
                shortcut: "",
            },
            CommandItem {
                id: "show_history",
                label: "Show Query History",
                shortcut: "",
            },
        ];
        let filtered: Vec<usize> = (0..commands.len()).collect();

        Self {
            query: String::new(),
            commands,
            filtered,
            selected_index: 0,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn focus(&self, window: &mut Window) {
        self.focus_handle.focus(window);
    }

    fn filter_commands(&mut self) {
        let query_lower = self.query.to_lowercase();
        self.filtered = self
            .commands
            .iter()
            .enumerate()
            .filter(|(_, cmd)| {
                query_lower.is_empty() || cmd.label.to_lowercase().contains(&query_lower)
            })
            .map(|(i, _)| i)
            .collect();
        self.selected_index = 0;
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event.keystroke.key.as_str() {
            "escape" => {
                cx.emit(CommandPaletteEvent::Dismissed);
            }
            "enter" => {
                if let Some(&idx) = self.filtered.get(self.selected_index) {
                    let id = self.commands[idx].id.to_string();
                    cx.emit(CommandPaletteEvent::Selected(id));
                }
            }
            "up" => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                    cx.notify();
                }
            }
            "down" => {
                if self.selected_index + 1 < self.filtered.len() {
                    self.selected_index += 1;
                    cx.notify();
                }
            }
            "backspace" => {
                self.query.pop();
                self.filter_commands();
                cx.notify();
            }
            _ => {
                if let Some(ref ch) = event.keystroke.key_char
                    && !event.keystroke.modifiers.platform
                    && !event.keystroke.modifiers.control
                {
                    self.query.push_str(ch);
                    self.filter_commands();
                    cx.notify();
                }
            }
        }
    }
}

impl Render for CommandPalette {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("command-palette-backdrop")
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(80.0))
            .bg(rgba(0x00000088))
            .on_click(cx.listener(|_this, _, _window, cx| {
                cx.emit(CommandPaletteEvent::Dismissed);
            }))
            .child(
                div()
                    .id("command-palette")
                    .track_focus(&self.focus_handle)
                    .on_key_down(cx.listener(Self::handle_key_down))
                    .w(px(500.0))
                    .max_h(px(400.0))
                    .bg(rgb(0x252525))
                    .border_1()
                    .border_color(rgb(0x4fc1ff))
                    .rounded_md()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    // Search input display
                    .child(
                        div()
                            .h(px(40.0))
                            .px_3()
                            .flex()
                            .flex_row()
                            .items_center()
                            .border_b_1()
                            .border_color(rgb(0x333333))
                            .child(div().text_sm().text_color(rgb(0x888888)).mr_2().child(">"))
                            .child(div().text_sm().text_color(rgb(0xd4d4d4)).child(
                                if self.query.is_empty() {
                                    "Type a command...".to_string()
                                } else {
                                    self.query.clone()
                                },
                            )),
                    )
                    // Results list
                    .children(
                        self.filtered
                            .iter()
                            .enumerate()
                            .map(|(visual_idx, &cmd_idx)| {
                                let cmd = &self.commands[cmd_idx];
                                let is_selected = visual_idx == self.selected_index;
                                let bg = if is_selected {
                                    rgb(0x333333)
                                } else {
                                    rgb(0x252525)
                                };
                                let cmd_id = cmd.id.to_string();
                                let label = cmd.label.to_string();
                                let shortcut = cmd.shortcut.to_string();
                                let has_shortcut = !cmd.shortcut.is_empty();

                                div()
                                    .id(SharedString::from(format!("cmd-{}", cmd.id)))
                                    .h(px(32.0))
                                    .px_3()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .bg(bg)
                                    .hover(|s| s.bg(rgb(0x2a2a2a)))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |_this, _, _window, cx| {
                                        cx.emit(CommandPaletteEvent::Selected(cmd_id.clone()));
                                    }))
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_sm()
                                            .text_color(if is_selected {
                                                rgb(0xffffff)
                                            } else {
                                                rgb(0xcccccc)
                                            })
                                            .child(label),
                                    )
                                    .when(has_shortcut, |this| {
                                        this.child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0x666666))
                                                .child(shortcut),
                                        )
                                    })
                            }),
                    ),
            )
    }
}
