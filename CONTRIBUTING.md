# Contributing to PgBlade

## Development Setup

1. Install Rust 1.85+ via [rustup](https://rustup.rs/)
2. Clone the repository
3. Build: `cargo build`
4. Run: `cargo run`

## Testing

Unit tests run without external dependencies:

```bash
cargo test -p pgblade-core
```

Integration tests require a PostgreSQL instance (Docker):

```bash
./scripts/test-db.sh start    # Start test container
cargo test --all               # Run all tests
./scripts/test-db.sh stop     # Stop container
```

## Code Quality

Before submitting changes:

```bash
cargo fmt --all               # Format code
cargo clippy -- -D warnings   # Lint
cargo test --all              # All tests pass
```

## Project Structure

```
pgblade/
  crates/
    pgblade-core/       -- Domain types, no I/O dependencies
    pgblade-postgres/   -- PostgreSQL driver implementation
    pgblade-security/   -- OS keychain integration
    pgblade-ui/         -- GPUI desktop interface
    pgblade-app/        -- Application entry point
  scripts/
    test-db.sh          -- Docker helper for test database
  docs/
    superpowers/specs/  -- Design specification
```

## Guidelines

- Rust edition 2024, strict clippy
- 4-space indentation (rustfmt default)
- Keep crate boundaries clean: UI never imports postgres types directly
- All database work must be async and non-blocking
- Test coverage for business logic (safety classification, storage, error mapping)
