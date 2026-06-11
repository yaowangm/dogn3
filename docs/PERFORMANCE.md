# Server Performance

This document records server-side performance decisions, completed
behavior-preserving improvements, and database work that must be reviewed and
applied separately.

## Current Runtime Approach

The application uses SQLx with a configurable PostgreSQL connection pool.
`DATABASE_MAX_CONNECTIONS` defaults to `5`, so request handlers should avoid
both unnecessary sequential round trips and unbounded query fan-out.

Current principles:

- Keep user input in SQLx bind parameters.
- Run independent reads concurrently when their inputs are already known.
- Preserve sequential ordering for reads or writes with real dependencies.
- Prefer one grouped aggregate over several correlated scans of the same rows.
- Write search SQL with only the active predicates so PostgreSQL can choose the
  relevant PGroonga or B-tree indexes.
- Keep denormalized statistics transactionally consistent with post mutations.
- Validate index changes against representative production traffic and query
  plans rather than relying only on static inspection.

## Implemented Code-Only Improvements

The following changes require no database schema update:

### Page query concurrency

- Home cache misses fetch independent post, user, and board sections
  concurrently.
- Board pages overlap authentication, board metadata, visible-post count, and
  navigation reads. Announcement and paged-post reads then run concurrently.
- User pages overlap profile and authentication reads, then overlap independent
  profile sections, activity counts, and navigation.
- Post pages overlap session validation with the primary post lookup and then
  overlap permission, favorite, signature, reply-window, tree, and navigation
  reads.
- Post-list and print pages overlap independent authentication/navigation
  reads where their required identifiers are already known.
- Site-manager overview sections load concurrently.

These handlers still respect the configured pool limit; SQLx queues excess
acquisitions rather than creating connections beyond the configured maximum.

### Statistics aggregation

Post/user/board statistic refreshes use grouped aggregates and PostgreSQL
`FILTER` clauses. Each affected post set is scanned once to derive counts and
latest timestamps instead of once per statistic.

This applies to:

- user statistics after post mutations
- board statistics after post mutations
- statistics for users affected by tree deletion
- explicit user-statistics recalculation
- site-wide board-statistics recalculation

### Search predicate construction

Search SQL contains only conditions selected by the user. For example, a
subject-only search emits a direct predicate:

```sql
COALESCE(p.subject, '')::text &@ $1
```

It no longer surrounds every optional condition with an empty-parameter `OR`.
This gives PostgreSQL a clearer opportunity to choose the matching partial
PGroonga index while preserving bound parameters and controlled ordering.

### Award date range

The once-per-day root-post award check uses a timestamp range:

```sql
post_time >= CURRENT_DATE
AND post_time < CURRENT_DATE + INTERVAL '1 day'
```

This avoids applying `::date` to every stored timestamp and is compatible with
ordinary timestamp indexes.

### Site-manager grouping

Board masters are grouped by board in one Rust pass. The response builder no
longer scans the complete master list once for every board.

### Board navigation cache

All page APIs use one shared board-navigation query helper. When Redis caching
is enabled, the ordered board/category list is cached under a dedicated key.
Category and board create/update/delete operations invalidate both navigation
and home caches. When Redis is disabled or unavailable, the helper continues
to query PostgreSQL and preserves the same response.

## Deferred Database Work

The following changes require a separate database migration and live-plan
review. They are intentionally not part of the code-only optimization pass.

### Signature identity and locking

Current signature insertion locks the entire `sign_log` table and calculates
`MAX(id) + 1`. The target design is:

1. Confirm that `sign_log.id` has a working identity/sequence default in every
   deployed database.
2. Repair the sequence value to at least the current maximum id.
3. Change insertion to omit `id`.
4. Remove the table-wide exclusive lock.
5. Test concurrent signature changes.

### Unique normalized user names

User creation currently locks `user_info` to prevent duplicate trimmed names.
The target design is:

1. Detect duplicate `BTRIM(name)` values.
2. Resolve any duplicates manually.
3. Add a unique expression index on `BTRIM(name)`.
4. Replace the table lock and pre-check with insert/conflict handling.

### Unique favorites

Favorite writes use a transaction advisory lock because the database does not
currently guarantee uniqueness of `(user_id, post_id)`.

Planned work:

1. Detect and resolve duplicate relationships.
2. Add a unique constraint or unique index on `(user_id, post_id)`.
3. Use `INSERT ... ON CONFLICT DO NOTHING`.
4. Reassess whether the advisory lock remains necessary.

### Root-award lookup index

After changing the award query to a timestamp range, inspect its live plan.
If needed, add a partial index beginning with `(user_id, post_time)` for visible
root posts. The exact predicate should match the production root-row
representation.

### User-directory substring search

The administrator user list performs case-insensitive substring matching.
Ordinary B-tree indexes cannot accelerate arbitrary middle-of-string matches.
Candidate solutions are PGroonga expression indexes or `pg_trgm` indexes on
normalized name and email values. Choose only after measuring table size,
search frequency, and extension availability.

### Index cleanup

Use `scripts/review_index_usage.sql` after representative traffic and confirm
plans with `EXPLAIN (ANALYZE, BUFFERS)`. Candidate redundant or stale indexes
include prefix duplicates and indexes with no current filter/order path.

Do not remove an index solely because `idx_scan` is zero after statistics were
recently reset.

### Effective-root expression consistency

Most tree indexes and reads use `COALESCE(root_id, id)`, while a few defensive
legacy paths use `COALESCE(NULLIF(root_id, 0), id)`. PostgreSQL expression
indexes require matching expressions.

Before simplifying those paths:

1. Verify every deployed database has no `root_id = 0` rows.
2. Decide whether zero remains a supported legacy representation.
3. Normalize stored data if necessary.
4. Standardize runtime SQL and expression indexes on one effective-root
   expression.

## Deferred Application Design

The browser requests `/api/auth/session` alongside the route API, and each
authenticated endpoint validates the in-memory session against `user_info`.
Major HTML shells may also query page-specific Open Graph metadata. Combining
these requests could remove database work, but it would change API response
contracts or weaken immediate frozen-account/role-change detection.

That work is deferred until the authentication response contract and acceptable
validation freshness are explicitly decided. It is not a safe
behavior-preserving refactor.

## Live Verification Plan

When read-only access to the target database is explicitly approved:

1. Capture table and index sizes.
2. Capture `pg_stat_user_indexes` after representative traffic.
3. Run `EXPLAIN (ANALYZE, BUFFERS)` for home, board pagination, post tree,
   user activity, each PGroonga search field, award lookup, and statistics
   refresh source queries.
4. Confirm partial-index predicates and expression forms match runtime SQL.
5. Propose one migration containing only changes supported by those results.
6. Run the migration first against the disposable test database.
