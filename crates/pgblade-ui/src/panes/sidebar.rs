use gpui::prelude::FluentBuilder;
use gpui::*;

use pgblade_core::connection::ConnectionProfile;
use pgblade_core::schema::{SchemaTree, TableKind};

use crate::components::tree_view::{TreeNode, TreeView, TreeViewEvent};

pub struct SchemaSidebar {
    tree: Entity<TreeView>,
    saved_connections: Vec<ConnectionProfile>,
    connected: bool,
}

/// Events emitted by the sidebar.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SidebarEvent {
    /// User clicked a saved connection to connect.
    ConnectTo(ConnectionProfile),
    /// User clicked a table to preview.
    PreviewTable { schema: String, table: String },
}

impl EventEmitter<SidebarEvent> for SchemaSidebar {}

#[allow(dead_code)]
impl SchemaSidebar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let tree = cx.new(|_| TreeView::new());

        // Subscribe to tree click events
        cx.subscribe(&tree, |_this, _tree, event: &TreeViewEvent, cx| {
            let TreeViewEvent::NodeClicked(id) = event;
            // id format: "table:schema.table_name"
            if let Some(rest) = id.strip_prefix("table:")
                && let Some((schema, table)) = rest.split_once('.')
            {
                cx.emit(SidebarEvent::PreviewTable {
                    schema: schema.to_string(),
                    table: table.to_string(),
                });
            }
        })
        .detach();

        Self {
            tree,
            saved_connections: Vec::new(),
            connected: false,
        }
    }

    pub fn set_saved_connections(
        &mut self,
        connections: Vec<ConnectionProfile>,
        cx: &mut Context<Self>,
    ) {
        self.saved_connections = connections;
        cx.notify();
    }

    pub fn set_schema_tree(&mut self, schema_tree: &SchemaTree, cx: &mut Context<Self>) {
        self.connected = true;
        let nodes = schema_tree_to_nodes(schema_tree);
        self.tree.update(cx, |tree, cx| {
            tree.set_nodes(nodes, cx);
        });
        cx.notify();
    }

    pub fn clear_schema(&mut self, cx: &mut Context<Self>) {
        self.connected = false;
        self.tree.update(cx, |tree, cx| {
            tree.set_nodes(Vec::new(), cx);
        });
        cx.notify();
    }

    fn render_connections_section(&self, cx: &mut Context<Self>) -> Div {
        let mut section = div().w_full().flex().flex_col().p_2().child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(0x888888))
                .mb_2()
                .child("CONNECTIONS"),
        );

        if self.saved_connections.is_empty() {
            section = section.child(
                div()
                    .text_xs()
                    .text_color(rgb(0x555555))
                    .child("No saved connections"),
            );
        } else {
            for conn in &self.saved_connections {
                let profile = conn.clone();
                let env_color = match conn.environment {
                    pgblade_core::connection::Environment::Production => rgb(0xf14c4c),
                    pgblade_core::connection::Environment::Staging => rgb(0xccaa00),
                    _ => rgb(0x4ec94e),
                };
                let conn_id = SharedString::from(format!("conn-{}", conn.id.0));
                let conn_name = conn.name.clone();
                section = section.child(
                    div()
                        .id(conn_id)
                        .h(px(24.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .px_2()
                        .rounded_sm()
                        .hover(|s| s.bg(rgb(0x2a2a2a)))
                        .cursor_pointer()
                        .on_click(cx.listener(move |_this, _, _window, cx| {
                            cx.emit(SidebarEvent::ConnectTo(profile.clone()));
                        }))
                        .child(
                            div()
                                .w(px(6.0))
                                .h(px(6.0))
                                .rounded_full()
                                .bg(env_color)
                                .mr_2()
                                .flex_shrink_0(),
                        )
                        .child(div().text_xs().text_color(rgb(0xcccccc)).child(conn_name)),
                );
            }
        }

        section
    }
}

impl Render for SchemaSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("schema-sidebar")
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            // Connections section
            .child(self.render_connections_section(cx))
            // Divider
            .child(div().h(px(1.0)).w_full().bg(rgb(0x333333)))
            // Schema tree section
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .p_2()
                    .overflow_hidden()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x888888))
                            .mb_2()
                            .child("SCHEMA"),
                    )
                    .when(!self.connected, |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x555555))
                                .child("Connect to browse schema"),
                        )
                    })
                    .when(self.connected, |this| this.child(self.tree.clone())),
            )
    }
}

/// Convert a SchemaTree into tree view nodes.
#[allow(dead_code)]
fn schema_tree_to_nodes(tree: &SchemaTree) -> Vec<TreeNode> {
    tree.schemas
        .iter()
        .map(|schema| {
            let table_nodes: Vec<TreeNode> = schema
                .tables
                .iter()
                .map(|table| {
                    let kind_icon = match table.info.kind {
                        TableKind::Table => "T",
                        TableKind::View => "V",
                        TableKind::MaterializedView => "M",
                    };

                    let column_nodes: Vec<TreeNode> = table
                        .columns
                        .iter()
                        .map(|col| {
                            let pk = if col.is_primary_key { "PK " } else { "" };
                            let null = if col.nullable { "?" } else { "" };
                            TreeNode {
                                id: format!(
                                    "col:{}.{}.{}",
                                    table.info.schema, table.info.name, col.name
                                ),
                                label: format!("{pk}{} {}{null}", col.name, col.data_type),
                                icon: None,
                                children: Vec::new(),
                                depth: 2,
                            }
                        })
                        .collect();

                    TreeNode {
                        id: format!("table:{}.{}", table.info.schema, table.info.name),
                        label: table.info.name.clone(),
                        icon: Some(kind_icon),
                        children: column_nodes,
                        depth: 1,
                    }
                })
                .collect();

            TreeNode {
                id: format!("schema:{}", schema.info.name),
                label: schema.info.name.clone(),
                icon: Some("S"),
                children: table_nodes,
                depth: 0,
            }
        })
        .collect()
}
