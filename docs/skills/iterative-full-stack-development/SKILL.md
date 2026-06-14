---
name: iterative-full-stack-development
description: Develop and evolve an existing database-backed web application through small, reviewable vertical slices. Use when work spans backend APIs, SQL/schema changes, frontend UI, authentication, caching, search, tests, configuration, deployment, or architecture documentation, especially for legacy migrations and long-running projects where behavior, data safety, and operational compatibility must remain aligned.
---

# Iterative Full-Stack Development

Use an evidence-driven loop that keeps code, database state, APIs, UI,
configuration, tests, operations, and documentation synchronized.

Read [references/lessons.md](references/lessons.md) when planning substantial
work, debugging a cross-layer failure, designing a migration, or reviewing the
quality of a completed feature.

## 1. Establish Boundaries

Before editing:

- Read repository agent instructions and local design documents.
- Inspect the worktree and preserve unrelated user changes.
- Identify explicit restrictions for commits, pushes, databases, package
  installation, server control, and production services.
- Distinguish read-only inspection from mutation. Require explicit approval for
  destructive or production-data changes.
- State the behavioral contract: what must change and what must remain stable.

Do not treat an operational symptom as proof of a code defect. Gather logs,
runtime configuration, filesystem ownership, network topology, query plans, or
database rows before choosing a fix.

## 2. Learn The Existing System

Inspect before designing:

- Trace the complete request path: route, handler, SQL, DTO, frontend fetch,
  renderer, cache, and tests.
- Prefer existing framework patterns, helpers, naming, and component styles.
- Read schema and relationship documentation before inferring domain rules from
  column names.
- Search history and docs for prior decisions before introducing a competing
  approach.
- Verify assumptions against representative data or primary runtime evidence
  when access is allowed.

Record unresolved semantics instead of silently encoding guesses.

## 3. Design The Smallest Vertical Slice

Define the slice across all affected layers:

1. User-visible behavior and authorization.
2. API request/response contract.
3. Database reads, writes, constraints, and derived fields.
4. Cache behavior and invalidation.
5. Frontend states, accessibility, responsive behavior, and localization.
6. Configuration and deployment requirements.
7. Tests and documentation.

Start with the simplest architecture that satisfies current requirements. Add
abstractions only when they remove demonstrated duplication or complexity.

For uncertain architecture, write a draft design first. Mark open decisions
clearly and update the design after implementation choices become concrete.

## 4. Implement From Invariants

Encode domain invariants at the strongest practical layer:

- Use database constraints for uniqueness and referential integrity.
- Use transactions for multi-table writes and denormalized statistics.
- Recheck authorization and time-sensitive rules at mutation time.
- Use parameterized SQL and typed DTOs.
- Keep cache data derived from the database, never authoritative.
- Invalidate caches only after successful commits.
- Preserve identical observable behavior when optional cache services are
  disabled.
- Keep protected content out of shared caches and crawler metadata.

When a mutation affects counts, timestamps, points, roles, ordering, or history,
enumerate every affected table and field before coding. Prefer one atomic
transaction over best-effort follow-up updates.

## 5. Evolve The Database Safely

For schema or index work:

- Generate a reviewable SQL script; do not apply it to a real database without
  permission.
- Add preflight checks for unexpected data and abort instead of guessing.
- Make deployment migrations rerunnable where practical.
- Avoid gaps in uniqueness or integrity enforcement during index replacement.
- Preserve existing IDs and data unless the approved change requires otherwise.
- Keep a cumulative upgrade path when multiple deployments must reach the same
  target schema.
- Validate runtime SQL and indexes together with representative
  `EXPLAIN (ANALYZE, BUFFERS)` output, not index names alone.
- Update schema fixtures, sample initialization SQL, and database docs.

Separate one-time deployment scripts from normal runtime migrations and label
their starting-state assumptions.

## 6. Build The UI As A Client

Keep page shells and JSON APIs separate when that is the project architecture:

- Render dynamic data from API responses.
- Reuse established components and icon conventions.
- Make the entire expected interaction target clickable.
- Handle loading, empty, error, unauthorized, encrypted, and missing-resource
  states explicitly.
- Escape user-controlled data before HTML insertion.
- Sanitize Markdown and external URLs with proven libraries or strict allowlists.
- Add translations for every new or changed interface string.
- Preserve desktop behavior while testing narrow layouts.
- Version static assets automatically; revalidate HTML while caching immutable
  versioned assets.

Do not infer external crawler or browser behavior from standards alone. Use
compatible formats and clearly report when real-platform verification was not
performed.

## 7. Keep Configuration And Deployment Honest

Whenever configuration changes:

- Keep real and sample config files aligned.
- Comment every option and keep samples free of secrets.
- Separate application runtime variables from container build/deployment
  variables, while documenting both where users will look.
- Log sanitized startup configuration and name the failing external service.
- Validate host/container path mappings, numeric UID/GID ownership, bind
  addresses, and service reachability.
- Treat bind mounts as host-owned filesystem state; image-layer ownership does
  not override mounted-directory permissions.

Do not fix a Docker problem in application code until the same failure is
reproduced outside container-specific boundaries.

## 8. Test At The Right Layers

Scale verification with risk:

- Unit-test pure validation, parsing, transformations, and policy helpers.
- Test routers in process without opening sockets.
- Use a disposable database with deterministic fixtures for database behavior.
- Keep production-like databases outside normal tests.
- Delete the test database on success and retain it on failure for diagnosis.
- Test optional services both enabled and disabled.
- Test security boundaries: anonymous/authenticated/admin, stale sessions,
  encrypted data, unsafe input, traversal, duplicate writes, and rate limits.
- Test the exact regression that motivated the change.
- Update stale tests when an intentional contract changes.

Report counts of passed, failed, and skipped tests. Do not claim an ignored test
ran successfully.

## 9. Diagnose Before Refactoring

For failures:

1. Reproduce through the supported control path.
2. Read server logs and identify the exact failing operation.
3. Inspect effective runtime configuration.
4. Check the external boundary directly: database, Redis, SMTP, filesystem,
   Docker network, browser cache, or crawler.
5. Compare the failing environment with a working one.
6. Change the narrowest responsible layer.
7. Add contextual logging and regression coverage.

Keep user-facing errors safe and stable, but log operation, path/endpoint,
error kind, and sanitized context server-side.

## 10. Finish The Slice

Before declaring completion:

- Review the diff for accidental behavior changes and unused code.
- Run formatting, build checks, focused tests, and the full test harness when
  appropriate.
- Verify docs describe the code that actually exists.
- Verify config samples expose all supported options.
- Verify migration and deployment commands match current files.
- State what was validated and what was not.
- Do not commit unless explicitly requested; never push unless explicitly
  permitted by repository rules.

Treat user feedback as new evidence. Reopen the relevant layer, find the root
cause, and correct both implementation and documentation instead of stacking
compatibility workarounds.
