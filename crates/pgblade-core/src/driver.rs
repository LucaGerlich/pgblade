use async_trait::async_trait;

use crate::connection::ConnectionProfile;
use crate::error::{ConnectionError, QueryError};
use crate::result::ResultSet;

/// Trait boundary for database drivers.
///
/// PgBlade ships only a Postgres driver in v1, but this boundary keeps
/// future drivers (SQLite, MySQL) possible without an external plugin system.
///
/// The application controller depends on this trait, not concrete Postgres types.
#[async_trait]
pub trait DatabaseDriver: Send + Sync + 'static {
    /// Establish a connection to the database.
    async fn connect(
        &self,
        profile: &ConnectionProfile,
        password: &str,
    ) -> Result<Box<dyn DatabaseSession>, ConnectionError>;
}

/// A live session with a database.
///
/// Owns the connection resources and provides query execution.
/// Dropping the session closes the underlying connection.
#[async_trait]
pub trait DatabaseSession: Send + Sync {
    /// Execute a SQL statement and return the full result set.
    async fn execute(&self, sql: &str) -> Result<ResultSet, QueryError>;

    /// Cancel any currently running query on this session.
    async fn cancel(&self) -> Result<(), QueryError>;

    /// The server version string (e.g., "PostgreSQL 16.2").
    fn server_version(&self) -> &str;
}
