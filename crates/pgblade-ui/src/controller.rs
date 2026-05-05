use std::sync::Arc;

use gpui::{Context, EventEmitter};
use tokio::runtime::Runtime;
use uuid::Uuid;

use pgblade_core::connection::{ConnectionProfile, ConnectionState};
use pgblade_core::driver::{DatabaseDriver, DatabaseSession};
use pgblade_core::error::QueryError;
use pgblade_core::event::AppEvent;
use pgblade_core::query::{QueryId, QueryState};
use pgblade_core::safety::WritePolicy;
use pgblade_postgres::PostgresDriver;

/// The application controller sits between the UI and the backend.
///
/// It owns the tokio runtime, the database driver, the active session,
/// and all background work. UI views read state from this entity
/// and subscribe to `AppEvent` emissions.
pub struct AppController {
    runtime: Arc<Runtime>,
    driver: PostgresDriver,
    session: Option<Arc<dyn DatabaseSession>>,
    connection_state: ConnectionState,
    active_query: Option<(QueryId, QueryState)>,
    write_policy: WritePolicy,
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

        Self {
            runtime,
            driver,
            session: None,
            connection_state: ConnectionState::Disconnected,
            active_query: None,
            write_policy: WritePolicy::ReadOnly,
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
                    cx.emit(AppEvent::ConnectionStateChanged(
                        controller.connection_state.clone(),
                    ));
                    cx.notify();
                    tracing::info!("connected to postgres");
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
        self.connection_state = ConnectionState::Disconnected;
        cx.emit(AppEvent::ConnectionStateChanged(
            self.connection_state.clone(),
        ));
        cx.notify();
        tracing::info!("disconnected");
    }

    /// Execute a SQL query on the active session.
    pub fn execute_query(&mut self, sql: String, cx: &mut Context<Self>) {
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

        cx.spawn(async move |this, cx| {
            // Execute on tokio runtime (the Client channels should work from
            // any executor, but we route through tokio for reliability)
            let result = runtime
                .spawn(async move { session.execute(&sql).await })
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
