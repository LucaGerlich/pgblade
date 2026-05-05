use std::sync::Arc;

use gpui::{Context, EventEmitter};
use tokio::runtime::Runtime;
use uuid::Uuid;

use pgblade_core::classifier::classify_sql;
use pgblade_core::connection::{ConnectionId, ConnectionProfile, ConnectionState};
use pgblade_core::driver::{DatabaseDriver, DatabaseSession};
use pgblade_core::error::QueryError;
use pgblade_core::event::AppEvent;
use pgblade_core::query::{QueryClassification, QueryId, QueryRecord, QueryState};
use pgblade_core::safety::{SafetyLevel, WritePolicy, evaluate_safety};
use pgblade_core::schema::SchemaTree;
use pgblade_core::security::CredentialStore;
use pgblade_core::storage::StorageManager;
use pgblade_postgres::PostgresDriver;
use pgblade_security::KeychainStore;

/// The application controller sits between the UI and the backend.
///
/// It owns the tokio runtime, the database driver, the active session,
/// storage, credential store, and all background work. UI views read state
/// from this entity and subscribe to `AppEvent` emissions.
pub struct AppController {
    runtime: Arc<Runtime>,
    driver: PostgresDriver,
    session: Option<Arc<dyn DatabaseSession>>,
    storage: StorageManager,
    credential_store: KeychainStore,
    connection_state: ConnectionState,
    active_query: Option<(QueryId, QueryState)>,
    write_policy: WritePolicy,
    schema_tree: Option<SchemaTree>,
    saved_connections: Vec<ConnectionProfile>,
    query_history: Vec<QueryRecord>,
    active_profile: Option<ConnectionProfile>,
}

impl Default for AppController {
    fn default() -> Self {
        Self::new()
    }
}

impl AppController {
    pub fn new() -> Self {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("failed to create tokio runtime"),
        );

        let driver = PostgresDriver::new(runtime.clone());

        let storage = StorageManager::init().unwrap_or_else(|e| {
            tracing::error!("failed to init storage, falling back to in-memory: {e}");
            StorageManager::in_memory().expect("in-memory storage should not fail")
        });

        let credential_store = KeychainStore::new();

        Self {
            runtime,
            driver,
            session: None,
            storage,
            credential_store,
            connection_state: ConnectionState::Disconnected,
            active_query: None,
            write_policy: WritePolicy::ReadOnly,
            schema_tree: None,
            saved_connections: Vec::new(),
            query_history: Vec::new(),
            active_profile: None,
        }
    }

    pub fn connection_state(&self) -> &ConnectionState {
        &self.connection_state
    }

    pub fn write_policy(&self) -> WritePolicy {
        self.write_policy
    }

    pub fn active_query_state(&self) -> Option<&QueryState> {
        self.active_query.as_ref().map(|(_, s)| s)
    }

    pub fn is_connected(&self) -> bool {
        self.connection_state.is_connected()
    }

    pub fn credential_store(&self) -> &KeychainStore {
        &self.credential_store
    }

    #[allow(dead_code)]
    pub fn schema_tree(&self) -> Option<&SchemaTree> {
        self.schema_tree.as_ref()
    }

    #[allow(dead_code)]
    pub fn saved_connections(&self) -> &[ConnectionProfile] {
        &self.saved_connections
    }

    #[allow(dead_code)]
    pub fn query_history(&self) -> &[QueryRecord] {
        &self.query_history
    }

    /// Returns the active database name from the connection state.
    fn active_database(&self) -> Option<String> {
        match &self.connection_state {
            ConnectionState::Connected { database, .. } => Some(database.clone()),
            _ => None,
        }
    }

    /// Load saved connections from SQLite storage.
    pub fn load_saved_connections(&mut self, cx: &mut Context<Self>) {
        match self.storage.load_connections() {
            Ok(connections) => {
                self.saved_connections = connections;
                cx.emit(AppEvent::SavedConnectionsLoaded(
                    self.saved_connections.clone(),
                ));
            }
            Err(e) => tracing::error!("failed to load connections: {e}"),
        }
    }

    /// Save a connection profile to SQLite + credential to keychain.
    pub fn save_connection(
        &mut self,
        profile: &ConnectionProfile,
        password: &str,
        cx: &mut Context<Self>,
    ) {
        if let Err(e) = self.storage.save_connection(profile) {
            tracing::error!("failed to save connection: {e}");
            return;
        }
        if let Err(e) = self
            .credential_store
            .store(&profile.keychain_service_key(), password)
        {
            tracing::error!("failed to store credential: {e}");
        }
        self.load_saved_connections(cx);
    }

    /// Delete a connection profile from SQLite + credential from keychain.
    #[allow(dead_code)]
    pub fn delete_connection(&mut self, id: &ConnectionId, cx: &mut Context<Self>) {
        // Retrieve the profile to get the keychain key before deleting
        let service_key = self
            .saved_connections
            .iter()
            .find(|c| c.id == *id)
            .map(|c| c.keychain_service_key());

        if let Err(e) = self.storage.delete_connection(id) {
            tracing::error!("failed to delete connection: {e}");
            return;
        }
        if let Some(key) = service_key
            && let Err(e) = self.credential_store.delete(&key)
        {
            tracing::error!("failed to delete credential: {e}");
        }
        self.load_saved_connections(cx);
    }

    /// Fetch the database schema tree via async introspection.
    pub fn fetch_schema(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else {
            return;
        };
        let runtime = self.runtime.clone();

        cx.spawn(async move |this, cx| {
            let schemas_result = runtime
                .spawn(async move {
                    let schemas = session.list_schemas().await?;
                    let mut entries = Vec::new();
                    for schema in &schemas {
                        let tables = session.list_tables(&schema.name).await?;
                        let mut table_entries = Vec::new();
                        for table in &tables {
                            let columns = session.list_columns(&schema.name, &table.name).await?;
                            table_entries.push(pgblade_core::schema::TableEntry {
                                info: table.clone(),
                                columns,
                            });
                        }
                        entries.push(pgblade_core::schema::SchemaEntry {
                            info: schema.clone(),
                            tables: table_entries,
                        });
                    }
                    Ok::<_, QueryError>(SchemaTree { schemas: entries })
                })
                .await;

            match schemas_result {
                Ok(Ok(tree)) => {
                    this.update(cx, |controller, cx| {
                        controller.schema_tree = Some(tree.clone());
                        cx.emit(AppEvent::SchemaLoaded(tree));
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => tracing::error!("schema introspection failed: {e}"),
                Err(e) => tracing::error!("runtime error: {e}"),
            }
        })
        .detach();
    }

    /// Preview a table by executing `SELECT * FROM schema.table LIMIT 100`.
    pub fn preview_table(&mut self, schema: String, table: String, cx: &mut Context<Self>) {
        let sql = format!("SELECT * FROM \"{schema}\".\"{table}\" LIMIT 100");
        self.force_execute_query(sql, cx);
    }

    /// Load query history from SQLite storage.
    pub fn load_history(&mut self, cx: &mut Context<Self>) {
        match self.storage.load_recent_history(100) {
            Ok(history) => {
                self.query_history = history;
                cx.emit(AppEvent::HistoryLoaded(self.query_history.clone()));
            }
            Err(e) => tracing::error!("failed to load history: {e}"),
        }
    }

    /// Initiate a connection to a database.
    pub fn connect(
        &mut self,
        profile: ConnectionProfile,
        password: String,
        cx: &mut Context<Self>,
    ) {
        self.connection_state = ConnectionState::Connecting {
            profile_id: profile.id,
            attempt: 1,
        };
        cx.emit(AppEvent::ConnectionStateChanged(
            self.connection_state.clone(),
        ));
        cx.notify();

        let driver = self.driver.clone();
        let profile_id = profile.id;
        let database = profile.database.clone();
        let read_only_default = profile.read_only_default;
        let stored_profile = profile.clone();

        // We need to run the connection on the tokio runtime since it does TCP I/O.
        let runtime = self.runtime.clone();

        cx.spawn(async move |this, cx| {
            // Run connect on the tokio runtime
            let result = runtime
                .spawn(async move { driver.connect(&profile, &password).await })
                .await;

            let connect_result = match result {
                Ok(inner) => inner,
                Err(e) => {
                    tracing::error!("tokio task panicked: {e}");
                    return;
                }
            };

            this.update(cx, |controller, cx| match connect_result {
                Ok(session) => {
                    let server_version = session.server_version().to_string();
                    let session_arc: Arc<dyn DatabaseSession> = Arc::from(session);
                    controller.session = Some(session_arc);
                    controller.connection_state = ConnectionState::Connected {
                        profile_id,
                        session_id: Uuid::new_v4(),
                        server_version,
                        database,
                        read_only: read_only_default,
                    };
                    controller.write_policy = if read_only_default {
                        WritePolicy::ReadOnly
                    } else {
                        WritePolicy::ConfirmWrites
                    };
                    controller.active_profile = Some(stored_profile);
                    cx.emit(AppEvent::ConnectionStateChanged(
                        controller.connection_state.clone(),
                    ));
                    cx.notify();
                    tracing::info!("connected to postgres");

                    // Auto-fetch schema after successful connection
                    controller.fetch_schema(cx);
                }
                Err(error) => {
                    controller.connection_state = ConnectionState::Failed {
                        profile_id,
                        error,
                        retries: 0,
                    };
                    cx.emit(AppEvent::ConnectionStateChanged(
                        controller.connection_state.clone(),
                    ));
                    cx.notify();
                    tracing::error!("connection failed");
                }
            })
            .ok();
        })
        .detach();
    }

    /// Disconnect the active session.
    pub fn disconnect(&mut self, cx: &mut Context<Self>) {
        // Dropping the Arc<dyn DatabaseSession> causes the client to drop,
        // which signals the background connection task to exit.
        self.session = None;
        self.schema_tree = None;
        self.active_profile = None;
        self.connection_state = ConnectionState::Disconnected;
        cx.emit(AppEvent::ConnectionStateChanged(
            self.connection_state.clone(),
        ));
        cx.notify();
        tracing::info!("disconnected");
    }

    /// Execute a SQL query on the active session with safety classification.
    pub fn execute_query(&mut self, sql: String, cx: &mut Context<Self>) {
        let Some(_session) = self.session.as_ref() else {
            return;
        };

        // Classify the SQL and check safety
        let classification = classify_sql(&sql);
        let safety = evaluate_safety(classification, self.write_policy);

        match safety {
            SafetyLevel::Blocked => {
                let query_id = QueryId::new();
                let started_at = chrono::Utc::now();
                cx.emit(AppEvent::QueryStateChanged {
                    query_id,
                    state: QueryState::Failed {
                        started_at,
                        error: QueryError::ReadOnlyViolation,
                    },
                });
                cx.notify();
                return;
            }
            SafetyLevel::RequiresConfirmation => {
                cx.emit(AppEvent::WriteConfirmationNeeded {
                    sql,
                    classification,
                });
                return;
            }
            SafetyLevel::Safe => {}
        }

        self.do_execute_query(sql, classification, cx);
    }

    /// Execute a query without safety classification checks.
    /// Used after user confirms a write operation.
    pub fn force_execute_query(&mut self, sql: String, cx: &mut Context<Self>) {
        let classification = classify_sql(&sql);
        self.do_execute_query(sql, classification, cx);
    }

    /// Internal: actually execute the query on the session.
    fn do_execute_query(
        &mut self,
        sql: String,
        classification: QueryClassification,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self.session.clone() else {
            return;
        };

        let query_id = QueryId::new();
        let started_at = chrono::Utc::now();

        self.active_query = Some((query_id, QueryState::Executing { started_at }));
        cx.emit(AppEvent::QueryStateChanged {
            query_id,
            state: QueryState::Executing { started_at },
        });
        cx.notify();

        let runtime = self.runtime.clone();
        let sql_clone = sql.clone();
        let database = self.active_database().unwrap_or_default();

        cx.spawn(async move |this, cx| {
            // Execute on tokio runtime
            let result = runtime
                .spawn(async move { session.execute(&sql_clone).await })
                .await;

            let query_result = match result {
                Ok(inner) => inner,
                Err(e) => Err(QueryError::Other {
                    message: format!("task panicked: {e}"),
                }),
            };

            this.update(cx, |controller, cx| {
                let completed_at = chrono::Utc::now();
                match query_result {
                    Ok(result_set) => {
                        let total_rows = result_set.total_rows;
                        let elapsed = (completed_at - started_at).num_milliseconds();
                        controller.active_query = Some((
                            query_id,
                            QueryState::Completed {
                                started_at,
                                completed_at,
                                total_rows,
                            },
                        ));
                        cx.emit(AppEvent::QueryStateChanged {
                            query_id,
                            state: QueryState::Completed {
                                started_at,
                                completed_at,
                                total_rows,
                            },
                        });
                        cx.emit(AppEvent::ResultPageReady {
                            query_id,
                            page: pgblade_core::result::ResultPage {
                                columns: result_set.columns,
                                rows: result_set.rows,
                                page_index: 0,
                                has_more: false,
                            },
                        });

                        // Save to query history
                        let record = QueryRecord {
                            id: query_id,
                            sql: sql.clone(),
                            classification,
                            executed_at: started_at,
                            duration_ms: Some(elapsed),
                            row_count: Some(total_rows),
                            error: None,
                            database: database.clone(),
                        };
                        let _ = controller.storage.save_query(&record);
                    }
                    Err(error) => {
                        controller.active_query = Some((
                            query_id,
                            QueryState::Failed {
                                started_at,
                                error: error.clone(),
                            },
                        ));
                        cx.emit(AppEvent::QueryStateChanged {
                            query_id,
                            state: QueryState::Failed { started_at, error },
                        });
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Cancel the currently running query.
    pub fn cancel_query(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else {
            return;
        };

        let Some((query_id, QueryState::Executing { started_at })) = &self.active_query else {
            return;
        };
        let query_id = *query_id;
        let started_at = *started_at;

        let runtime = self.runtime.clone();

        cx.spawn(async move |this, cx| {
            let cancel_result = runtime.spawn(async move { session.cancel().await }).await;

            if cancel_result.is_ok() {
                this.update(cx, |controller, cx| {
                    let cancelled_at = chrono::Utc::now();
                    controller.active_query = Some((
                        query_id,
                        QueryState::Cancelled {
                            started_at,
                            cancelled_at,
                        },
                    ));
                    cx.emit(AppEvent::QueryStateChanged {
                        query_id,
                        state: QueryState::Cancelled {
                            started_at,
                            cancelled_at,
                        },
                    });
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }
}

impl EventEmitter<AppEvent> for AppController {}
