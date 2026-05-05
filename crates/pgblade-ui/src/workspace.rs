use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::actions::{
    CloseTab, ExecuteQuery, NewConnection, NewTab, ToggleCommandPalette, ToggleSidebar,
};
use crate::controller::AppController;
use crate::modals::command_palette::{CommandPalette, CommandPaletteEvent};
use crate::modals::connection_modal::{ConnectionModal, ConnectionModalEvent};
use crate::modals::write_confirm::{WriteConfirmEvent, WriteConfirmModal};
use crate::panes::editor_area::EditorEvent;
use crate::panes::{EditorArea, ResultArea, SchemaSidebar, SidebarEvent, StatusBar, Toolbar};

use pgblade_core::connection::ConnectionState;
use pgblade_core::event::AppEvent;
use pgblade_core::query::QueryState;
use pgblade_core::security::CredentialStore;

pub struct Workspace {
    controller: Entity<AppController>,
    sidebar: Entity<SchemaSidebar>,
    editor_area: Entity<EditorArea>,
    result_area: Entity<ResultArea>,
    toolbar: Entity<Toolbar>,
    status_bar: Entity<StatusBar>,
    connection_modal: Option<Entity<ConnectionModal>>,
    write_confirm_modal: Option<Entity<WriteConfirmModal>>,
    command_palette: Option<Entity<CommandPalette>>,
    sidebar_visible: bool,
}

impl Workspace {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let controller = cx.new(|_| AppController::new());
        let sidebar = cx.new(SchemaSidebar::new);
        let editor_area = cx.new(EditorArea::new);
        let result_area = cx.new(ResultArea::new);
        let toolbar = cx.new(|_| Toolbar::new());
        let status_bar = cx.new(|_| StatusBar::new());

        // Subscribe to controller events
        cx.subscribe(&controller, Self::handle_app_event).detach();

        // Subscribe to sidebar events
        cx.subscribe(&sidebar, Self::handle_sidebar_event).detach();

        // Subscribe to editor area events (Execute from SqlEditor)
        cx.subscribe(&editor_area, Self::handle_editor_event)
            .detach();

        // Load saved connections and history on startup
        controller.update(cx, |c, cx| {
            c.load_saved_connections(cx);
            c.load_history(cx);
        });

        // Show connection modal on startup
        let modal = cx.new(ConnectionModal::new);
        cx.subscribe(&modal, Self::handle_modal_event).detach();
        let connection_modal = Some(modal);

        Self {
            controller,
            sidebar,
            editor_area,
            result_area,
            toolbar,
            status_bar,
            connection_modal,
            write_confirm_modal: None,
            command_palette: None,
            sidebar_visible: true,
        }
    }

    fn toggle_sidebar(
        &mut self,
        _action: &ToggleSidebar,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sidebar_visible = !self.sidebar_visible;
        cx.notify();
    }

    fn handle_new_connection(
        &mut self,
        _action: &NewConnection,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.connection_modal.is_none() {
            let modal = cx.new(ConnectionModal::new);
            cx.subscribe(&modal, Self::handle_modal_event).detach();
            self.connection_modal = Some(modal);
            cx.notify();
        }
    }

    fn handle_toggle_command_palette(
        &mut self,
        _action: &ToggleCommandPalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.command_palette.is_some() {
            self.command_palette = None;
        } else {
            let palette = cx.new(CommandPalette::new);
            cx.subscribe(&palette, Self::handle_command_palette_event)
                .detach();
            palette.read(cx).focus(window);
            self.command_palette = Some(palette);
        }
        cx.notify();
    }

    fn handle_command_palette_event(
        &mut self,
        _palette: Entity<CommandPalette>,
        event: &CommandPaletteEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            CommandPaletteEvent::Selected(id) => match id.as_str() {
                "execute_query" => {
                    let sql = self.editor_area.read(cx).text(cx);
                    if !sql.trim().is_empty() {
                        self.result_area.update(cx, |r, cx| r.set_loading(cx));
                        self.controller.update(cx, |c, cx| c.execute_query(sql, cx));
                    }
                }
                "new_connection" if self.connection_modal.is_none() => {
                    let modal = cx.new(ConnectionModal::new);
                    cx.subscribe(&modal, Self::handle_modal_event).detach();
                    self.connection_modal = Some(modal);
                }
                "disconnect" => {
                    self.controller.update(cx, |c, cx| c.disconnect(cx));
                }
                "toggle_sidebar" => {
                    self.sidebar_visible = !self.sidebar_visible;
                }
                "toggle_read_only" => {
                    self.controller
                        .update(cx, |c, cx| c.toggle_write_policy(cx));
                }
                _ => {}
            },
            CommandPaletteEvent::Dismissed => {}
        }
        self.command_palette = None;
        cx.notify();
    }

    fn handle_modal_event(
        &mut self,
        _modal: Entity<ConnectionModal>,
        event: &ConnectionModalEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            ConnectionModalEvent::Connect {
                profile,
                password,
                save,
            } => {
                let profile_clone = profile.clone();
                let password_clone = password.clone();
                let should_save = *save;

                self.controller.update(cx, |controller, cx| {
                    if should_save {
                        controller.save_connection(&profile_clone, &password_clone, cx);
                    }
                    controller.connect(profile_clone, password_clone, cx);
                });
                self.connection_modal = None;
                cx.notify();
            }
            ConnectionModalEvent::Dismiss => {
                self.connection_modal = None;
                cx.notify();
            }
        }
    }

    fn handle_sidebar_event(
        &mut self,
        _sidebar: Entity<SchemaSidebar>,
        event: &SidebarEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            SidebarEvent::ConnectTo(profile) => {
                // Get password from keychain
                let password = self
                    .controller
                    .read(cx)
                    .credential_store()
                    .retrieve(&profile.keychain_service_key())
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                let profile_clone = profile.clone();
                self.controller.update(cx, |c, cx| {
                    c.connect(profile_clone, password, cx);
                });
            }
            SidebarEvent::PreviewTable { schema, table } => {
                let schema_clone = schema.clone();
                let table_clone = table.clone();

                // Show loading state in result area
                self.result_area.update(cx, |result, cx| {
                    result.set_loading(cx);
                });

                self.controller.update(cx, |c, cx| {
                    c.preview_table(schema_clone, table_clone, cx);
                });
            }
        }
    }

    fn handle_editor_event(
        &mut self,
        _editor: Entity<EditorArea>,
        event: &EditorEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            EditorEvent::Execute(sql) => {
                if sql.trim().is_empty() {
                    return;
                }
                self.result_area
                    .update(cx, |result, cx| result.set_loading(cx));
                let sql = sql.clone();
                self.controller.update(cx, |c, cx| c.execute_query(sql, cx));
            }
            EditorEvent::TabChanged(_) => {}
        }
    }

    fn handle_write_confirm_event(
        &mut self,
        _modal: Entity<WriteConfirmModal>,
        event: &WriteConfirmEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            WriteConfirmEvent::Confirmed(sql) => {
                let sql_clone = sql.clone();

                // Show loading state in result area
                self.result_area.update(cx, |result, cx| {
                    result.set_loading(cx);
                });

                self.controller.update(cx, |c, cx| {
                    c.force_execute_query(sql_clone, cx);
                });
            }
            WriteConfirmEvent::Cancelled => {}
        }
        self.write_confirm_modal = None;
        cx.notify();
    }

    fn handle_new_tab(&mut self, _action: &NewTab, _window: &mut Window, cx: &mut Context<Self>) {
        self.editor_area.update(cx, |editor, cx| editor.new_tab(cx));
    }

    fn handle_close_tab(
        &mut self,
        _action: &CloseTab,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let active_id = self.editor_area.read(cx).active_tab_id();
        self.editor_area
            .update(cx, |editor, cx| editor.close_tab(active_id, cx));
    }

    fn handle_execute_query(
        &mut self,
        _action: &ExecuteQuery,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sql = self.editor_area.read(cx).text(cx);
        if sql.trim().is_empty() {
            return;
        }

        // Show loading state in result area
        self.result_area.update(cx, |result, cx| {
            result.set_loading(cx);
        });

        // Execute via controller
        self.controller.update(cx, |controller, cx| {
            controller.execute_query(sql, cx);
        });
    }

    fn handle_app_event(
        &mut self,
        _controller: Entity<AppController>,
        event: &AppEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            AppEvent::ConnectionStateChanged(state) => {
                tracing::info!(?state, "connection state changed");
                let environment = self
                    .controller
                    .read(cx)
                    .active_profile()
                    .map(|p| p.environment);
                self.toolbar.update(cx, |toolbar, cx| {
                    toolbar.set_connection_state(state.clone(), cx);
                    toolbar.set_environment(environment, cx);
                });
                let status = match state {
                    ConnectionState::Connected {
                        database,
                        server_version,
                        ..
                    } => {
                        format!("Connected to {database} — {server_version}")
                    }
                    ConnectionState::Disconnected => "Disconnected".to_string(),
                    ConnectionState::Connecting { .. } => "Connecting...".to_string(),
                    ConnectionState::Failed { error, .. } => format!("Connection failed: {error}"),
                };
                self.status_bar.update(cx, |sb, cx| {
                    sb.set_status(status, cx);
                });
                cx.notify();
            }
            AppEvent::QueryStateChanged { state, .. } => {
                match state {
                    QueryState::Executing { .. } => {
                        self.status_bar.update(cx, |sb, cx| {
                            sb.set_status("Executing...".to_string(), cx);
                        });
                    }
                    QueryState::Failed { error, .. } => {
                        self.result_area.update(cx, |result, cx| {
                            result.set_error(error.clone(), cx);
                        });
                        self.status_bar.update(cx, |sb, cx| {
                            sb.set_status(format!("Error: {error}"), cx);
                        });
                    }
                    QueryState::Cancelled { .. } => {
                        self.status_bar.update(cx, |sb, cx| {
                            sb.set_status("Query cancelled".to_string(), cx);
                        });
                    }
                    _ => {}
                }
                cx.notify();
            }
            AppEvent::ResultPageReady { page, .. } => {
                let duration_ms = self
                    .controller
                    .read(cx)
                    .active_query_state()
                    .and_then(|s| s.elapsed_ms())
                    .unwrap_or(0);

                let row_count = page.rows.len();
                let row_word = if row_count == 1 { "row" } else { "rows" };
                self.status_bar.update(cx, |sb, cx| {
                    sb.set_status(format!("{row_count} {row_word} in {duration_ms}ms"), cx);
                });

                self.result_area.update(cx, |result, cx| {
                    result.set_results(page.columns.clone(), page.rows.clone(), duration_ms, cx);
                });
            }
            AppEvent::WritePolicyChanged(_) => {
                cx.notify();
            }
            AppEvent::SchemaLoaded(tree) => {
                self.sidebar.update(cx, |sidebar, cx| {
                    sidebar.set_schema_tree(tree, cx);
                });
            }
            AppEvent::SavedConnectionsLoaded(connections) => {
                self.sidebar.update(cx, |sidebar, cx| {
                    sidebar.set_saved_connections(connections.clone(), cx);
                });
            }
            AppEvent::WriteConfirmationNeeded {
                sql,
                classification,
            } => {
                let modal = cx.new(|cx| {
                    WriteConfirmModal::new(sql.clone(), classification.label().to_string(), cx)
                });
                cx.subscribe(&modal, Self::handle_write_confirm_event)
                    .detach();
                self.write_confirm_modal = Some(modal);
                cx.notify();
            }
            AppEvent::HistoryLoaded(_) => {
                // TODO: update history panel when we build it
                cx.notify();
            }
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_connection_modal = self.connection_modal.is_some();
        let has_write_confirm_modal = self.write_confirm_modal.is_some();
        let has_command_palette = self.command_palette.is_some();

        div()
            .id("workspace")
            .key_context("Workspace")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1a1a1a))
            .text_color(rgb(0xcccccc))
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_action(cx.listener(Self::handle_execute_query))
            .on_action(cx.listener(Self::handle_new_connection))
            .on_action(cx.listener(Self::handle_toggle_command_palette))
            .on_action(cx.listener(Self::handle_new_tab))
            .on_action(cx.listener(Self::handle_close_tab))
            // Toolbar
            .child(self.toolbar.clone())
            // Main content area
            .child(
                div()
                    .id("main-content")
                    .flex_1()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    // Sidebar (conditionally visible)
                    .when(self.sidebar_visible, |this| {
                        this.child(
                            div()
                                .id("sidebar-container")
                                .w(px(260.0))
                                .flex_shrink_0()
                                .border_r_1()
                                .border_color(rgb(0x333333))
                                .bg(rgb(0x1e1e1e))
                                .child(self.sidebar.clone()),
                        )
                    })
                    // Center panel: editor (top) + results (bottom)
                    .child(
                        div()
                            .id("center-panel")
                            .flex_1()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            // Editor area
                            .child(
                                div()
                                    .id("editor-container")
                                    .flex_1()
                                    .min_h(px(200.0))
                                    .border_b_1()
                                    .border_color(rgb(0x333333))
                                    .child(self.editor_area.clone()),
                            )
                            // Result area
                            .child(
                                div()
                                    .id("result-container")
                                    .flex_1()
                                    .min_h(px(150.0))
                                    .child(self.result_area.clone()),
                            ),
                    ),
            )
            // Status bar
            .child(self.status_bar.clone())
            // Connection modal overlay (if active)
            .when(has_connection_modal, |this| {
                if let Some(modal) = &self.connection_modal {
                    this.child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .child(modal.clone()),
                    )
                } else {
                    this
                }
            })
            // Write confirmation modal overlay (if active)
            .when(has_write_confirm_modal, |this| {
                if let Some(modal) = &self.write_confirm_modal {
                    this.child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .child(modal.clone()),
                    )
                } else {
                    this
                }
            })
            // Command palette overlay (if active)
            .when(has_command_palette, |this| {
                if let Some(palette) = &self.command_palette {
                    this.child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .child(palette.clone()),
                    )
                } else {
                    this
                }
            })
    }
}
