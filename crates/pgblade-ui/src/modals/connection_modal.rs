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
    focus_handle: FocusHandle,
}

/// Events emitted by ConnectionModal.
#[derive(Debug, Clone)]
pub enum ConnectionModalEvent {
    /// User submitted the connection form.
    Connect {
        profile: ConnectionProfile,
        password: String,
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
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &App) {
        self.host.read(cx).focus(window);
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

        cx.emit(ConnectionModalEvent::Connect { profile, password });
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
        // Full-screen backdrop
        div()
            .id("connection-modal-backdrop")
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x000000aa))
            // Modal card
            .child(
                div()
                    .id("connection-modal")
                    .track_focus(&self.focus_handle)
                    .on_action(cx.listener(|_this, _: &crate::actions::Quit, _window, cx| {
                        cx.emit(ConnectionModalEvent::Dismiss);
                    }))
                    .w(px(400.0))
                    .p_6()
                    .bg(rgb(0x252525))
                    .border_1()
                    .border_color(rgb(0x444444))
                    .rounded_md()
                    .flex()
                    .flex_col()
                    // Title
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0xeeeeee))
                            .mb_4()
                            .child("Connect to PostgreSQL"),
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
