# dogn3

`dogn3` is a PostgreSQL-backed Rust web application.

The initial architecture uses:

- `axum` for HTTP routing and handlers
- `sqlx` for PostgreSQL access
- `serde` for JSON DTOs
- `tower-http` for HTTP middleware
- HTML5, CSS3, and native Web Components for the frontend

Architecture notes live in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Local Setup

Copy the example environment file and adjust the database URL:

```bash
cp .env.example .env
```

Run the development server:

```bash
./scripts/server.sh start
```

The default server address is `http://127.0.0.1:3000`.

Manage the development server:

```bash
./scripts/server.sh status
./scripts/server.sh restart
./scripts/server.sh stop
```

## Project Layout

- `src/main.rs`: application entry point and router assembly
- `src/config.rs`: environment-based runtime configuration
- `src/routes/`: HTTP page and API routes
- `src/state.rs`: shared application state
- `src/error.rs`: shared API error shape
- `static/`: HTML, CSS, and native Web Components
- `migrations/`: future SQLx migrations
- `scripts/`: operational helper scripts
- `docs/`: architecture and design notes

## Database

The application expects a PostgreSQL database. Future schema changes should use
SQLx migrations under `migrations/`.

The existing MySQL-to-PostgreSQL migration helper is
`scripts/migrate_mysql_to_postgres.sh`.
