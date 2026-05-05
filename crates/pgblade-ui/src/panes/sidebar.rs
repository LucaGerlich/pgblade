use gpui::*;

use pgblade_core::connection::{ConnectionId, ConnectionProfile, Environment};
use pgblade_core::schema::{SchemaTree, TableEntry, TableKind};

use crate::components::tree_view::{TreeNode, TreeView, TreeViewEvent};

pub struct SchemaSidebar {
    tree: Entity<TreeView>,
    saved_connections: Vec<ConnectionProfile>,
    active_connection_id: Option<ConnectionId>,
    schema_tree: Option<SchemaTree>,
}

#[derive(Debug, Clone)]
pub enum SidebarEvent {
    ConnectTo(ConnectionProfile),
    EditConnection(ConnectionProfile),
    DeleteConnection(ConnectionId),
    PreviewTable { schema: String, table: String },
}

impl EventEmitter<SidebarEvent> for SchemaSidebar {}

impl SchemaSidebar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let tree = cx.new(|_| TreeView::new());

        cx.subscribe(&tree, |this, _tree, event: &TreeViewEvent, cx| {
            let TreeViewEvent::NodeClicked(id) = event;
            if let Some(rest) = id.strip_prefix("table:") {
                if let Some((schema, table)) = rest.split_once('.') {
                    cx.emit(SidebarEvent::PreviewTable {
                        schema: schema.to_string(),
                        table: table.to_string(),
                    });
                }
            } else if let Some(conn_id_str) = id.strip_prefix("connect:") {
                if let Some(profile) = this.find_connection(conn_id_str) {
                    cx.emit(SidebarEvent::ConnectTo(profile));
                }
            } else if let Some(conn_id_str) = id.strip_prefix("edit:") {
                if let Some(profile) = this.find_connection(conn_id_str) {
                    cx.emit(SidebarEvent::EditConnection(profile));
                }
            } else if let Some(conn_id_str) = id.strip_prefix("delete:")
                && let Some(profile) = this.find_connection(conn_id_str)
            {
                cx.emit(SidebarEvent::DeleteConnection(profile.id));
            }
        })
        .detach();

        Self {
            tree,
            saved_connections: Vec::new(),
            active_connection_id: None,
            schema_tree: None,
        }
    }

    pub fn set_saved_connections(
        &mut self,
        connections: Vec<ConnectionProfile>,
        cx: &mut Context<Self>,
    ) {
        self.saved_connections = connections;
        self.rebuild_tree(cx);
    }

    pub fn set_schema_tree(
        &mut self,
        connection_id: ConnectionId,
        schema_tree: &SchemaTree,
        cx: &mut Context<Self>,
    ) {
        self.active_connection_id = Some(connection_id);
        self.schema_tree = Some(schema_tree.clone());
        self.rebuild_tree(cx);
    }

    #[allow(dead_code)]
    pub fn clear_schema(&mut self, cx: &mut Context<Self>) {
        self.active_connection_id = None;
        self.schema_tree = None;
        self.rebuild_tree(cx);
    }

    fn find_connection(&self, id_str: &str) -> Option<ConnectionProfile> {
        self.saved_connections
            .iter()
            .find(|c| c.id.0.to_string() == id_str)
            .cloned()
    }

    fn rebuild_tree(&self, cx: &mut Context<Self>) {
        let nodes = self.build_unified_tree();
        self.tree.update(cx, |tree, cx| {
            tree.set_nodes(nodes, cx);
        });
        cx.notify();
    }

    /// Build a unified tree: connections as top-level, schema nested under active connection.
    fn build_unified_tree(&self) -> Vec<TreeNode> {
        if self.saved_connections.is_empty() && self.schema_tree.is_none() {
            return vec![TreeNode {
                id: "empty".to_string(),
                label: "No connections — press Cmd+N".to_string(),
                icon: None,
                children: Vec::new(),
                depth: 0,
            }];
        }

        self.saved_connections
            .iter()
            .map(|conn| {
                let is_active = self
                    .active_connection_id
                    .map(|id| id == conn.id)
                    .unwrap_or(false);

                let env_icon = match conn.environment {
                    Environment::Production => "P",
                    Environment::Staging => "S",
                    Environment::Development => "D",
                    Environment::Local => "L",
                };

                let children = if is_active {
                    if let Some(schema_tree) = &self.schema_tree {
                        build_database_children(&conn.database, schema_tree)
                    } else {
                        vec![TreeNode {
                            id: format!("loading:{}", conn.id.0),
                            label: "Loading...".to_string(),
                            icon: None,
                            children: Vec::new(),
                            depth: 1,
                        }]
                    }
                } else {
                    // Not connected — actions
                    vec![
                        TreeNode {
                            id: format!("connect:{}", conn.id.0),
                            label: "Connect".to_string(),
                            icon: None,
                            children: Vec::new(),
                            depth: 1,
                        },
                        TreeNode {
                            id: format!("edit:{}", conn.id.0),
                            label: "Edit".to_string(),
                            icon: None,
                            children: Vec::new(),
                            depth: 1,
                        },
                        TreeNode {
                            id: format!("delete:{}", conn.id.0),
                            label: "Delete".to_string(),
                            icon: None,
                            children: Vec::new(),
                            depth: 1,
                        },
                    ]
                };

                let status = if is_active { " (connected)" } else { "" };

                TreeNode {
                    id: format!("connection:{}", conn.id.0),
                    label: format!("{env_icon} {}{status}", conn.name),
                    icon: None,
                    children,
                    depth: 0,
                }
            })
            .collect()
    }
}

impl Render for SchemaSidebar {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("schema-sidebar")
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .p_1()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0x888888))
                    .px_1()
                    .mb_1()
                    .child("DATABASE NAVIGATOR"),
            )
            .child(self.tree.clone())
    }
}

/// Build children for a database: Schemas > ...
fn build_database_children(database_name: &str, schema_tree: &SchemaTree) -> Vec<TreeNode> {
    let schema_children: Vec<TreeNode> = schema_tree
        .schemas
        .iter()
        .map(|schema| {
            let mut categories = Vec::new();

            // Tables
            let table_nodes: Vec<TreeNode> = schema
                .tables
                .iter()
                .map(|t| table_entry_to_node(t, 5))
                .collect();
            categories.push(TreeNode {
                id: format!("tables:{}", schema.info.name),
                label: format!("Tables ({})", schema.tables.len()),
                icon: None,
                children: table_nodes,
                depth: 4,
            });

            // Views
            let view_nodes: Vec<TreeNode> = schema
                .views
                .iter()
                .map(|v| table_entry_to_node(v, 5))
                .collect();
            categories.push(TreeNode {
                id: format!("views:{}", schema.info.name),
                label: format!("Views ({})", schema.views.len()),
                icon: None,
                children: view_nodes,
                depth: 4,
            });

            // Materialized Views
            let mv_nodes: Vec<TreeNode> = schema
                .materialized_views
                .iter()
                .map(|v| table_entry_to_node(v, 5))
                .collect();
            categories.push(TreeNode {
                id: format!("matviews:{}", schema.info.name),
                label: format!("Materialized Views ({})", schema.materialized_views.len()),
                icon: None,
                children: mv_nodes,
                depth: 4,
            });

            // Functions
            let fn_nodes: Vec<TreeNode> = schema
                .functions
                .iter()
                .map(|f| TreeNode {
                    id: format!("func:{}.{}", schema.info.name, f.name),
                    label: format!("{}({}) -> {}", f.name, f.arguments, f.return_type),
                    icon: None,
                    children: Vec::new(),
                    depth: 5,
                })
                .collect();
            categories.push(TreeNode {
                id: format!("functions:{}", schema.info.name),
                label: format!("Functions ({})", schema.functions.len()),
                icon: None,
                children: fn_nodes,
                depth: 4,
            });

            // Sequences
            let seq_nodes: Vec<TreeNode> = schema
                .sequences
                .iter()
                .map(|s| TreeNode {
                    id: format!("seq:{}.{}", schema.info.name, s.name),
                    label: s.name.clone(),
                    icon: None,
                    children: Vec::new(),
                    depth: 5,
                })
                .collect();
            categories.push(TreeNode {
                id: format!("sequences:{}", schema.info.name),
                label: format!("Sequences ({})", schema.sequences.len()),
                icon: None,
                children: seq_nodes,
                depth: 4,
            });

            TreeNode {
                id: format!("schema:{}", schema.info.name),
                label: schema.info.name.clone(),
                icon: None,
                children: categories,
                depth: 3,
            }
        })
        .collect();

    vec![TreeNode {
        id: "databases".to_string(),
        label: "Databases".to_string(),
        icon: None,
        children: vec![TreeNode {
            id: format!("db:{database_name}"),
            label: database_name.to_string(),
            icon: None,
            children: vec![TreeNode {
                id: "schemas".to_string(),
                label: "Schemas".to_string(),
                icon: None,
                children: schema_children,
                depth: 2,
            }],
            depth: 1,
        }],
        depth: 0,
    }]
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
                id: format!("col:{qualified}.{}", col.name),
                label: format!("{pk}{} ({}){null}", col.name, col.data_type),
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

    let kind_label = match table.info.kind {
        TableKind::Table => "T",
        TableKind::View => "V",
        TableKind::MaterializedView => "MV",
    };

    TreeNode {
        id: format!("table:{qualified}"),
        label: format!("{kind_label} {}", table.info.name),
        icon: None,
        children,
        depth,
    }
}
