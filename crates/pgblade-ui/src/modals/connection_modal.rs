use gpui::prelude::FluentBuilder;
use gpui::*;

use pgblade_core::connection::{ConnectionId, ConnectionProfile, Environment, SslMode};

use crate::components::TextInput;

/// Connection form modal dialog.
pub struct ConnectionModal {
    host: Entity<TextInput>,
    port: Entity<TextInput>,
    database: Entity<TextInput>,
    username: Entity<TextInput>,
    password: Entity<TextInput>,
    environment: Environment,
    save_connection: bool,
    focus_handle: FocusHandle,
}

/// Events emitted by ConnectionModal.
#[derive(Debug, Clone)]
pub enum ConnectionModalEvent {
    /// User submitted the connection form.
    Connect {
        profile: ConnectionProfile,
        password: String,
        save: bool,
    },
    /// User dismissed the modal.
    Dismiss,
}

impl EventEmitter<ConnectionModalEvent> for ConnectionModal {}

impl ConnectionModal {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let host = cx.new(|cx| {
            let mut input = TextInput::new(cx, false).with_placeholder("localhost");
            input.set_text("localhost".to_string(), cx);
            input
        });
        let port = cx.new(|cx| {
            let mut input = TextInput::new(cx, false).with_placeholder("5432");
            input.set_text("5432".to_string(), cx);
            input
        });
        let database = cx.new(|cx| TextInput::new(cx, false).with_placeholder("postgres"));
        let username = cx.new(|cx| TextInput::new(cx, false).with_placeholder("postgres"));
        let password = cx.new(|cx| {
            TextInput::new(cx, false)
                .with_placeholder("password")
                .with_masked(true)
        });

        Self {
            host,
            port,
            database,
            username,
            password,
            environment: Environment::Local,
            save_connection: true,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn from_profile(profile: &ConnectionProfile, cx: &mut Context<Self>) -> Self {
        let host = cx.new(|cx| {
            let mut input = TextInput::new(cx, false).with_placeholder("localhost");
            input.set_text(profile.host.clone(), cx);
            input
        });
        let port = cx.new(|cx| {
            let mut input = TextInput::new(cx, false).with_placeholder("5432");
            input.set_text(profile.port.to_string(), cx);
            input
        });
        let database = cx.new(|cx| {
            let mut input = TextInput::new(cx, false).with_placeholder("postgres");
            input.set_text(profile.database.clone(), cx);
            input
        });
        let username = cx.new(|cx| {
            let mut input = TextInput::new(cx, false).with_placeholder("postgres");
            input.set_text(profile.username.clone(), cx);
            input
        });
        let password = cx.new(|cx| {
            TextInput::new(cx, false)
                .with_placeholder("password")
                .with_masked(true)
        });

        Self {
            host,
            port,
            database,
            username,
            password,
            environment: profile.environment,
            save_connection: true,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn focus_first_field(&self, window: &mut Window) {
        self.focus_handle.focus(window);
    }

    fn submit(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let host = self.host.read(cx).text().to_string();
        let port_str = self.port.read(cx).text().to_string();
        let database = self.database.read(cx).text().to_string();
        let username = self.username.read(cx).text().to_string();
        let password = self.password.read(cx).text().to_string();

        let port: u16 = port_str.parse().unwrap_or(5432);

        let profile = ConnectionProfile {
            id: ConnectionId::new(),
            name: format!("{database}@{host}"),
            host,
            port,
            database,
            username,
            environment: self.environment,
            ssl_mode: SslMode::Disable,
            read_only_default: self.environment.is_production(),
        };

        cx.emit(ConnectionModalEvent::Connect {
            profile,
            password,
            save: self.save_connection,
        });
    }

    fn render_field(&self, label: &str, input: &Entity<TextInput>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .mb_3()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0xaaaaaa))
                    .child(label.to_string()),
            )
            .child(div().h(px(32.0)).child(input.clone()))
    }

    fn render_env_selector(&self) -> impl IntoElement {
        let envs = [
            Environment::Local,
            Environment::Development,
            Environment::Staging,
            Environment::Production,
        ];

        div()
            .flex()
            .flex_col()
            .gap_1()
            .mb_3()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0xaaaaaa))
                    .child("Environment"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .children(envs.into_iter().map(|env| {
                        let is_selected = env == self.environment;
                        let bg = if is_selected {
                            rgb(0x4fc1ff)
                        } else {
                            rgb(0x333333)
                        };
                        let text_color = if is_selected {
                            rgb(0x1a1a1a)
                        } else {
                            rgb(0xaaaaaa)
                        };

                        div()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(bg)
                            .text_xs()
                            .text_color(text_color)
                            .cursor_pointer()
                            .child(env.label().to_string())
                    })),
            )
    }
}

impl Render for ConnectionModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Full-screen backdrop — captures all mouse events to prevent click-through
        div()
            .id("connection-modal-backdrop")
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x000000aa))
            .on_mouse_down(MouseButton::Left, |_, _, _| {})
            .on_mouse_down(MouseButton::Right, |_, _, _| {})
            // Modal card
            .child(
                div()
                    .id("connection-modal")
                    .track_focus(&self.focus_handle)
                    .on_key_down(cx.listener(|_this, event: &KeyDownEvent, _window, cx| {
                        if event.keystroke.key.as_str() == "escape" {
                            cx.emit(ConnectionModalEvent::Dismiss);
                        }
                    }))
                    .w(px(400.0))
                    .p_6()
                    .bg(rgb(0x252525))
                    .border_1()
                    .border_color(rgb(0x444444))
                    .rounded_md()
                    .flex()
                    .flex_col()
                    // Title bar with close button
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .mb_4()
                            .child(
                                div()
                                    .flex_1()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xeeeeee))
                                    .child("Connect to PostgreSQL"),
                            )
                            .child(
                                div()
                                    .id("close-modal-btn")
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .text_sm()
                                    .text_color(rgb(0x888888))
                                    .hover(|s| s.text_color(rgb(0xeeeeee)).bg(rgb(0x333333)))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|_this, _, _window, cx| {
                                        cx.emit(ConnectionModalEvent::Dismiss);
                                    }))
                                    .child("x"),
                            ),
                    )
                    // Fields
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(div().flex_1().child(self.render_field("Host", &self.host)))
                            .child(
                                div()
                                    .w(px(80.0))
                                    .child(self.render_field("Port", &self.port)),
                            ),
                    )
                    .child(self.render_field("Database", &self.database))
                    .child(self.render_field("Username", &self.username))
                    .child(self.render_field("Password", &self.password))
                    .child(self.render_env_selector())
                    // Save connection checkbox
                    .child(
                        div()
                            .id("save-connection-toggle")
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .mb_3()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.save_connection = !this.save_connection;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .w(px(14.0))
                                    .h(px(14.0))
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(rgb(0x555555))
                                    .bg(if self.save_connection {
                                        rgb(0x4fc1ff)
                                    } else {
                                        rgb(0x252525)
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(self.save_connection, |this| {
                                        this.child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0x1a1a1a))
                                                .child("\u{2713}"),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0xaaaaaa))
                                    .child("Save connection"),
                            ),
                    )
                    // Connect button
                    .child(
                        div().mt_2().flex().justify_end().child(
                            div()
                                .id("connect-button")
                                .px_4()
                                .py_2()
                                .bg(rgb(0x4fc1ff))
                                .text_color(rgb(0x1a1a1a))
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .rounded_sm()
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.submit(window, cx);
                                }))
                                .child("Connect"),
                        ),
                    ),
            )
    }
}
