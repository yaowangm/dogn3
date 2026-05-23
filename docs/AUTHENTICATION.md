# Authentication Design Draft

This document records the initial authentication direction. It is a draft:
login UI, session management, credential migration, and authorization behavior
still require implementation and further decisions.

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

### What This Improves

An attacker who steals only the transformed database must evaluate the
configured Argon2id work factor for each attempted password guess, rather than
testing guesses directly with fast MD5.

### Remaining Limitation

The migrated credential is still derived from the legacy MD5 representation.
If an attacker already possesses an older dump containing the original MD5
hashes, rewriting the current database cannot remove the risk from that older
dump.

## Hash Scheme Identification

Credential format must be identifiable without ambiguity. The Argon2id encoded
hash includes algorithm parameters and salt, but it does not state whether its
input was a raw password or a legacy MD5 string.

Initial implementation should record a scheme/version, for example:

```text
argon2id-md5-v1
argon2id-v1
```

Possible schema approaches:

- Add a `password_scheme` column beside `user_info.password`.
- Rename the credential column to `password_hash` and add
  `password_scheme`.

The precise schema migration is not yet selected.

## Login Verification

For a migrated account using `argon2id-md5-v1`, login verifies the existing
password without storing MD5 again:

```text
submitted_password -> md5 in memory -> Argon2id verify against stored hash
```

Expected flow:

1. User submits name and password over HTTPS.
2. Backend locates the account by normalized login identifier.
3. Backend reads the credential scheme.
4. For `argon2id-md5-v1`, backend computes `md5(submitted_password)` in
   memory and passes that derived string to Argon2id verification.
5. Backend returns a generic authentication failure on mismatch.
6. Backend establishes an authenticated session on success.

The MD5 intermediate should exist only transiently during verification; it
must not be logged, returned in an API response, or stored as a standalone
credential.

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

Initial design target:

```text
algorithm: Argon2id
memory:    at least 19 MiB
passes:    at least 2
parallel:  at least 1
salt:      unique random salt generated per credential
```

Exact parameters should be benchmarked for the deployment environment before
authentication is released. Login response time must remain acceptable while
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

Likely endpoints:

```text
POST /api/auth/login
POST /api/auth/logout
GET  /api/auth/session
```

These routes are a design direction only; request and response formats are
still to be specified.

Login requests must not expose credentials in URLs, logs, browser history, or
cacheable responses.

## Session Direction

Server-managed sessions with an opaque cookie are the preferred initial
direction.

Session requirements:

- Cookies must be `HttpOnly`.
- Cookies must be `Secure` in HTTPS deployments.
- Use an appropriate `SameSite` policy, initially expected to be `Lax` or
  stricter.
- Session identifiers must be random and not derived from credentials.
- Logout invalidates the server-side session.
- Sessions should expire and support renewal policy decisions later.

Authentication is separate from authorization. Access rules for encrypted
posts and future write operations require additional design after session
identity is available.

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

## Database Change Boundary

The following actions require database schema or data changes and therefore
must be separately approved before execution:

- Adding credential scheme/version columns.
- Replacing MD5-only password values with Argon2id-wrapped values.
- Transparently converting a returning user to direct Argon2id storage.
- Creating session tables or other authentication persistence structures.

Until explicit approval is given, authentication work may design and implement
code and scripts, but must not modify the real database.

## Open Questions

- Whether to require transparent upgrade from `argon2id-md5-v1` to direct
  `argon2id-v1` after a migrated user's successful login.
- Which schema shape records credential hashes and schemes.
- Exact Argon2id parameters after local performance benchmarking.
- User name matching rules, including case sensitivity and normalization.
- Session persistence, lifetime, renewal, and logout behavior.
- Rate-limit storage and failure-tracking behavior.
- Authorization rules for encrypted posts and future write endpoints.
- Account recovery and password-change workflows.
