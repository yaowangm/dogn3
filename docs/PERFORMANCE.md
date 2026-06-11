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

## Completed Database Work

### Signature identity and locking

Section 1 of `scripts/apply_performance_improvements.sql` prepares an upgraded
database for concurrent signature-history insertion. It:

- reuses an existing serial/identity sequence when available
- creates and attaches `public.sign_log_id_seq` only when no sequence exists
- aligns the sequence with the current maximum `sign_log.id`
- does not renumber or modify existing signature-history rows

Runtime insertion omits `sign_log.id` and lets PostgreSQL generate it.

Signature assignment and non-administrator post update use the same
transaction-level advisory lock keyed by a signature namespace plus `post_id`.
This preserves the rule that a post used in any signature history cannot be
updated by a non-administrator, while unrelated signature/post operations no
longer block each other through table-wide `sign_log` locks.

Administrators do not need the advisory lock when updating a post because they
are explicitly permitted to update posts used as signatures.

### Effective root normalization

Section 2 normalizes the unsupported legacy `root_id = 0` representation to
`NULL`. Runtime tree queries now consistently use `COALESCE(root_id, id)`, which
matches the existing effective-root expression indexes.

Current root posts normally store their own id in `root_id`; this normalization
only repairs legacy zero values.

### Unique normalized user names

Section 3 first preserves the two known frozen, unreferenced legacy placeholder
accounts (`id = 535` and `id = 536`, both migrated as `?`) under the stable
names `legacy-user-535` and `legacy-user-536`. Each repair is guarded by both
the expected id/name, the verified frozen/unused account fields, and the
absence of references from active relationship tables. It is a no-op after the
first successful run. If either known id still has the placeholder name after
the guarded repair, an explicit preflight stops the migration for manual review
even when no other duplicate remains.

The section then verifies that no other duplicate `BTRIM(user_info.name)`
values exist. When upgrading a former non-unique index, it builds a temporary
unique replacement before dropping the old index and then renames the
replacement. This avoids an interval without uniqueness enforcement. Reruns
skip the replacement when the target index is already unique, and an
interrupted run reuses a valid prepared replacement instead of dropping it.
The migration aborts and reports representative remaining duplicates instead
of making an unapproved rename decision.

User creation now relies on this database guarantee and translates a matching
unique violation into the existing duplicate-name response. It no longer locks
the complete `user_info` table or performs a race-prone pre-insert check.
Authentication uses the same `BTRIM(name)` expression, allowing the unique
normalized-name index to support login lookup.

### Unique favorites

Section 4 verifies that no duplicate `(user_id, post_id)` relationships exist
and builds a unique replacement before removing a former non-unique index, so
the upgraded database never loses uniqueness after the replacement is ready.
Reruns skip the swap when the target index is already unique, and interrupted
runs reuse a valid prepared replacement. Favorite writes use
`INSERT ... ON CONFLICT DO NOTHING` or a direct delete, preserving idempotent
set/unset behavior without an advisory lock or separate existence query.

### Indexed user-directory search

Section 5 installs `pg_trgm` and adds GIN trigram indexes for normalized user
names and email addresses. The administrator directory emits name/email search
predicates only when a search term is present and escapes `%`, `_`, and `\` so
the public behavior remains literal, case-insensitive substring matching.

The page query obtains its total with `COUNT(*) OVER()` in the normal case. A
separate count is issued only for an empty or out-of-range page.

### Root-post award lookup

Section 6 adds a partial `(user_id, post_time, type)` index for root rows. The
once-per-day award query uses a timestamp range and direct type predicates so
the planner can use this index without applying functions to `post_time`.

### Stale index cleanup

Section 7 removes single-column and legacy indexes superseded by current
composite indexes or absent runtime paths. The retained indexes cover board
ordering, favorite activity paging, signature history, home/user ordering,
post search, post trees, and post mutation/statistics paths.

## Cumulative Deployment Script

All database modifications required by the performance-improvement work are
kept in:

```text
scripts/apply_performance_improvements.sql
```

Future performance migrations must be appended to this script in execution
order. Sections should be rerunnable so the complete script can update another
real database after all performance tasks are finished.

Run the complete script with:

```bash
psql dogn -v ON_ERROR_STOP=1 -f scripts/apply_performance_improvements.sql
```

The earlier standalone `scripts/prepare_sign_log_id_sequence.sql` was folded
into this cumulative script and removed.

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
