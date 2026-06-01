# Architecture Draft

This document is a working draft. It records the current architectural direction
and design principles, but many details are intentionally still open. The goal
is to start with a simple, flexible system and refine it as real product
requirements become clearer.

## Status

Current decision:

- Build a PostgreSQL-backed Rust web application.
- Start with a lightweight backend stack: `axum`, `sqlx`, `serde`, and
  `tower-http`.
- Build the frontend with HTML5, CSS3, native Web Components, and JSON APIs.
- Keep the frontend and backend separated by REST-style HTTP endpoints.
- Use Redis as a lightweight cache layer.

This is not yet a final architecture. The first implementation should validate
the stack, the migrated database shape, and the first real user workflows before
adding more framework structure.

## Goals

- Keep the system lightweight, fast, straightforward, and flexible.
- Preserve Rust's explicitness instead of hiding too much behavior behind a
  large framework.
- Keep SQL visible and reviewable while the legacy schema is being understood.
- Provide a stable JSON API that can serve the web UI and future clients.
- Keep frontend implementation simple, accessible, fast, and visually
  consistent.
- Leave room to change direction if the product or codebase demands it.

## Initial Stack

- Backend language: Rust
- Web framework: `axum`
- Async runtime: `tokio`
- Database: PostgreSQL
- Database access: `sqlx`
- Cache: Redis
- JSON serialization: `serde`
- HTTP middleware and services: `tower-http`
- Frontend: HTML5, CSS3, native Web Components
- Browser/backend communication: REST-style Ajax using JSON

This stack is close in spirit to FastAPI and SQLModel in the Python ecosystem,
but uses a more explicit Rust style. `sqlx` is not a full ORM; that is an
intentional starting choice because the migrated schema already exists and
explicit SQL should make early behavior easier to verify.

## Backend Architecture

The backend should start as a small `axum` application with clear boundaries:

- Routing: maps HTTP routes to handlers.
- Handlers: parse requests, call application logic, and return JSON responses.
- Application services: hold workflow-level behavior that should not live in
  HTTP handlers.
- Repositories or query modules: contain SQLx queries and database mapping.
- DTOs: define request and response JSON structures with `serde`.
- Configuration: centralizes database URL, server address, logging level, and
  other runtime settings.

Handlers should stay thin. Database queries should not be scattered directly
through unrelated code. If a query is reused or carries business meaning, it
belongs in a dedicated query/repository module.

## Database Access

`sqlx` is the preferred first database layer.

Guidelines:

- Use explicit SQL for important reads and writes.
- Prefer typed result structs for query outputs.
- Keep database table structs separate from public API DTOs when the public
  response shape differs from the schema.
- Use migrations for future schema changes.
- Avoid exposing the legacy schema directly as the external API contract.
- Convert database errors into consistent application/API errors.

SeaORM remains a possible future option if generated entities, relationship
handling, or model-driven CRUD become more valuable than explicit SQL.

## Cache Layer

Redis is the initial cache layer. The cache layer is optional; the application
must preserve behavior with or without Redis.

Current guidelines:

- Keep cache usage explicit and local to the query or service that benefits
  from it.
- Use structured serialization such as JSON for cached DTOs.
- Prefix keys with a configurable namespace to avoid collisions.
- Use a default TTL and avoid indefinite cached application data unless there is
  a clear reason.
- Treat Redis as an optimization, not the source of truth.
- If cache is enabled and Redis is unavailable at startup, fail fast so
  deployment problems are visible.
- If cache is disabled, skip Redis startup checks and serve all requests from
  PostgreSQL.

Initial configuration:

- `REDIS_URL`: Redis connection URL, default `redis://127.0.0.1:6379`.
- `CACHE_ENABLED`: enables Redis cache usage, default `true`.
- `REDIS_KEY_PREFIX`: cache key prefix, default `dogn3`.
- `REDIS_DEFAULT_TTL_SECONDS`: default cache TTL, default `300`.

Media configuration:

- `IMAGE_DIRECTORY`: filesystem directory containing local post image
  attachments. The development checkout configures
  `/home/wy/pic/dogn_pic`.
- `IMAGE_UPLOAD_MAX_BYTES`: maximum uploaded post-image size, default
  `2097152` (2 MB), with a route safety ceiling of 10 MB.
- The backend exposes approved raster image files (`jpg`, `jpeg`, `png`, and
  `gif`) from this directory beneath `/images`; other files are not served.
- Local `post.image_url` values are treated as paths relative to this
  directory; remote `http`/`https` values remain external resources.
- Local files referenced only by encrypted posts require an authenticated
  session even when requested directly beneath `/images`.
- The post editor uploads new image attachments beneath
  `IMAGE_DIRECTORY/uploads`; unreferenced files in that managed upload
  namespace are not served.
- Uploaded files larger than 500 KB are normalized to JPEG and reduced in
  quality and, when necessary, dimensions until the stored payload is less
  than 500 KB. Smaller accepted files retain their original format.
- The current publication workflow allows an initial local image attachment;
  existing local attachments are immutable through the post editor and upload
  endpoint.

Post-write configuration:

- `POST_REPLY_MAX_AGE_DAYS`: maximum age of a discussion tree that may receive
  a reply, default `10`. The age is based on the root post creation time and
  is enforced again when the write is submitted.
- `POST_REPLY_MAX_POINTS`: maximum points transferable with one reply,
  default `100`.
- `NEW_USER_INITIAL_POINTS`: point balance assigned to an administrator-created
  account, default `100`.
- `POST_SUBJECT_MAX_LENGTH`: maximum post-subject length in characters,
  default `50`.
- `POST_CONTENT_MAX_BYTES`: maximum UTF-8 post-body size in bytes, default
  `131072` (128 KB).

Authentication configuration:

- `SESSION_TTL_SECONDS`: in-memory login session lifetime, default `604800`
  (7 days).
- `SESSION_COOKIE_SECURE`: set `true` for HTTPS deployments so browser
  session cookies are not sent over plaintext HTTP; local development defaults
  to `false`.
- `LOGIN_MAX_CONCURRENT_HASHES`: maximum concurrent password-verification
  operations, default `2`, bounding Argon2id resource usage during login.
- Login sessions are currently opaque server-managed tokens held in process
  memory. They expire by TTL and are cleared on server restart; persistent
  session storage is deferred until its database design is approved.
- A successful login maintains `user_info.last_login`, `last_login_ip`, and
  `login_count`; the recorded IP is the TCP peer address seen by the server,
  not an untrusted forwarding header.
- A failed login for a resolved account maintains `log_error_time` and
  `log_error_count` while keeping the externally visible failure response
  generic; unknown submitted names cannot update an account row.

Initial endpoint caching:

- `/api/home` uses read-through caching with a generation key
  (`api:home:v4:generation`) and separate metadata-visibility keys within
  each generation (`api:home:v4:public:{generation}` and
  `api:home:v4:authenticated:{generation}`).
- Cache hits return the cached JSON DTO.
- Cache misses read PostgreSQL and then write the response to Redis.
- Runtime cache read/write errors are logged and fall back to PostgreSQL.
- User-statistics recalculation advances the home cache generation after its
  database update. Older-generation writes cannot be served afterward.

Current cache invalidation status:

- User-statistics recalculation advances the home cache generation because the
  portal includes cached user post counts.
- Post creation, update, reply creation, and soft deletion advance the home
  cache generation after their successful database transaction.
- Soft deletion hides a selected reply individually, but hides an entire
  discussion when its root is deleted; populated-root deletion is reserved
  for a board master of that board or an administrator.
- If generation advancement fails, the application process disables home
  cache use to avoid serving potentially stale statistics.

Planned invalidation direction:

- After a successful database write transaction affecting portal data, advance
  the home cache generation.
- Post create, update, or delete advances the home cache generation.
- User create or update should advance the home cache generation.
- Board or category updates should advance the home cache generation.
- Site-manager board/category metadata updates and board-statistics
  recalculation advance the home cache generation after successful writes.
- Invalidation should happen only after the database transaction succeeds.
- Failed generation advancement is logged and disables cache reads in the
  application process; it does not roll back an already-successful database
  write.
- PostgreSQL notifications or another cross-process invalidation mechanism
  should be considered if multiple application processes are introduced.

## API Architecture

The backend should expose REST-style JSON endpoints as the primary application
contract. The same API should support the browser UI and future clients such as
mobile apps, scripts, or other integrations.

Initial API guidelines:

- Design endpoints around user workflows, not only CRUD table access.
- Keep request and response shapes stable and client-oriented.
- Use explicit DTO structs for request and response JSON.
- Keep backend validation authoritative even when the frontend also validates.
- Use consistent error response shapes.
- Version the API later if incompatible client-facing changes become likely.
- Avoid embedding dynamic page data in server-rendered HTML.

The API should not casually mirror every database table. The public contract
should describe application concepts and workflows, even when the implementation
uses legacy table names internally.

## Frontend Architecture

The frontend should start with standard browser technologies:

- HTML5 for document structure.
- CSS3 for styling and layout.
- Native Web Components for reusable UI pieces.
- No third-party JavaScript framework or UI library by default.
- `fetch`/Ajax calls using JSON for all dynamic content.

Dynamic content must be fetched from backend endpoints. The server may serve
static shell pages and assets, but the primary data path should be JSON API
responses.

The frontend can follow a lightweight MVC-style organization:

- Model: client-side state and API data shapes.
- View: Web Components and DOM rendering.
- Controller: event handling, navigation decisions, and API orchestration.

This should stay pragmatic. MVC is a code organization guide, not a reason to
build a large custom frontend framework.

## UI Design Principles

The UI should balance usability and creativity. The project should preserve its
unique character while making the interface clear, consistent, accessible, and
fast.

Frontend development checklist:

- Preserve unique value and creativity while optimizing usability.
- Maintain visual consistency across design elements, typography, and color
  schemes.
- Use a single typeface to create a unified look and feel unless a specific
  design need justifies an exception.
- Organize content clearly with headings, subheadings, and whitespace so users
  can scan and read comfortably.
- Design primarily for desktop devices while ensuring a robust, fully functional
  mobile experience.
- Use responsive CSS, including CSS Grid where appropriate.
- Follow WCAG-oriented accessibility practices: alternative text for images,
  keyboard navigation, semantic markup, visible focus states, and high-contrast
  colors.
- Keep pages fast by compressing images, enabling caching, and prioritizing
  critical content.
- Use concise menus, search bars, and breadcrumb navigation where they help
  orientation.
- Avoid overcrowded navigation and menus.
- Use simple, clear, line-drawing SVG icons for common actions and navigation.
  The visual reference should be restrained and functional, similar in spirit to
  the icon treatment on openai.com.
- Keep icons consistent in stroke width, visual weight, sizing, and alignment.
- Prefer SVG icons that clarify meaning without becoming decorative noise.
- Minimize pop-ups and other distractions.
- For content consumption, prefer scrolling over unnecessary clicking when it
  improves reading flow.
- Avoid scroll hijacking and excessive infinite scrolling.

These principles should be used as a frontend development checklist so technical
choices stay aligned with the intended user experience.

## Frontend Implementation Conventions

These conventions should be refined during implementation:

- Keep Web Components small and focused on one UI responsibility.
- Use semantic HTML inside components whenever possible.
- Keep component names consistent, preferably with a project prefix.
- Centralize API calls in a small client module instead of scattering raw
  `fetch` calls everywhere.
- Standardize loading, empty, and error states early.
- Keep CSS organized around layout, components, and utilities.
- Use progressive enhancement where practical.
- Avoid global mutable browser state unless it is explicitly owned and
  documented.

## Cross-Cutting Concerns

The project should define these concerns early, even if the first version keeps
the implementation minimal:

- Configuration: environment-based settings for database and server runtime.
- Logging and tracing: structured request logs and useful error context.
- Error handling: consistent API error responses and internal error boundaries.
- Security: input validation, output encoding, password/session handling, and
  conservative CORS settings.
- Performance: database indexes, query plans, response caching where useful, and
  efficient static asset delivery.
- Testing: unit tests for pure logic, integration tests for database-backed
  behavior, and lightweight API tests for important workflows.

## Non-Goals For Now

- Do not start with a full Rails-style framework.
- Do not introduce a heavy ORM layer before the domain model and query patterns
  are better understood.
- Do not over-design module boundaries before the first real web endpoints are
  implemented.
- Do not introduce a frontend framework or JavaScript dependency before the UI
  complexity justifies it.
- Do not let web pages depend on server-rendered dynamic HTML as the primary
  data delivery mechanism.
- Do not expose the legacy database schema as the public API simply because it
  is convenient.

## Possible Future Changes

The stack can change later if the project needs more structure.

Possible future options:

- Add SeaORM if model-driven CRUD, relationship handling, or generated entities
  become more valuable than explicit SQL.
- Move to a batteries-included framework such as Loco.rs if the project needs
  stronger conventions, scaffolding, background jobs, and integrated structure.
- Add a frontend framework if native Web Components become too limiting for the
  UI complexity.
- Add a template engine only for static shell pages, error pages, or other cases
  where server-rendered dynamic data is not the primary application path.
- Add OpenAPI documentation if external clients or stronger API contracts become
  important.

## Open Questions

- What are the first user-facing workflows?
- What are the first API resources and workflow-oriented endpoints?
- How should frontend files be organized?
- What browser versions need to be supported?
- What client-side routing, if any, is needed?
- How much of the migrated legacy schema should be preserved directly?
- What authentication and authorization model is needed?
- What deployment environment should the app target?
- What logging, metrics, and error reporting should be used?
- What test strategy is appropriate for database-backed handlers?
- What data migration or cleanup steps are needed after the initial MySQL to
  PostgreSQL migration?
