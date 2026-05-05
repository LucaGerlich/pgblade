use gpui::*;

use crate::components::TextInput;

pub struct EditorArea {
    input: Entity<TextInput>,
}

impl EditorArea {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextInput::new(cx, true)
                .with_placeholder("-- Type your SQL here\n-- Press Cmd+Enter to execute")
        });

        Self { input }
    }

    pub fn text(&self, cx: &App) -> String {
        self.input.read(cx).text().to_string()
    }

    #[allow(dead_code)]
    pub fn focus(&self, window: &mut Window, cx: &App) {
        self.input.read(cx).focus(window);
    }
}

impl Render for EditorArea {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("editor-area")
            .size_full()
            .child(self.input.clone())
    }
}
