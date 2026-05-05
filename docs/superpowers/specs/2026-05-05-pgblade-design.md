# PgBlade Design

## Summary

PgBlade is an open-source, native Rust database workbench for PostgreSQL. Its promise is simple: a fast query-first Postgres client with production safety built into the normal workflow.

The first version targets macOS first and keeps a cross-platform architecture. PgBlade starts clean from scratch, uses GPUI for the native interface, and borrows ideas from Zed and other open-source database tools without forking an existing client.

## Goals

- Build a fast, low-memory Postgres workbench that feels closer to Zed than DBeaver.
- Keep the first public version useful: saved connections, schema browser, SQL editor, result grid, query history, and table preview.
- Treat production safety as a core product behavior, not an enterprise add-on.
- Keep the architecture ready for future AI and MCP workflows without making AI a v1 feature.
- Publish the project as open source from day one under Apache-2.0.

## Non-Goals

- PgBlade v1 will not support MySQL, SQLite, MongoDB, Redis, or other databases.
- PgBlade v1 will not include inline row editing, ER diagrams, backups, import flows, visual query builders, or SSH tunnels.
- PgBlade v1 will not include an agent UI, built-in AI assistant, or MCP server.
- PgBlade will not try to match DBeaver's full feature surface.

## Product Direction

PgBlade uses a query-first workbench layout. The SQL editor and result grid sit at the center of the app. The schema sidebar, connection picker, query history, table preview, and safety state support that main loop.

The first public MVP includes:

- saved Postgres connection profiles
- OS-keychain credential storage
- optional temporary connections that do not save secrets
- schema sidebar for databases, schemas, tables, views, and functions
- SQL editor
- run selected query and run full query
- cancellable query execution
- virtualized result grid
- table preview
- query history
- production profile marker
- read-only mode
- warnings and confirmation for writes on production profiles

The public product position is "fast by default, measured continuously." Internal budgets should target fast startup, low idle memory, immediate first-page results, cancellable long-running work, and smooth scrolling over large result sets. These budgets guide engineering decisions but do not become marketing claims until we measure them on real builds.

## Technical Architecture

PgBlade will use a Rust workspace with small crates and clear boundaries:

- `pgblade-core`: domain types, connection profiles, query models, result models, safety classification, query history models, and app events.
- `pgblade-postgres`: the built-in PostgreSQL driver, session management, schema introspection, query execution, cancellation, paging, and structured database errors.
- `pgblade-security`: credential storage through the operating system keychain.
- `pgblade-ui`: the GPUI desktop interface, command palette, layout shell, editor surface, schema sidebar, result grid, connection manager, and state rendering.
- `pgblade-app`: the thin executable that wires the crates together.

The UI must not talk directly to raw database connections. It should call an application controller or task layer. That layer owns background tasks, cancellation, status updates, result paging, and error delivery.

The Postgres implementation should satisfy an internal `DatabaseDriver` boundary. PgBlade will ship only the Postgres driver in v1, but this boundary keeps future SQLite or MySQL support possible without forcing an external plugin system now.

## Open-Source Reuse

PgBlade starts as a clean implementation. It may use mature crates and study compatible open-source projects, but it will not fork an existing database client.

Likely direct dependencies include:

- GPUI for native UI
- Tokio for async runtime work
- `tokio-postgres` for PostgreSQL connections and query execution
- `sqlparser` for first-pass SQL classification
- a keychain crate such as `keyring` for credential storage
- Serde for local data serialization
- `tracing` for diagnostics
- Tree-sitter SQL support for syntax highlighting after the editor surface stabilizes

Reference projects include Zed, `pgui`, `zqlz`, DBFlux, PostgresGUI, and pgweb. PgBlade may borrow architecture and product lessons from these projects. It should copy code only after license review and with proper attribution.

## Persistence And Secrets

Connection metadata and query history live in local app data. Secrets never live in plaintext config files or the query history store.

Passwords and future secret types, such as SSH passphrases or tokens, must go through the platform keychain. Temporary connections may hold credentials in memory for the session and discard them on close.

## Query Execution

The query path should support:

- connection validation
- selected-text execution
- full-buffer execution
- structured parameters later, though not required for the first MVP
- first-page result delivery without waiting for the full result set
- explicit cancellation
- structured errors with Postgres code, severity, message, position, detail, and hint when available

PgBlade should avoid pulling huge result sets into the UI. The data model should support paging or streaming into a bounded result store, with the grid rendering only visible cells.

## Safety Model

PgBlade classifies each query before execution as likely read, likely write, destructive, transaction/control, or unknown. The classifier is advisory and helps the UI choose warnings and confirmation states.

Production-marked connections default to read-only mode. Writes on production profiles require an explicit unlock or confirmation. Destructive queries require stronger friction. The execution layer should prefer real database-level protection where possible, such as read-only transactions, because SQL classification is not a security boundary.

Safety states must be visible in the main workbench. A user should always know whether they are connected to local, staging, or production data and whether the current session can write.

## Error Handling

Errors should be precise and recoverable. Connection failures should explain the failing stage, such as host resolution, authentication, SSL, or database selection, without leaking secrets.

Query errors should map Postgres metadata into the UI. Cancellation should appear as a normal terminal state, not as a crash or unexpected failure.

Background task failures should never freeze the UI. The app should keep enough state for the user to edit the query, reconnect, retry, or cancel.

## Testing Strategy

Core behavior gets tested before UI polish.

Unit tests should cover:

- SQL safety classification
- connection profile validation
- secret-key naming and keychain abstraction behavior
- result value formatting
- query history models

Integration tests should run against a local Postgres container and cover:

- connecting with valid and invalid credentials
- schema introspection
- query execution
- structured query errors
- cancellation
- result paging
- read-only protections where practical

UI tests can start with smoke coverage: app launch, empty states, connection form routing, command dispatch, and result-rendering states. Performance instrumentation should ship early so regressions are visible.

## Milestones

### 1. Native Shell Spike

Build the GPUI window, app layout, command routing, basic theme, startup timing, and empty panes.

### 2. Postgres Engine Spike

Connect to Postgres, run a query, stream the first result page, cancel a query, and return structured errors.

### 3. Query Pad Prototype

Add a connection form, SQL editor area, run command, result grid, and error panel.

### 4. Workbench MVP

Add saved connections, keychain credentials, schema sidebar, table preview, query history, production markers, and read-only state.

### 5. Public Alpha Polish

Add README, Apache-2.0 license, contribution guide, build instructions, screenshots, smoke tests, and basic release packaging.

## Open Questions

- Which GPUI editor approach gives PgBlade the best SQL editing surface without pulling in too much Zed internals?
- Should query history use SQLite, a compact local file format, or another local store?
- How much of `gpui-component` should PgBlade adopt versus hand-building a small component set?
- Which release packaging path should ship first for macOS?
