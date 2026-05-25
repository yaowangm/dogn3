# Testing Design Draft

This document records the current testing direction for the project. It is a
draft and should evolve as backend modules, database migrations, and application
workflows become clearer.

## Status

Current decision:

- Backend tests are the first priority.
- Frontend tests are intentionally out of scope for now.
- Normal tests must not touch the migrated `dogn` database.
- Database tests should use a separate disposable PostgreSQL database.
- The test database should be deleted after successful tests.
- If tests fail, the test database may be retained for diagnosis.
- `scripts/test.sh` is the standard command for running fixture-backed
  database tests.

## Goals

- Verify backend behavior with deterministic test data.
- Avoid accidental reads or writes against the migrated production-like
  database.
- Keep the first test setup simple enough to run locally.
- Make database test failures easy to inspect.
- Leave room to adopt formal migrations and broader integration tests later.

## Test Layers

### Unit Tests

Unit tests should cover pure Rust behavior that does not require PostgreSQL.

Good early targets:

- Configuration parsing.
- Default values such as `SITE_NAME`.
- Invalid environment values.
- Small helper functions for enum/status mapping when they are introduced.
- Formatting or transformation logic that is independent of external services.

These tests run with plain `cargo test`.

### HTTP Route Tests

Route tests should exercise `axum` routers directly without starting the
development server.

Guidelines:

- Do not use `./scripts/server.sh` for tests.
- Do not bind a TCP socket for route-level tests.
- Use the router as a service and call it in process.
- Start with routes that do not require database fixture data, such as health
  checks and static page handlers.
- Verify configured local image-directory serving without database access by
  mounting a temporary fixture directory, including rejection of non-image
  files.

Useful dev dependencies may include:

```toml
[dev-dependencies]
tower = "0.5"
http-body-util = "0.1"
```

### Database Integration Tests

Database integration tests should use a separate test database, not `dogn`.

Suggested database name:

```text
dogn_test
```

The test database should be treated as disposable. Test setup may create,
modify, and drop this database. No test should create, modify, or depend on the
real migrated `dogn` database.

Tests read the database URL from a test-specific environment variable:

```text
TEST_DATABASE_URL=postgres:///dogn_test
```

Using `TEST_DATABASE_URL` instead of `DATABASE_URL` reduces the risk of pointing
test code at the application database by accident.

Database-backed integration tests are marked `ignored` by default. This keeps
plain `cargo test` safe for quick local checks and makes the skipped database
coverage visible in the test summary. Use `scripts/test.sh` to run the full
fixture-backed path.

The test script derives `TEST_DATABASE_URL` from `TEST_DB_NAME` instead of
trusting a caller-provided URL. This keeps the database it creates and the
database used by tests aligned.

### Cache Integration Tests

Redis-backed cache tests run through `scripts/test.sh`.

The script sets:

```text
TEST_REDIS_URL=redis://127.0.0.1:6379
```

The cache tests use a unique Redis key prefix based on the test process ID and
advance only that test prefix's cache generation. They do not flush Redis.

Current cache coverage:

- `/api/home` returns cached data until the cache generation advances.
- Home cache variants do not disclose encrypted post resource locations to
  anonymous responses.
- Encrypted posts retain public metadata but redact body resources without a
  login session, including direct local-image access.
- Session-dependent endpoint responses declare `Cache-Control: no-store`.
- `/api/home` returns the same JSON shape and values with or without cache.
- Cached `/api/home` requests are faster than repeated uncached database-backed
  requests in the local fixture test.
- Statistics recalculation advances the home cache generation, and an
  old-generation write completed after the mutation is not served.

The test script runs ignored tests explicitly and uses one test thread so tests
that temporarily modify fixture data remain deterministic.

### Authentication Integration Tests

Authentication tests run only against the disposable PostgreSQL fixture
database through `scripts/test.sh`.

Current authentication coverage:

- A fixture credential wrapped as `argon2id-md5-v1` accepts the original raw
  password and issues an opaque `HttpOnly; SameSite=Lax` session cookie.
- `GET /api/auth/session` resolves that cookie to the authenticated public
  session identity.
- `POST /api/auth/logout` invalidates the in-memory session and clears the
  cookie.
- Unknown or unmigrated credentials receive the same generic authentication
  failure.
- The administrator-only user directory rejects anonymous/member sessions and
  stale downgraded-administrator sessions, and permits administrator search,
  role filtering, ordering, and paging of fixture accounts.

These tests never transform or authenticate against the migrated `dogn`
database.

## Fixture Strategy

Use a small deterministic fixture dataset for normal database tests.

Suggested fixture files:

```text
tests/fixtures/schema.sql
tests/fixtures/home_data.sql
```

The initial fixture should include only data needed to test current backend
behavior. For the default page API, useful rows include:

- Categories.
- Boards in more than one category.
- Root posts.
- Child posts.
- Announcement posts.
- Original posts.
- Forward posts.
- Encrypted posts.
- Deleted posts.
- Posts with image attachments.
- New users.
- Users with high points.

The fixture should be small enough that expected API results can be checked
directly in tests. It should not be copied from the full migrated database.

## Test Database Script

The project provides:

```text
scripts/test.sh
```

Recommended workflow:

```text
1. Refuse to run if the configured test database name is dogn.
2. Drop any existing dogn_test database.
3. Create dogn_test.
4. Apply schema fixture.
5. Apply data fixture.
6. Run cargo test with TEST_DATABASE_URL=postgres:///dogn_test.
7. If all tests pass, drop dogn_test.
8. If any test fails, keep dogn_test for diagnosis.
```

The script should print a clear diagnostic command when it keeps the database:

```bash
psql dogn_test
```

The script should use strict shell behavior:

```bash
set -euo pipefail
```

It should also clearly distinguish cleanup-on-success from retain-on-failure.

## Safety Rules

- Normal `cargo test` should not touch `dogn`.
- Database integration tests must require `TEST_DATABASE_URL`.
- Test setup may only drop or recreate the dedicated test database.
- The test script must refuse dangerous database names such as `dogn`,
  `postgres`, `template0`, and `template1`.
- Failed database tests should keep the test database only for diagnosis.
- No test should rely on mutable production-like data.

## Future Direction

When schema migrations become formal, database tests should create the test
database and apply real migrations instead of a hand-maintained schema fixture.
The fixture data can remain small and deterministic.

Optional read-only compatibility tests against the migrated `dogn` database may
be added later, but they should be explicitly gated and never part of the normal
test command.
