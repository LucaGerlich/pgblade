use gpui::prelude::FluentBuilder;
use gpui::*;

use pgblade_core::connection::{ConnectionState, Environment};
use pgblade_core::safety::WritePolicy;

#[allow(dead_code)]
pub struct Toolbar {
    connection_state: ConnectionState,
    write_policy: WritePolicy,
    environment: Option<Environment>,
}

#[allow(dead_code)]
impl Toolbar {
    pub fn new() -> Self {
        Self {
            connection_state: ConnectionState::Disconnected,
            write_policy: WritePolicy::ReadOnly,
            environment: None,
        }
    }

    pub fn set_connection_state(&mut self, state: ConnectionState, cx: &mut Context<Self>) {
        self.connection_state = state;
        cx.notify();
    }

    pub fn set_write_policy(&mut self, policy: WritePolicy, cx: &mut Context<Self>) {
        self.write_policy = policy;
        cx.notify();
    }

    pub fn set_environment(&mut self, env: Option<Environment>, cx: &mut Context<Self>) {
        self.environment = env;
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
                let ro = if *read_only { "[RO] " } else { "" };
                (format!("{ro}{database}"), 0x4ec94e)
            }
            ConnectionState::Failed { error, .. } => (format!("Failed: {error}"), 0xf14c4c),
        }
    }

    fn environment_badge(&self) -> Option<(&'static str, u32)> {
        self.environment.map(|env| match env {
            Environment::Production => ("PROD", 0xf14c4c),
            Environment::Staging => ("STAGING", 0xccaa00),
            Environment::Development => ("DEV", 0x4ec94e),
            Environment::Local => ("LOCAL", 0x4488cc),
        })
    }
}

impl Render for Toolbar {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let (conn_text, conn_color) = self.connection_label();
        let badge = self.environment_badge();
        let is_connected = self.connection_state.is_connected();

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
            // Environment badge (only when connected)
            .when(is_connected && badge.is_some(), |this| {
                let (label, bg_color) = badge.unwrap_or(("", 0x333333));
                this.child(
                    div()
                        .px_2()
                        .py_1()
                        .mr_2()
                        .rounded_sm()
                        .bg(rgb(bg_color))
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(0x1a1a1a))
                        .child(label),
                )
            })
            // Connection text
            .child(div().text_xs().text_color(rgb(conn_color)).child(conn_text))
    }
}
