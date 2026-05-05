# PgBlade

A fast, native PostgreSQL workbench built with Rust and GPUI.

PgBlade is a query-first Postgres client with production safety built into the normal workflow. It uses GPUI (Zed's GPU-accelerated UI framework) for a native macOS experience.

## Features

- SQL editor with multi-line editing and Cmd+Enter execution
- Result grid with virtualized scrolling for large result sets
- Schema browser with collapsible tree (schemas, tables, views, columns)
- Saved connections with macOS Keychain credential storage
- Query history persisted to local SQLite
- Production safety: environment badges, read-only mode, write confirmation dialogs
- SQL classification: automatic detection of reads, writes, and destructive operations
- Command palette (Cmd+K) for quick access to all actions
- Fast startup (~200ms), 2ms query round-trip

## Requirements

- macOS (primary platform, cross-platform architecture)
- Rust 1.85+ (edition 2024)
- PostgreSQL 12+ (for connecting to)

## Building

```bash
# Clone and build
git clone https://github.com/LucaGerlich/pgblade.git
cd pgblade
cargo build --release

# Run
cargo run --release

# Run tests (requires Docker for integration tests)
./scripts/test-db.sh start
cargo test --all
./scripts/test-db.sh stop
```

## Architecture

PgBlade uses a 5-crate Rust workspace:

| Crate | Purpose |
|-------|---------|
| `pgblade-core` | Domain types, SQL classifier, SQLite storage, trait boundaries |
| `pgblade-postgres` | PostgreSQL driver, schema introspection, query execution |
| `pgblade-security` | macOS Keychain credential storage |
| `pgblade-ui` | GPUI desktop interface, components, modals |
| `pgblade-app` | Application entry point |

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Cmd+Enter | Execute query |
| Cmd+K | Command palette |
| Cmd+N | New connection |
| Cmd+B | Toggle sidebar |

## License

Apache-2.0 -- see [LICENSE](LICENSE) for details.
