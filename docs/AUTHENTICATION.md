# Authentication Design Draft

This document records the initial authentication direction and current
implementation. Authorization behavior and durable session persistence still
require further decisions.

## Status

Current direction:

- Add a login page at `/login`, reached from the header `login` link.
- Preserve existing users' ability to authenticate with their current
  passwords.
- Remove legacy MD5-only password hashes from active storage before login is
  enabled.
- Migrate each existing credential by hashing its stored MD5 value with
  Argon2id.
- Verify migrated users by applying MD5 to the submitted password in memory
  and verifying that result against the stored Argon2id hash.
- Use `user_info.password` as the expanded hash value column and add
  `user_info.password_scheme` for scheme/version identification.
- Authenticate through JSON API routes and maintain opaque in-memory server
  sessions for the initial implementation.

Important terminology: password values are hashed, not encrypted. No password
decryption operation exists.

## Background

The migrated `user_info.password` column currently contains values produced by
legacy unsalted MD5 password hashing:

```text
md5(raw_password)
```

MD5 alone is unsuitable for password storage because it is fast and offers
little resistance against offline password guessing after a credential
database leak.

The original raw passwords are unavailable. Requiring every user to reset a
password before login would disrupt compatibility with the legacy forum, so
the initial credential migration must operate on the existing stored value.

## Credential Migration

### Active Credential Storage

Authentication reads active credentials only from `user_info`:

| Column | Use |
| --- | --- |
| `password` | PHC-formatted Argon2id encoded hash after transformation. |
| `password_scheme` | Identifies which input was hashed. Current login support requires `argon2id-md5-v1`. |

The legacy backup table `info_bak` is not an authentication source and is not
modified by the active credential migration. It must be reviewed separately
because it may still contain legacy password material.

### One-Time Transformation

Before authentication is made available, each MD5-only value in
`user_info.password` should be replaced with an Argon2id hash of the existing
MD5 value:

```text
legacy stored value: md5(raw_password)
new stored value:    argon2id(md5(raw_password), random_salt, parameters)
```

After this transformation, active credential storage contains only
Argon2id-encoded values; a standalone MD5 hash is no longer stored in
`user_info.password`.

This migration changes password data in the PostgreSQL database. It must not be
executed until an implementation is prepared and explicit database-change
approval is given.

Generated migration artifacts:

- `scripts/migrate_legacy_password_schema.sql` defines the schema changes
  required for expanded Argon2id hashes and the scheme marker.
- `src/bin/migrate_legacy_passwords.rs` is the executable migration utility.
  It performs the actual one-time transformation and executes the SQL fragment
  in the same transaction.

The SQL fragment **does not transform password values**. It is included by the
Rust utility so schema changes and password transformations are committed
together. Do not use the SQL file alone for a fresh migration.

### Transformation Command

This command changes credential data. It should be run only after explicit
database-change approval and with an appropriate backup/recovery plan:

```bash
DATABASE_URL=postgres:///dogn cargo run --bin migrate_legacy_passwords -- --execute
```

The `--execute` argument is mandatory. Without exactly that flag, the utility
prints usage information and refuses to modify credentials.

For a future refresh from a newer legacy MySQL database, run this utility only
after the imported PostgreSQL schema has been upgraded to the application
naming convention by `scripts/upgrade_initial_postgres_schema.sql`. The
credential utility requires `user_info.password`; it creates
`user_info.password_scheme` when that column is absent.

### Transformation Algorithm

For every unmarked active credential, the utility applies:

```text
stored legacy input: md5(raw_password)
generated salt:      random and unique per account
new password value:  argon2id(stored legacy input, generated salt)
new scheme value:    argon2id-md5-v1
```

The command:

1. Begins one database transaction.
2. Executes `scripts/migrate_legacy_password_schema.sql` inside that
   transaction, expanding `password` to `text`, adding `password_scheme` when
   absent, and recording credential column comments.
3. Refuses to proceed if any row has an unsupported non-empty
   `password_scheme`.
4. Selects and locks all rows whose `password_scheme` is `NULL` or empty.
5. Validates every selected password value as a lowercase
   32-character MD5 string.
6. Replaces each validated value with a uniquely salted Argon2id PHC string
   and sets `password_scheme = 'argon2id-md5-v1'`.
7. Commits only after every credential has been transformed successfully.

If validation or hashing fails, the transaction rolls back, leaving active
credentials unchanged.

### Rerun and Recovery Behavior

The executable utility is designed to be safe to rerun for credentials it has
already migrated:

- Rows marked `argon2id-md5-v1` are left unchanged.
- The reserved marker `argon2id-v1` is recognized and left unchanged, although
  direct-hash login is not implemented yet.
- If all active credentials are already marked, a rerun migrates zero rows.
- An unknown non-empty scheme aborts the transaction rather than assuming a
  credential interpretation.
- An unmarked row whose `password` is no longer a lowercase MD5 value aborts
  the transaction rather than overwriting uncertain data.

If `scripts/migrate_legacy_password_schema.sql` is run by itself, the schema
is prepared but no password is transformed. Login rejects those accounts
because they do not have a supported `password_scheme`. Recovery is to run the
executable transformation command above; its schema operations remain valid
after the SQL fragment has already been applied, and it transforms the
still-unmarked MD5 values.

Do not manually set `password_scheme` on a legacy MD5 row. A row marked as
migrated without an Argon2id password value cannot authenticate and will be
skipped by the migration utility.

### Post-Transformation Checks

The utility prints how many active credentials it transformed. A read-only
database inspection may also confirm scheme counts without exposing password
hashes:

```sql
SELECT password_scheme, COUNT(*)
FROM user_info
GROUP BY password_scheme
ORDER BY password_scheme;
```

Active migrated credentials are expected to use `argon2id-md5-v1`. Never print
or export `password` values while checking migration status.

### What This Improves

An attacker who steals only the transformed database must evaluate the
configured Argon2id work factor for each attempted password guess, rather than
testing guesses directly with fast MD5.

### Remaining Limitation

The migrated credential is still derived from the legacy MD5 representation.
If an attacker already possesses an older dump containing the original MD5
hashes, rewriting the current database cannot remove the risk from that older
dump.

The separate `info_bak.password` column may also contain legacy password
material. It is not an active authentication source and is intentionally not
altered by the active-account migration utility. Before production deployment,
decide whether that archive must be removed, independently migrated, or
retained under stricter access controls.

## Hash Scheme Identification

Credential format must be identifiable without ambiguity. The Argon2id encoded
hash includes algorithm parameters and salt, but it does not state whether its
input was a raw password or a legacy MD5 string.

Initial implementation records a scheme/version in
`user_info.password_scheme`, beginning with:

```text
argon2id-md5-v1
```

`argon2id-md5-v1` is implemented: its Argon2id input is the MD5 digest derived
from the submitted raw password. `argon2id-v1` is reserved for a future
direct-hash upgrade; the current login route does not authenticate it.

`user_info.password` is expanded to `text` because Argon2id PHC-formatted
hashes do not fit the legacy `char(32)` storage type.

## Login Verification

For a migrated account using `argon2id-md5-v1`, login verifies the existing
password without storing MD5 again:

```text
submitted_password -> md5 in memory -> Argon2id verify against stored hash
```

Expected flow:

1. User submits name and password over HTTPS.
2. Backend trims the submitted user name and locates an exact matching
   `user_info.name`.
3. Backend reads the credential scheme.
4. Backend denies login when `user_info.level = 0`, which identifies a frozen
   account. `user_info.state` does not affect authentication eligibility.
5. For `argon2id-md5-v1`, backend computes `md5(submitted_password)` in
   memory and passes that derived string to Argon2id verification.
6. Backend returns a generic authentication failure for unknown, frozen,
   unmigrated, unsupported-scheme, or incorrect-password accounts.
7. Backend establishes an authenticated session on success.

The MD5 intermediate should exist only transiently during verification; it
must not be logged, returned in an API response, or stored as a standalone
credential.

To reduce a basic timing distinction for missing, frozen, and unmigrated
accounts, the handler also performs an Argon2id hashing operation before
returning their generic failure response.

## Later Direct-Hash Upgrade

After a successful migrated-user login, the backend temporarily has the user's
raw submitted password and could replace the wrapped credential with:

```text
argon2id(raw_password, random_salt, parameters)
```

and set its scheme to:

```text
argon2id-v1
```

This transparent upgrade does not ask the user to choose a new password and
removes the MD5 derivation from future verification for users who return.

This improvement is recommended, but is not yet accepted as a required
behavior. It requires an authenticated login write path and explicit approval
before any real database modification.

New registrations and password changes should use direct
`argon2id(raw_password)` from the start.

## Argon2id Configuration

Use Argon2id for modern password storage and the wrapped migration. Its
configuration must be centralized so parameters can be reviewed and adjusted
later.

Current application configuration:

```text
algorithm: Argon2id
version:   0x13
memory:    19 MiB (19456 KiB)
passes:    2
parallel:  1
salt:      unique random salt generated per credential
```

Exact parameters should be benchmarked for the deployment environment before
production deployment. Login response time must remain acceptable while
making offline guessing expensive.

## Login Page

Route:

```text
/login
```

Entry point:

- The header `login` link shown to unauthenticated visitors navigates to
  `/login`.

Initial page content:

- Shared header and footer.
- Focused login form requesting user name and password.
- Submit control.
- Neutral error state for invalid credentials or temporary failure.

The form should be accessible by keyboard, use proper labels, and avoid
revealing whether a specific user name exists.

## API Direction

Implemented endpoints:

```text
POST /api/auth/login
POST /api/auth/logout
GET  /api/auth/session
```

`POST /api/auth/login` accepts:

```json
{"name": "user name", "password": "raw password"}
```

Successful login and session responses expose only the authenticated user's
public session identity (`id`, `name`, and `level`). Authentication failure
returns the same neutral response for an unknown, frozen, unmigrated, or
incorrect-password account.

For authentication eligibility, `user_info.level = 0` identifies a frozen
account and is denied login. `user_info.state` is not used to decide whether
an account may authenticate.

Login requests must not expose credentials in URLs, logs, browser history, or
cacheable responses.

## Session Direction

The initial implementation uses server-managed sessions with a random opaque
cookie held in application memory. Sessions are intentionally not stored in
PostgreSQL because adding persistent session tables needs separate database
change approval.

Session requirements:

- Cookies must be `HttpOnly`.
- Cookies must be `Secure` in HTTPS deployments.
- Use an appropriate `SameSite` policy, initially expected to be `Lax` or
  stricter.
- Session identifiers must be random and not derived from credentials.
- Logout invalidates the server-side session.
- Sessions should expire and support renewal policy decisions later.
- Sessions are cleared when the server process restarts until durable
  persistence is designed.

Runtime options:

```text
SESSION_TTL_SECONDS    default: 43200
SESSION_COOKIE_SECURE  default: false for local HTTP development
```

Set `SESSION_COOKIE_SECURE=true` when serving through HTTPS.

Authentication is separate from authorization. The first implemented
authorization rule protects encrypted post content: anonymous visitors may
see post metadata, but a live login session is required for encrypted body
text, related resource locations, signature content, detailed point awards,
and encrypted-only local image files. Future write operations require
additional design.

### Current Cookie and Session Behavior

- Successful login returns the `dogn_session` cookie.
- The cookie uses `Path=/`, `HttpOnly`, `SameSite=Lax`, and `Max-Age` derived
  from `SESSION_TTL_SECONDS`.
- The cookie includes `Secure` only when `SESSION_COOKIE_SECURE=true`.
- The server stores only an opaque token mapping and the public session
  identity (`id`, `name`, and `level`) in application memory.
- `GET /api/auth/session` returns that public identity for a live session.
- `POST /api/auth/logout` removes the server-side session and expires the
  browser cookie.

## Security Requirements

- Serve login and authenticated sessions only over HTTPS in deployment.
- Return the same login failure message for unknown users and incorrect
  passwords.
- Apply request throttling, rate limiting, or progressive delay to repeated
  failed attempts.
- Do not log raw passwords, derived MD5 inputs, Argon2id hashes, or session
  identifiers.
- Use parameter-bound SQL queries for account lookup and session storage.
- Protect future authenticated state-changing requests from CSRF.
- Invalidate affected cached data after future authenticated writes.
- Keep cached post summaries visibility-aware so protected link/image
  locations cannot be returned through an anonymous cached response.
- Mark session-dependent browser responses `Cache-Control: no-store` so
  protected content returned before logout is not reused afterward.

## Database Change Boundary

The following actions require database schema or data changes and therefore
must be separately approved before execution:

- Executing the generated schema/data migration against `user_info`.
- Transparently converting a returning user to direct Argon2id storage.
- Creating durable session tables or other authentication persistence
  structures.

Until explicit approval is given, authentication work may design and implement
code and scripts, but must not modify the real database.

## Testing and Operational Checklist

Before enabling login against a migrated database:

1. Confirm a backup/recovery plan for source credential data.
2. Run the executable transformation utility, not the standalone schema SQL
   fragment.
3. Confirm transformation output and credential scheme counts without
   exposing password hashes.
4. Configure `SESSION_COOKIE_SECURE=true` when deploying over HTTPS.
5. Test a known non-frozen migrated account with its original raw password.
6. Test rejection of an invalid password and a frozen (`level = 0`) account.

Automated coverage currently checks:

- Argon2id-over-MD5 hashing verifies the original raw password.
- A migrated account authenticates and can establish and clear a session.
- `state` does not prevent authentication for an otherwise valid account.
- A `level = 0` account, an unmigrated account, and an unknown account receive
  the generic authentication failure.
- Anonymous encrypted-post responses expose metadata but redact body
  resources; logged-in responses expose the protected content.
- Encrypted-only local image files require a logged-in session.

## Open Questions

- Whether to require transparent upgrade from `argon2id-md5-v1` to direct
  `argon2id-v1` after a migrated user's successful login.
- Exact Argon2id parameters after local performance benchmarking.
- Whether to remove, separately migrate, or strictly archive legacy password
  material in `info_bak.password`.
- User name matching rules, including case sensitivity and normalization.
- Durable session persistence and renewal behavior.
- Rate-limit storage and failure-tracking behavior.
- Authorization rules for future write endpoints.
- Account recovery and password-change workflows.
