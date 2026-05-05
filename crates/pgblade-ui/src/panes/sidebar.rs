use gpui::*;

pub struct SchemaSidebar;

impl SchemaSidebar {
    pub fn new() -> Self {
        Self
    }
}

impl Render for SchemaSidebar {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("schema-sidebar")
            .size_full()
            .p_2()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0xaaaaaa))
                    .child("Schema Browser"),
            )
            .child(
                div()
                    .flex_1()
                    .mt_2()
                    .text_xs()
                    .text_color(rgb(0x666666))
                    .child("Connect to a database to browse schema"),
            )
    }
}
