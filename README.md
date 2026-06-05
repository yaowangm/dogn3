# dogn3

`dogn3` is a PostgreSQL-backed Rust web application.

The initial architecture uses:

- `axum` for HTTP routing and handlers
- `sqlx` for PostgreSQL access
- `redis` for the cache layer
- `serde` for JSON DTOs
- `tower-http` for HTTP middleware
- HTML5, CSS3, and native Web Components for the frontend

Architecture notes live in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
Docker deployment notes live in [docs/DOCKER.md](docs/DOCKER.md).

## Local Setup

Copy the example environment file and adjust the database URL:

```bash
cp .env.example .env
```

Set `SITE_NAME` in `.env` to control the site name shown in the browser UI.
Set `SESSION_TTL_SECONDS` to control the login session lifetime; the default is
`604800` seconds (7 days).
The cache layer is optional. Set `CACHE_ENABLED=false` to run without Redis.
When enabled, the default Redis cache configuration expects Redis at
`127.0.0.1:6379`.

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

## License

This project is released under the Apache License 2.0.
