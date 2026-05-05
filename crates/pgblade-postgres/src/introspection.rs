use pgblade_core::error::QueryError;
use pgblade_core::schema::{ColumnInfo, SchemaInfo, TableInfo, TableKind};

use crate::error_map::map_query_error;

pub async fn fetch_schemas(client: &tokio_postgres::Client) -> Result<Vec<SchemaInfo>, QueryError> {
    let rows = client
        .query(
            "SELECT schema_name, \
                    CASE WHEN schema_name = current_schema() THEN true ELSE false END as is_default \
             FROM information_schema.schemata \
             WHERE schema_name NOT IN ('pg_catalog', 'information_schema', 'pg_toast') \
             ORDER BY schema_name",
            &[],
        )
        .await
        .map_err(map_query_error)?;

    Ok(rows
        .iter()
        .map(|row| SchemaInfo {
            name: row.get(0),
            is_default: row.get(1),
        })
        .collect())
}

pub async fn fetch_tables(
    client: &tokio_postgres::Client,
    schema: &str,
) -> Result<Vec<TableInfo>, QueryError> {
    let rows = client
        .query(
            "SELECT t.table_name, t.table_type, \
                    (SELECT reltuples::bigint FROM pg_class c \
                     JOIN pg_namespace n ON c.relnamespace = n.oid \
                     WHERE c.relname = t.table_name AND n.nspname = t.table_schema) as row_estimate \
             FROM information_schema.tables t \
             WHERE t.table_schema = $1 \
             ORDER BY t.table_type, t.table_name",
            &[&schema],
        )
        .await
        .map_err(map_query_error)?;

    Ok(rows
        .iter()
        .map(|row| {
            let table_type: String = row.get(1);
            let kind = match table_type.as_str() {
                "VIEW" => TableKind::View,
                _ => TableKind::Table,
            };
            TableInfo {
                schema: schema.to_string(),
                name: row.get(0),
                kind,
                row_estimate: row.get(2),
            }
        })
        .collect())
}

pub async fn fetch_columns(
    client: &tokio_postgres::Client,
    schema: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>, QueryError> {
    let rows = client
        .query(
            "SELECT c.column_name, c.data_type, c.is_nullable, c.column_default, \
                    c.ordinal_position::int, \
                    EXISTS( \
                        SELECT 1 FROM information_schema.table_constraints tc \
                        JOIN information_schema.key_column_usage kcu \
                            ON tc.constraint_name = kcu.constraint_name \
                            AND tc.table_schema = kcu.table_schema \
                        WHERE tc.constraint_type = 'PRIMARY KEY' \
                            AND tc.table_schema = $1 \
                            AND tc.table_name = $2 \
                            AND kcu.column_name = c.column_name \
                    ) as is_pk \
             FROM information_schema.columns c \
             WHERE c.table_schema = $1 AND c.table_name = $2 \
             ORDER BY c.ordinal_position",
            &[&schema, &table],
        )
        .await
        .map_err(map_query_error)?;

    Ok(rows
        .iter()
        .map(|row| {
            let nullable_str: String = row.get(2);
            ColumnInfo {
                name: row.get(0),
                data_type: row.get(1),
                nullable: nullable_str == "YES",
                is_primary_key: row.get(5),
                default_value: row.get(3),
                ordinal_position: row.get(4),
            }
        })
        .collect())
}
