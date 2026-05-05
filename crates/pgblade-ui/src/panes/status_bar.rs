use gpui::*;

pub struct StatusBar {
    status: String,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            status: "Ready".to_string(),
        }
    }

    pub fn set_status(&mut self, status: String, cx: &mut Context<Self>) {
        self.status = status;
        cx.notify();
    }
}

impl Render for StatusBar {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("status-bar")
            .h(px(24.0))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .px_3()
            .border_t_1()
            .border_color(rgb(0x333333))
            .bg(rgb(0x1e1e1e))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x888888))
                    .child(self.status.clone()),
            )
    }
}
