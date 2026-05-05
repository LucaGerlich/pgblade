use gpui::prelude::FluentBuilder;
use gpui::*;

use pgblade_core::connection::ConnectionProfile;
use pgblade_core::schema::{SchemaTree, TableEntry};

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

/// Build child nodes for a single table entry (columns, constraints, FKs, indexes, triggers).
fn table_entry_to_node(table: &TableEntry, depth: usize) -> TreeNode {
    let qualified = format!("{}.{}", table.info.schema, table.info.name);
    let mut children = Vec::new();

    // Columns
    let column_nodes: Vec<TreeNode> = table
        .columns
        .iter()
        .map(|col| {
            let pk = if col.is_primary_key { "PK " } else { "" };
            let null = if col.nullable { "?" } else { "" };
            TreeNode {
                id: format!("col:{}.{}", qualified, col.name),
                label: format!("{pk}{} {}{null}", col.name, col.data_type),
                icon: None,
                children: Vec::new(),
                depth: depth + 2,
            }
        })
        .collect();
    children.push(TreeNode {
        id: format!("columns:{qualified}"),
        label: format!("Columns ({})", table.columns.len()),
        icon: None,
        children: column_nodes,
        depth: depth + 1,
    });

    // Constraints
    let constraint_nodes: Vec<TreeNode> = table
        .constraints
        .iter()
        .map(|c| TreeNode {
            id: format!("constraint:{qualified}.{}", c.name),
            label: format!("{} ({})", c.name, c.kind.label()),
            icon: None,
            children: Vec::new(),
            depth: depth + 2,
        })
        .collect();
    children.push(TreeNode {
        id: format!("constraints:{qualified}"),
        label: format!("Constraints ({})", table.constraints.len()),
        icon: None,
        children: constraint_nodes,
        depth: depth + 1,
    });

    // Foreign Keys
    let fk_nodes: Vec<TreeNode> = table
        .foreign_keys
        .iter()
        .map(|fk| TreeNode {
            id: format!("fk:{qualified}.{}", fk.name),
            label: format!(
                "{} -> {}({})",
                fk.name,
                fk.referenced_table,
                fk.referenced_columns.join(", ")
            ),
            icon: None,
            children: Vec::new(),
            depth: depth + 2,
        })
        .collect();
    children.push(TreeNode {
        id: format!("fks:{qualified}"),
        label: format!("Foreign Keys ({})", table.foreign_keys.len()),
        icon: None,
        children: fk_nodes,
        depth: depth + 1,
    });

    // Indexes
    let index_nodes: Vec<TreeNode> = table
        .indexes
        .iter()
        .map(|idx| {
            let unique_str = if idx.is_unique { ", unique" } else { "" };
            TreeNode {
                id: format!("idx:{qualified}.{}", idx.name),
                label: format!("{} ({}{})", idx.name, idx.index_type, unique_str),
                icon: None,
                children: Vec::new(),
                depth: depth + 2,
            }
        })
        .collect();
    children.push(TreeNode {
        id: format!("indexes:{qualified}"),
        label: format!("Indexes ({})", table.indexes.len()),
        icon: None,
        children: index_nodes,
        depth: depth + 1,
    });

    // Triggers
    let trigger_nodes: Vec<TreeNode> = table
        .triggers
        .iter()
        .map(|t| TreeNode {
            id: format!("trigger:{qualified}.{}", t.name),
            label: format!("{} ({} {})", t.name, t.timing, t.event),
            icon: None,
            children: Vec::new(),
            depth: depth + 2,
        })
        .collect();
    children.push(TreeNode {
        id: format!("triggers:{qualified}"),
        label: format!("Triggers ({})", table.triggers.len()),
        icon: None,
        children: trigger_nodes,
        depth: depth + 1,
    });

    TreeNode {
        id: format!("table:{qualified}"),
        label: table.info.name.clone(),
        icon: None,
        children,
        depth,
    }
}

/// Convert a SchemaTree into tree view nodes with DBeaver-style hierarchy.
fn schema_tree_to_nodes(tree: &SchemaTree) -> Vec<TreeNode> {
    tree.schemas
        .iter()
        .map(|schema| {
            let schema_name = &schema.info.name;
            let mut children = Vec::new();

            // Tables
            let table_nodes: Vec<TreeNode> = schema
                .tables
                .iter()
                .map(|t| table_entry_to_node(t, 2))
                .collect();
            children.push(TreeNode {
                id: format!("tables:{schema_name}"),
                label: format!("Tables ({})", schema.tables.len()),
                icon: Some("T"),
                children: table_nodes,
                depth: 1,
            });

            // Views
            let view_nodes: Vec<TreeNode> = schema
                .views
                .iter()
                .map(|t| table_entry_to_node(t, 2))
                .collect();
            children.push(TreeNode {
                id: format!("views:{schema_name}"),
                label: format!("Views ({})", schema.views.len()),
                icon: Some("V"),
                children: view_nodes,
                depth: 1,
            });

            // Materialized Views
            let matview_nodes: Vec<TreeNode> = schema
                .materialized_views
                .iter()
                .map(|t| table_entry_to_node(t, 2))
                .collect();
            children.push(TreeNode {
                id: format!("matviews:{schema_name}"),
                label: format!("Materialized Views ({})", schema.materialized_views.len()),
                icon: Some("M"),
                children: matview_nodes,
                depth: 1,
            });

            // Functions
            let function_nodes: Vec<TreeNode> = schema
                .functions
                .iter()
                .map(|f| TreeNode {
                    id: format!("func:{schema_name}.{}", f.name),
                    label: format!("{}({}) -> {}", f.name, f.arguments, f.return_type),
                    icon: None,
                    children: Vec::new(),
                    depth: 2,
                })
                .collect();
            children.push(TreeNode {
                id: format!("functions:{schema_name}"),
                label: format!("Functions ({})", schema.functions.len()),
                icon: Some("F"),
                children: function_nodes,
                depth: 1,
            });

            // Sequences
            let sequence_nodes: Vec<TreeNode> = schema
                .sequences
                .iter()
                .map(|s| TreeNode {
                    id: format!("seq:{schema_name}.{}", s.name),
                    label: s.name.clone(),
                    icon: None,
                    children: Vec::new(),
                    depth: 2,
                })
                .collect();
            children.push(TreeNode {
                id: format!("sequences:{schema_name}"),
                label: format!("Sequences ({})", schema.sequences.len()),
                icon: Some("#"),
                children: sequence_nodes,
                depth: 1,
            });

            TreeNode {
                id: format!("schema:{schema_name}"),
                label: schema_name.clone(),
                icon: Some("S"),
                children,
                depth: 0,
            }
        })
        .collect()
}
