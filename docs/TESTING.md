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

These tests should run with plain `cargo test`.

### HTTP Route Tests

Route tests should exercise `axum` routers directly without starting the
development server.

Guidelines:

- Do not use `./scripts/server.sh` for tests.
- Do not bind a TCP socket for route-level tests.
- Use the router as a service and call it in process.
- Start with routes that do not require database fixture data, such as health
  checks and static page handlers.

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
dogn3_test
```

The test database should be treated as disposable. Test setup may create,
modify, and drop this database. No test should create, modify, or depend on the
real migrated `dogn` database.

Tests should read the database URL from a test-specific environment variable:

```text
TEST_DATABASE_URL=postgres:///dogn3_test
```

Using `TEST_DATABASE_URL` instead of `DATABASE_URL` reduces the risk of pointing
test code at the application database by accident.

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

Add a script such as:

```text
scripts/test.sh
```

Recommended workflow:

```text
1. Refuse to run if the configured test database name is dogn.
2. Drop any existing dogn3_test database.
3. Create dogn3_test.
4. Apply schema fixture.
5. Apply data fixture.
6. Run cargo test with TEST_DATABASE_URL=postgres:///dogn3_test.
7. If all tests pass, drop dogn3_test.
8. If any test fails, keep dogn3_test for diagnosis.
```

The script should print a clear diagnostic command when it keeps the database:

```bash
psql dogn3_test
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
