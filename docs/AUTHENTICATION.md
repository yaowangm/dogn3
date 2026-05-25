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
- Authenticate through JSON API routes and currently maintain opaque in-memory
  server sessions. Replace in-memory storage with Redis-backed opaque sessions
  to preserve login across application restarts while retaining revocation.

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
| `password_scheme` | Identifies which input was hashed. Supported values are `argon2id-md5-v1` for migrated credentials and `argon2id-v1` for changed passwords. |

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
- Rows marked `argon2id-v1` are recognized and left unchanged; they are
  produced by password changes and accepted by login, not by this migration
  utility.
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

`argon2id-md5-v1` identifies an Argon2id hash whose input is the MD5 digest
derived from the submitted raw password. `argon2id-v1` identifies an Argon2id
hash whose input is the submitted raw password itself. Login supports both
schemes. Password changes always replace either form with `argon2id-v1`.

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
   memory and passes that derived string to Argon2id verification. For
   `argon2id-v1`, it verifies the raw submitted password directly.
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

## Password Change And Administrative Reset

### Implemented Endpoint

```text
POST /api/users/{user_id}/password
```

Request body:

```json
{
  "current_password": "required for a non-administrator changing their own password",
  "new_password": "new credential",
  "confirm_password": "same new credential"
}
```

Successful password changes update only the selected active credential:

```text
user_info.password        = argon2id(new_password, random_salt, parameters)
user_info.password_scheme = argon2id-v1
```

This means a migrated account stops depending on the MD5 compatibility input
as soon as its password is changed. The endpoint does not update
`info_bak.password`.

### Authorization Rule

The backend enforces these rules independently from whether the profile page
shows a control:

| Requester | Target account | Current password required | Allowed |
| --- | --- | --- | --- |
| Anonymous | Any | N/A | No |
| Member or advanced member | Self | Yes | Yes |
| Member or advanced member | Another user | N/A | No |
| Administrator (`level >= 10`) | Self or any user | No | Yes |

An administrator reset is deliberately powerful: it allows replacing any
user's credential without knowing the current password. Administrative
account security and future audit logging therefore have direct impact on all
accounts.

### Password Policy

The implemented project policy for a newly selected password is:

```text
length:              8 to 30 characters inclusive
required classes:    at least one ASCII letter, one ASCII digit,
                     and one ASCII punctuation symbol
accepted characters: visible ASCII characters only (byte range 33..126)
rejected characters: spaces, control characters, and all non-ASCII input
```

The endpoint applies this policy to both owner changes and administrator
resets; the UI validation is guidance only and the server remains
authoritative.

This policy follows the requested legacy-site constraint but is narrower than
general modern password guidance: it excludes Unicode passphrases and limits
password-manager output to 30 characters. Revisit it before public deployment
if compatibility does not require those restrictions.

### Operation Flow

For an owner who is not an administrator:

1. Require an authenticated session matching `{user_id}`.
2. Validate new password confirmation and policy.
3. Read the target credential and verify `current_password` according to its
   recorded `password_scheme`.
4. Store a newly salted direct Argon2id hash and set `argon2id-v1`, only if
   the verified stored hash and scheme have not been concurrently replaced.
5. Invalidate all live sessions for the changed account.
6. Return success; the browser returns the affected user to login.

For an administrator:

1. Require an authenticated session whose user level is at least `10`.
2. Validate new password confirmation and policy.
3. Do not request or verify the target user's existing password.
4. Store a newly salted direct Argon2id hash and set `argon2id-v1`.
5. Invalidate all live sessions for the target account.
6. If the administrator reset their own password, return them to login;
   otherwise their administrator session remains valid.

Password hashing and current-password verification share the configured
concurrency bound used by login to prevent unbounded Argon2id work.

### Request And Session Protection

The profile page sends password changes as JSON and includes
`X-Dogn-Request: fetch`. The password-change endpoint rejects a request
without that custom header; cross-site HTML form submissions cannot set it.
The existing `SameSite=Lax` session cookie is an additional barrier. Deploy
authenticated operation pages over HTTPS.

Every successful password change invalidates every in-memory session for the
target account. When durable Redis sessions are introduced, invalidation must
remain account-wide rather than affecting only the browser that submitted the
change.

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
POST /api/users/{user_id}/password
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
SESSION_TTL_SECONDS    default: 604800 (7 days)
SESSION_COOKIE_SECURE  default: false for local HTTP development
LOGIN_MAX_CONCURRENT_HASHES default: 2
```

Set `SESSION_COOKIE_SECURE=true` when serving through HTTPS.
`LOGIN_MAX_CONCURRENT_HASHES` bounds simultaneous Argon2id work so a burst of
login attempts cannot allocate password-hash resources without limit. This is
resource protection, not a substitute for per-client failed-attempt policy.

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
- `POST /api/auth/login` returns `429 Too Many Requests` with `Retry-After`
  when all configured password-hash permits are in use.

### Restart Expiration And Seven-Day TTL

`SESSION_TTL_SECONDS` now defaults to `604800` seconds (7 days). In the
current implementation this value controls:

- The browser cookie `Max-Age`.
- The expiration timestamp on the matching server-side in-memory session.

The cookie does not contain authenticated user state that the server can
recover after restart. It contains an opaque random token only. When the
application process restarts, its in-memory token map is empty; a browser may
still send its unexpired cookie, but the backend cannot resolve that token and
treats the client as logged out.

Therefore, a seven-day TTL means "up to seven days while the server process
retains the session entry", not "login survives a server restart".

### Stateless Signed Token Alternative

A restart-surviving token without server-side session storage is possible, but
it should not be implemented using BCrypt or Argon2id. BCrypt and Argon2id are
password hash functions: they are deliberately expensive and one-way, making
them appropriate for password verification but not for authenticating every
HTTP request.

A stateless session cookie could instead contain signed claims such as:

```text
user_id, issued_at, expires_at, token_version
```

The backend would validate those claims using a persistent application signing
secret, for example with an HMAC-protected format or a carefully configured
signed token standard. The expiration claim would remain valid after restart
provided the same signing secret is available.

This option has important drawbacks for this forum:

- Logout clears the browser cookie but cannot invalidate a copied token
  immediately without a server-side revocation mechanism.
- Freezing a user, changing a password, or withdrawing administrator
  privileges cannot promptly revoke an already issued token without additional
  state.
- Embedding authorization claims such as `level` allows privilege changes to
  lag until token expiry unless each request refreshes authorization from a
  trusted source.

Confidential profile data must never be stored in a browser-held session token.

### Recommended Persistent Session Direction

Use Redis-backed opaque server sessions rather than stateless authentication.
Redis is already an optional infrastructure dependency for endpoint caching
and naturally supports expiring key/value entries.

Proposed behavior:

1. Login continues to verify `user_info.password` using the existing supported
   password scheme.
2. On successful login, generate a random opaque session token exactly as now.
3. Store the token-to-session mapping in Redis with a TTL derived from
   `SESSION_TTL_SECONDS`.
4. Send only the opaque token in the browser cookie.
5. Resolve the token through Redis on each authenticated request.
6. Delete the Redis session key on logout.
7. Provide an invalidation path for password changes, account freezing, and
   administrative privilege removal.

Benefits over a stateless signed cookie:

- Server restarts do not invalidate otherwise-live sessions.
- Logout can invalidate the token immediately.
- A compromised token can be revoked.
- Account or privilege changes can invalidate active sessions immediately.
- The browser never holds role or confidential profile claims.

Redis outage policy must be fail closed: when an authenticated session cannot
be validated, protected actions and protected content must be denied rather
than relying on stale browser state.

This is a design recommendation only. Redis-backed session persistence has not
yet been implemented.

References:

- RFC 7519, JSON Web Token and expiration claim: <https://www.rfc-editor.org/rfc/rfc7519>
- OWASP Session Management Cheat Sheet:
  <https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html>

## Authentication Process

### Login Request

Current login processing is:

1. The browser submits user name and raw password as JSON to
   `POST /api/auth/login`.
2. The backend applies the configured limit on simultaneous Argon2id work; it
   returns `429 Too Many Requests` when no password-hash permit is available.
3. The backend trims the submitted user name and selects the credential record
   with a parameter-bound query.
4. A user with `level = 0` is not eligible to log in. `state` is not currently
   used for login eligibility.
5. A credential marked `argon2id-md5-v1` is verified by computing
   `md5(raw_password)` only in memory and verifying that derived value against
   the stored Argon2id PHC hash.
6. Unknown users, frozen users, unsupported or unmigrated credential schemes,
   and incorrect passwords receive the same generic failure response. The
   backend performs password-hash work for absent or ineligible credentials to
   reduce a basic timing distinction.
7. On success, the backend issues an opaque session token and returns public
   session identity only: `id`, `name`, and `level`.

The login endpoint must not log raw passwords, MD5 intermediates, stored hash
values, or generated session tokens.

### Authenticated Request

Current authenticated-request processing is:

1. The browser sends the `dogn_session` cookie.
2. The backend looks up the opaque token in its process-memory session store.
3. If a live mapping exists, the request has authenticated identity containing
   `id`, `name`, and `level`.
4. If no mapping exists or it is expired, the request is anonymous.
5. Session-dependent responses use `Cache-Control: no-store`.

When Redis session storage is implemented, step 2 changes from a process-memory
lookup to a Redis lookup with the same externally visible authorization
semantics.

### Logout Request

Current logout processing is:

1. The browser submits `POST /api/auth/logout`.
2. The backend removes the matching in-memory token mapping when present.
3. The response expires the cookie with `Max-Age=0`.
4. The browser returns to the prior page in anonymous state, so protected
   encrypted content is no longer shown.

With Redis-backed sessions, logout must delete the session entry in Redis
before returning the expired browser cookie.

## Authorization And Privileges

Authentication establishes identity; it does not by itself permit every
operation. Every backend endpoint must enforce authorization independently of
whether the frontend exposes its control.

### Role Interpretation

Known legacy levels and application interpretation:

| Role | `user_info.level` | Authentication | General meaning |
| --- | ---: | --- | --- |
| Anonymous visitor | N/A | Not logged in | Public reader only. |
| Frozen account | `0` | Login denied | Retained account record without active privileges. |
| Member | `1` | Login permitted | Normal authenticated user. |
| Advanced member | `5` | Login permitted | Legacy elevated role; additional privileges not yet defined. |
| Administrator | `10` | Login permitted | Administrative profile-update privilege currently recognized. |

An advanced member must not implicitly receive administrator privileges until
specific operations are designed and approved. Authorization comparisons must
be explicit rather than assuming every elevated numeric level is equivalent.

### Implemented Read Privileges

| Operation or data | Anonymous | Logged-in member / advanced / admin | Notes |
| --- | --- | --- | --- |
| Read public pages and post metadata | Allowed | Allowed | Includes metadata cards for encrypted posts. |
| Read normal post content (`state = 0`) | Allowed | Allowed | Deleted or unknown states fail closed. |
| Read encrypted post content (`state = 1`) | Denied | Allowed | Anonymous UI shows an encrypted placeholder. |
| Read protected link/image locations and signatures on encrypted posts | Denied | Allowed | Applies to detail, list, print, and local image access. |
| Read public user profile and activity lists | Allowed | Allowed | Encrypted activity cards retain metadata-only anonymous visibility. |
| Read confidential user profile details | Denied | Owner or administrator only | Includes last login IP, introducing user, and login count. |

The current content rule permits any successfully logged-in non-frozen user to
read encrypted post content. Board-specific, author-specific, or administrator-
only encrypted-content policies are not implemented.

### Implemented Control Visibility

The user profile response sets `can_update = true` only when:

```text
viewer.id = profile_user.id OR viewer.level >= 10
```

The UI uses this result to show operation icon controls for:

- Change password.
- Recalculate statistics.

The change-password control opens the implemented password-change form. The
recalculate-statistics control remains disabled presentation only. The
password-change endpoint repeats authorization on the backend; `can_update`
and hidden/visible controls are not sufficient security checks.

### Write Privilege Matrix

Password change is implemented. Other entries remain draft decisions for
future endpoint design:

| Future operation | Anonymous | Member | Advanced (`5`) | Administrator (`10`) | Required additional controls |
| --- | --- | --- | --- | --- | --- |
| Change/reset password | Denied | Own account only | Own account only | Any account without current password | Owners must verify current password; all changes store `argon2id-v1` and invalidate target sessions. |
| Update own public profile/introduction/signature | Denied | Own account only | Own account only | Own account; managing others requires separate decision | CSRF protection, validation, escaping, audit decision. |
| Recalculate own statistics | Denied | Own account only, if exposed | Own account only, if exposed | Any account only if administrative workflow is approved | Transactional recomputation, authorization, audit record. |
| Freeze/unfreeze account or alter role | Denied | Denied | Denied unless explicitly introduced later | Administrator only | Audit trail; invalidate affected sessions immediately. |
| Create/reply/edit post | Denied until designed | Intended for authenticated user | Same baseline unless moderation privilege is defined | Moderation privilege to be designed | CSRF protection, content validation, tree/order maintenance, cache invalidation. |
| Delete/hide/moderate post | Denied | Not defined | Not defined | Not defined | Define ownership/moderation model before implementation. |
| Favorite/unfavorite post | Denied until designed | Own favorites only | Own favorites only | Own favorites unless admin behavior is separately needed | CSRF protection and duplicate-favorite rule decision. |

### Authorization Invalidation Rules

When state-changing authentication or privilege features are introduced:

- Password change or password reset must invalidate the user's active
  sessions, except possibly a newly issued replacement session.
- Freezing an account must invalidate all active sessions for that account.
- Administrator or advanced-role removal must invalidate sessions whose cached
  identity could retain elevated access.
- Future profile and content writes must invalidate affected cached API data.
- All mutation endpoints must use CSRF defenses appropriate to cookie-based
  authentication.

## Security Requirements

- Serve login and authenticated sessions only over HTTPS in deployment.
- Return the same login failure message for unknown users and incorrect
  passwords.
- Keep simultaneous password-hash work bounded and define request throttling,
  rate limiting, or progressive delay for repeated failed attempts before
  public deployment.
- Do not log raw passwords, derived MD5 inputs, Argon2id hashes, or session
  identifiers.
- Use parameter-bound SQL queries for account lookup and session storage.
- Protect authenticated state-changing requests from CSRF. Password change
  currently requires the custom same-origin-fetch header and `SameSite=Lax`
  session cookie.
- Invalidate affected cached data after future authenticated writes.
- Keep cached post summaries visibility-aware so protected link/image
  locations cannot be returned through an anonymous cached response.
- Mark session-dependent browser responses `Cache-Control: no-store` so
  protected content returned before logout is not reused afterward.

## Database Change Boundary

The following actions require database schema or data changes and therefore
must be separately approved before execution:

- Executing the generated schema/data migration against `user_info`.
- Transparently converting a returning user to direct Argon2id storage during
  login.
- Creating durable PostgreSQL session tables or other authentication
  persistence structures inside `dogn`.

Redis-backed session storage does not require changing the `dogn` schema, but
implementation and deployment configuration still require separate acceptance
before replacing current in-memory session behavior.

Until explicit approval is given, authentication work may design and implement
code and scripts, but must not modify the real database.

The implemented password-change endpoint is an intentional application
mutation: invoking it updates the selected user's credential and invalidates
their application sessions.

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
- Unsupported post visibility states fail closed and are not readable.
- Protected-image denial responses are non-cacheable across login changes.
- Concurrent password-hash work is rejected when the configured capacity is
  exhausted.
- Direct `argon2id-v1` hashes verify raw passwords without the migrated MD5
  input.
- Password changes enforce authorization, requested password policy, direct
  Argon2id storage, and target-session invalidation.

## Open Questions

- Whether to require transparent upgrade from `argon2id-md5-v1` to direct
  `argon2id-v1` after a migrated user's successful login, in addition to the
  implemented upgrade on password change.
- Exact Argon2id parameters after local performance benchmarking.
- Whether to remove, separately migrate, or strictly archive legacy password
  material in `info_bak.password`.
- User name matching rules, including case sensitivity and normalization.
- Exact Redis-backed session persistence and renewal behavior, including Redis
  outage handling and broad per-user session invalidation.
- Whether stateless signed tokens should be rejected permanently or retained
  only as a documented alternative.
- Per-client rate-limit storage, trusted proxy address handling, and
  failure-tracking behavior.
- Final authorization rules for future write endpoints, including whether
  advanced members receive any moderation privileges.
- Account recovery workflow and administrator reset auditing.
