use serde::{Deserialize, Serialize};

/// Metadata about a PostgreSQL schema (namespace).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaInfo {
    pub name: String,
    pub is_default: bool,
}

/// The kind of relation in a schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableKind {
    Table,
    View,
    MaterializedView,
}

/// Metadata about a table, view, or materialized view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub schema: String,
    pub name: String,
    pub kind: TableKind,
    pub row_estimate: Option<i64>,
}

impl TableInfo {
    /// Returns the fully qualified name: `schema.table`.
    pub fn qualified_name(&self) -> String {
        format!("{}.{}", self.schema, self.name)
    }
}

/// Metadata about a single column in a table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub is_primary_key: bool,
    pub default_value: Option<String>,
    pub ordinal_position: i32,
}

/// A full schema tree representing the introspected database structure.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchemaTree {
    pub schemas: Vec<SchemaEntry>,
}

/// A schema with its child tables/views.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaEntry {
    pub info: SchemaInfo,
    pub tables: Vec<TableEntry>,
}

/// A table/view with its child columns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableEntry {
    pub info: TableInfo,
    pub columns: Vec<ColumnInfo>,
}
