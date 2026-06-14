# Lessons From A Long-Running Full-Stack Migration

## Contents

1. Architecture and scope
2. Database migration and domain discovery
3. Mutation consistency
4. Authentication and security
5. Cache and session behavior
6. Frontend architecture and UI evolution
7. Search and performance
8. Testing strategy
9. Configuration and deployment
10. Debugging and observability
11. Documentation and work style
12. Failure patterns to avoid
13. Reusable delivery checklist

## 1. Architecture And Scope

### Start lightweight and leave escape routes

The project succeeded with an explicit stack: Axum, SQLx, Serde, Tower HTTP,
PostgreSQL, native Web Components, and Redis. This worked because the migrated
schema and business rules required visible SQL and incremental discovery.

General lesson:

- Prefer explicit tools while domain knowledge is incomplete.
- Avoid committing to a large ORM or frontend framework before real workflows
  expose the need.
- Record architecture as a draft and revise it after implementation evidence.

### Deliver vertical slices

Feature branches were organized around complete pages and workflows: portal,
board, post, authentication, users, administration, post updates, search,
Markdown, and performance.

General lesson:

- A useful slice includes API, query, UI, authorization, tests, and docs.
- Avoid building disconnected infrastructure without a user workflow proving
  it.
- Merge stable slices before beginning a new domain area.

## 2. Database Migration And Domain Discovery

### Migration is not merely syntax conversion

The MySQL-to-PostgreSQL migration exposed naming, type, copy-format, identity,
relationship, and legacy-data issues. Later application work required table and
column normalization, relationship tables, indexes, constraints, and data
repairs.

General lesson:

- Separate raw data transfer from application schema upgrade.
- Keep an upgrade script that converts a freshly imported database to the
  current application schema.
- Document the required starting state for every migration script.

### Infer carefully, then verify

Column names revealed likely relationships and tree semantics, but some fields
had misleading legacy values. Root-post detection, user state/level meaning,
point ownership, and image paths all required correction after real data was
examined.

General lesson:

- Treat legacy naming as evidence, not truth.
- Use read-only queries to test assumptions when permitted.
- Encode verified semantics in database docs and tests.
- Preserve unresolved legacy columns until their meaning is known.

### Prefer stable identities over mutable names

Board masters were originally stored as several username columns. Replacing
them with a `board_master(board_id, user_id, order_id)` relation enabled proper
integrity, unlimited relationships, and reliable role maintenance.

General lesson:

- Relationships should use stable primary keys.
- Fixed repeated columns usually indicate a missing child or join table.
- Migrations should validate every textual reference resolves uniquely before
  dropping legacy data.

### Preflight before enforcing constraints

Adding a unique normalized username index uncovered duplicate placeholder
accounts. The safe approach was to abort, inspect, make a guarded repair, and
rerun.

General lesson:

- Scan for violations before adding constraints.
- Report representative conflicts.
- Guard known repairs with IDs, state, expected values, and absence of
  references.
- Never silently merge, delete, or rename ambiguous production data.

## 3. Mutation Consistency

### Derived data is part of the transaction

Post creation, replies, updates, deletion, favorites, points, user statistics,
board statistics, category counts, tree ordering, and role changes interact.
Updating only the primary row left stale counts and permissions.

General lesson:

- List all derived fields before implementing a write.
- Update authoritative rows, history rows, denormalized counts, timestamps,
  ordering, and role effects in one transaction.
- Lock only the logical resource that must serialize.
- Recalculate as a repair tool, not as the normal consistency mechanism.

### Concurrency changes correctness

Check-then-insert patterns for favorites and usernames were race-prone.
Database uniqueness plus `ON CONFLICT` or unique-violation handling was simpler
and safer.

General lesson:

- Prefer constraints over application-level existence checks.
- Use advisory or row locks for cross-row invariants that constraints cannot
  express.
- Build replacement unique indexes before dropping old enforcement.

### File publication must match database publication

An uploaded image can be written before the database transaction commits. The
implementation needs cleanup on failure and retention only after successful
publication.

General lesson:

- Model external side effects as pending resources.
- Remove uncommitted files on error.
- Mark them retained only after the database transaction succeeds.

## 4. Authentication And Security

### Legacy credentials need an explicit scheme

Existing accounts stored MD5-derived credentials. A one-time transformation
wrapped the legacy digest in Argon2id, while new password changes used direct
Argon2id. A `password_scheme` field distinguished verification paths.

General lesson:

- Never infer a credential scheme from hash appearance alone.
- Version schemes explicitly.
- Make migration tools resumable and auditable.
- Upgrade credentials toward the strongest direct scheme when raw passwords
  become available.

### Authorization must use current database state

Session claims became stale after role or account-state changes. Sensitive
operations needed to resolve the current account rather than trusting old
session roles.

General lesson:

- Treat sessions as identity evidence, not permanent authorization truth.
- Revalidate mutable privileges for protected operations.
- Test downgraded, frozen, expired, and logged-out sessions.

### Protect content at every representation

Encrypted post bodies had to be hidden in detail pages, lists, signatures,
images, caches, and crawler metadata. Fixing only one route was insufficient.

General lesson:

- Enumerate every representation of protected data.
- Metadata visibility and body visibility may differ.
- Apply `no-store` to session-sensitive responses.
- Invalidate or partition caches by authorization visibility.

### Use proven parsers and strict output rules

Markdown, math, plain-text URL linking, uploaded images, redirects, and
external links created XSS and resource risks.

General lesson:

- Escape before interpolation.
- Sanitize rendered Markdown.
- Permit only expected URL schemes.
- Validate file signatures as well as MIME claims.
- Restrict image dimensions, decoded allocation, file size, and processing
  concurrency.
- Use `noopener noreferrer` for external windows.

### Rate limits need a production dependency

Redis-backed rate limiting supported login and password-reset protection, with
memory fallback only for development.

General lesson:

- Document whether limits are process-local or shared.
- Keep public reset responses generic to avoid account enumeration.
- Bound expensive password hashing separately from request counts.

## 5. Cache And Session Behavior

### Optional means behaviorally optional

Redis was useful for caching and rate limiting, but the application needed to
work with caching disabled and preserve API behavior.

General lesson:

- Test enabled and disabled modes against the same response contract.
- Treat cache failures as either explicit startup failures or documented
  runtime fallbacks.
- Never let cache contents become the source of truth.

### Invalidation follows commits

Invalidating before a database transaction commits can expose stale or
inconsistent generations. Generation keys prevented late old writes from
becoming visible.

General lesson:

- Commit first, invalidate second.
- Use generation/version keys where races between reads and writes matter.
- If invalidation fails, prefer disabling cache reads over serving known-stale
  protected or statistical data.

### Server restarts expose session architecture

In-memory sessions expired on restart despite a seven-day TTL. This was not a
cryptographic issue; it was a storage-lifetime issue.

General lesson:

- Document where sessions live and what restart means.
- Do not confuse password hashing with session signing or persistence.
- Choose opaque server sessions or signed tokens based on revocation,
  persistence, privilege freshness, and deployment topology.

## 6. Frontend Architecture And UI Evolution

### API-first rendering supported reuse

HTML shells plus JSON APIs allowed the same backend contract to serve pages and
future clients. Native Web Components kept dependencies light.

General lesson:

- Keep API DTOs intentional rather than exposing database rows.
- Make routed shells load only their own page data to avoid portal flashes.
- Reuse components for repeated post, user, board, pager, and metadata
  patterns.

### Small visual changes still need complete state handling

Cards, pills, icons, menus, masks, responsive controllers, loading indicators,
and clickable areas evolved through repeated feedback.

General lesson:

- Test normal, hover, active, disabled, loading, empty, error, and narrow-screen
  states.
- Use one icon system and established design tokens.
- Do not let decorative overlays capture unintended clicks.
- Ensure the whole visual item matches the expected click target.

### Internationalization must be a standing rule

Retrofitting translation exposed English-only labels embedded in render
helpers. Making multilingual support part of every UI change prevented new
drift.

General lesson:

- Centralize translation lookup.
- Keep user content outside automatic translation.
- Add translations whenever labels, tooltips, placeholders, errors, or titles
  change.
- Include localization in code review.

### Browser persistence needs a threat model

Draft autosave improved resilience, but local storage retains sensitive post
content. Drafts needed user-scoped keys, session-bounded expiry, cleanup after
success, and best-effort cleanup on window closure.

General lesson:

- Decide explicitly whether encrypted content may be stored client-side.
- Scope local data by user and editing target.
- Expire it with the authentication session.
- Treat file inputs separately because browsers do not allow restoring file
  objects from paths.

### Static assets need automatic versioning

Long-lived immutable browser caching caused deployments to appear unchanged.
Content-derived asset versions and HTML revalidation solved it.

General lesson:

- Cache versioned assets as immutable.
- Serve HTML with `no-cache` and validators.
- Derive versions automatically from content.
- Update tests when the generated asset contract changes.

## 7. Search And Performance

### Choose search for the actual language

PostgreSQL simple full-text search did not adequately solve Chinese search, and
combining it with `ILIKE` allowed the planner to choose slow paths. PGroonga
matched the lexical CJK requirement better.

General lesson:

- Evaluate tokenization and language support before selecting FTS technology.
- Do not claim an index is used because it exists.
- Show the actual search method and timing when useful to operators.
- Keep vector search separate until semantic search is a real requirement.

### Optimize queries with evidence

Useful changes included concurrent independent reads, grouped aggregates,
active-predicate SQL construction, expression alignment, and removal of stale
indexes.

General lesson:

- Start from request traces and query plans.
- Match predicates exactly to partial/expression indexes.
- Avoid optional-parameter `OR` patterns that obscure selectivity.
- Use window counts where they remove a round trip without distorting the plan.
- Review index usage after representative traffic, not immediately after
  creation.

## 8. Testing Strategy

### Disposable integration databases reduce fear

The test harness created `dogn_test`, loaded deterministic fixtures, ran tests,
dropped it on success, and retained it on failure.

General lesson:

- Use a distinct environment variable for test database access.
- Reject dangerous database names.
- Keep fixtures small and purpose-built.
- Run database tests serially when they mutate shared fixture rows.
- Print passed, failed, and skipped counts.

### Test contracts, not implementations only

Effective tests covered authorization variants, cache equivalence, encrypted
data redaction, route JSON, migration rerunnability, interrupted migration
recovery, file serving, and UI shell metadata.

General lesson:

- Add a regression test for the reported failure.
- Test both success and rejection paths.
- When a contract changes intentionally, update its test in the same change.
- Do not describe an ignored test as passed.

### Performance tests are comparative, not absolute

Cache tests compared repeated cached and uncached local fixture requests rather
than asserting a universal latency threshold.

General lesson:

- Use relative measurements for noisy environments.
- Warm up both paths.
- Keep performance assertions coarse enough to avoid flaky tests.

## 9. Configuration And Deployment

### Configuration parity is part of the feature

New options repeatedly became confusing when `.env`, `.env.example`, and
Docker examples diverged.

General lesson:

- Update real and sample configurations together.
- Comment every option.
- Keep secrets out of samples.
- Distinguish runtime environment variables from Docker build or orchestration
  variables.

### Container paths are not host paths

`IMAGE_DIRECTORY=/app/images` required a host bind mount such as
`/home/user/images:/app/images`. Reversing the mapping or reusing a standalone
host path inside the container caused failures.

General lesson:

- Document mount syntax as `host_path:container_path`.
- Log the effective path at startup.
- Verify it from inside the running container.

### Bind mounts override image ownership

Changing ownership in a Dockerfile does not change the ownership of a mounted
host directory. Numeric UID/GID must match or permissions must permit writes.
Even outside Docker, a month subdirectory owned by another account can block
uploads while the root directory appears writable.

General lesson:

- Inspect every path component and the exact target directory.
- Log the failing path and OS error kind.
- Make container UID/GID configurable without assuming an ID is unused in the
  base image.
- Avoid creating named users/groups when a numeric runtime user is sufficient.

### Build environments affect binaries and caches

A host-built glibc binary required a compatible runtime image. Deleting images
or building dependencies inside Docker discarded useful caches.

General lesson:

- Define the build/runtime ABI contract.
- Reuse host Cargo caches when using a runtime-only image.
- Do not delete images before normal rebuilds.
- Prefer BuildKit/buildx where available, but keep the Dockerfile valid for the
  supported builder.

### External service errors need named context

Generic pool timeouts and missing sendmail binaries were hard to diagnose.
Sanitized startup logs identifying PostgreSQL, Redis, SMTP, paths, and
endpoints made deployment failures actionable.

General lesson:

- Name the dependency and sanitized endpoint in errors.
- Never log credentials.
- Validate service reachability from the application network namespace.

## 10. Debugging And Observability

### Follow evidence across layers

Several apparent application bugs were configuration, filesystem ownership,
browser caching, stale Redis data, or missing database extensions.

General diagnostic sequence:

1. Reproduce through the supported server/controller path.
2. Inspect server logs for the request and failure.
3. Confirm effective configuration.
4. Test the external dependency directly.
5. Inspect the exact database row, filesystem target, or network endpoint.
6. Compare with a known-working environment.
7. Patch the responsible layer and add a regression test.

### Keep public errors safe, logs specific

Users should see stable messages such as "image storage unavailable", while
operators need the operation, exact path, error kind, and OS message.

General lesson:

- Separate public error contracts from diagnostic context.
- Include correlation/request context when the logging stack supports it.

## 11. Documentation And Work Style

### Documentation is executable memory

Architecture, database, page, authentication, testing, search, performance,
rate limiting, password reset, internationalization, and Docker documents
prevented repeated rediscovery.

General lesson:

- Update the relevant design document with every durable decision.
- Keep "current behavior", "migration procedure", and "future direction"
  distinct.
- Remove outdated approaches rather than accumulating contradictory guidance.
- Include commands, prerequisites, safety boundaries, and verification queries.

### Commit discipline preserves user control

The project explicitly separated implementation from commits and prohibited
pushes without permission.

General lesson:

- Never assume the user wants a commit.
- When asked to commit, commit the current intended work promptly with a
  descriptive message.
- Keep feature branches focused and use pull requests for integration.

### User feedback is evidence, not noise

Visual and behavioral refinements often revealed hidden requirements. The best
responses traced the actual rendered HTML, API payload, logs, or data rather
than defending the original implementation.

General lesson:

- Reproduce what the user sees.
- Ask only when semantics cannot be discovered safely.
- Correct root causes and clean obsolete compatibility code.

## 12. Failure Patterns To Avoid

### Assuming a platform contract without verification

The Open Graph work initially conflated a large content preview image with the
small site icon and claimed likely WeChat behavior without direct verification.

Avoidance:

- Separate standards-defined metadata from platform-specific presentation.
- State clearly what was tested locally and what was not verified externally.
- Model optional content images separately from always-present favicon/site
  identity.

### Fixing the wrong environment

A Docker UID/GID change did not solve a standalone application upload failure.
The actual failing month directory had different ownership.

Avoidance:

- Confirm whether the process runs on the host or in a container.
- Inspect the exact failing path before modifying deployment machinery.

### Updating code without updating tests

Changing `og:image` broke a shell assertion because the contract test still
expected the old path.

Avoidance:

- Search tests and docs for every changed route, field, option, asset, and
  message.
- Run the focused test immediately after changing a contract.

### Adding configuration without exposing it everywhere

Docker UID/GID build arguments were implemented before both sample files and
the deployment flow documented them.

Avoidance:

- Treat config implementation, samples, validation, startup logging, and docs
  as one atomic change.

### Over-generalizing user-facing errors

Multiple image-processing and storage failures shared one public message,
making root-cause analysis difficult.

Avoidance:

- Keep the public message generic when needed, but log operation-specific
  context.

### Trusting legacy fields too early

Root IDs, user state/level, point-log ownership, and manager names produced
incorrect behavior when interpreted from names alone.

Avoidance:

- Validate semantics with data, legacy code, and user confirmation.
- Add tests for corrected edge cases.

## 13. Reusable Delivery Checklist

### Before coding

- Read repository rules and relevant design docs.
- Inspect worktree and current branch.
- Trace the existing request/data path.
- State behavioral and authorization invariants.
- Identify database, cache, config, UI, test, and deployment impact.
- Ask for approval before real database mutation.

### During implementation

- Follow existing patterns.
- Bind SQL parameters.
- Keep writes atomic.
- Maintain derived statistics and history.
- Invalidate caches after commit.
- Preserve behavior without optional services.
- Escape and sanitize user-controlled content.
- Add translations for UI strings.
- Keep config files aligned and commented.
- Add exact regression coverage.

### Before completion

- Run formatter and build checks.
- Run focused and full tests as appropriate.
- Verify migration rerunnability and rejection behavior.
- Check narrow layouts and accessibility for UI changes.
- Review logs and error context.
- Update design, database, deployment, and testing docs.
- Search for stale code, docs, tests, assets, and configuration.
- Report what was and was not verified.
- Commit only when explicitly requested; never push without permission.
