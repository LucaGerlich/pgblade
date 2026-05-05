use gpui::*;

use pgblade_core::connection::ConnectionState;

pub struct Toolbar {
    connection_state: ConnectionState,
}

impl Toolbar {
    pub fn new() -> Self {
        Self {
            connection_state: ConnectionState::Disconnected,
        }
    }

    pub fn set_connection_state(&mut self, state: ConnectionState, cx: &mut Context<Self>) {
        self.connection_state = state;
        cx.notify();
    }

    fn connection_label(&self) -> (String, u32) {
        match &self.connection_state {
            ConnectionState::Disconnected => ("Disconnected".to_string(), 0x888888),
            ConnectionState::Connecting { .. } => ("Connecting...".to_string(), 0xccaa00),
            ConnectionState::Connected {
                database,
                read_only,
                ..
            } => {
                let ro = if *read_only { " (read-only)" } else { "" };
                (format!("{database}{ro}"), 0x4ec94e)
            }
            ConnectionState::Failed { error, .. } => (format!("Failed: {error}"), 0xf14c4c),
        }
    }
}

impl Render for Toolbar {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let (conn_text, conn_color) = self.connection_label();

        div()
            .id("toolbar")
            .h(px(40.0))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .px_3()
            .border_b_1()
            .border_color(rgb(0x333333))
            .bg(rgb(0x1e1e1e))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xeeeeee))
                    .child("PgBlade"),
            )
            .child(div().flex_1())
            .child(div().text_xs().text_color(rgb(conn_color)).child(conn_text))
    }
}
