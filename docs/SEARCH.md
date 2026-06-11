# Search Design

This document records the current post-search strategy, PostgreSQL/PGroonga
setup, runtime behavior, and the planned path for future semantic/vector search.

## Current Scope

Search is implemented as authenticated PGroonga-backed post search.

Routes:

```text
/search
GET /api/search/posts
```

Only logged-in users can perform search. Anonymous users may load the shared
HTML shell at `/search`, but the JSON API returns `401 authentication_required`.
The browser then shows a login prompt that points back to `/search`.

When no search condition is supplied, the API returns the search form data and
board navigation without running the post count or result queries. The page
shows an instruction to enter at least one condition and omits search timing,
pagination, and result items. Ordering and paging parameters alone do not count
as search conditions.

Search results include visible posts that can be opened by the post page:

```sql
post.state IN (0, 1)
```

The search query also requires `post.board_id` to match an existing board,
because the post page itself joins `post` to `board` and orphaned legacy posts
cannot be opened. Deleted posts (`state = 2`) are excluded. Because login is
required, encrypted posts (`state = 1`) can be returned with normal metadata,
related-resource flags, links, image paths, and result display.

## Why PostgreSQL-Local Search First

The original long-term idea was vector search with `pgvector`, but the first
implementation intentionally keeps search inside PostgreSQL.

Reasons:

- It is simpler to deploy and operate.
- It does not require an embedding model, API key, batch job, or vector refresh
  policy.
- It works naturally with structured forum filters such as user name, date
  ranges, post type, and attachment flags.
- It provides deterministic results ordered by post id, which matches current
  user expectations.

Vector search remains future work.

## Text Matching Strategy

The API uses PGroonga full-text search for each keyword field. For `subject`,
`content`, and `user_name`, each non-empty keyword condition uses this shape:

```sql
COALESCE(field, '')::text &@ keyword
```

The backend builds the `WHERE` clause from active conditions only. Empty
conditions are omitted rather than represented as `empty_parameter OR
predicate`. Search values remain SQLx bind parameters. This produces direct
PGroonga predicates that are easier for PostgreSQL to match to the partial
PGroonga indexes.

The `&@` operator is PGroonga's full-text search-by-keyword operator. This
replaces the previous `ILIKE OR to_tsvector('simple')` hybrid search, which was
not a real Chinese full-text search strategy and often let PostgreSQL choose a
sequential scan for subject/content searches.

The API returns this as `search_method`, including a measured
`search_time_ms`. The search page displays only the SQL method and measured
server search time after a search finishes. Normal non-empty result pages use
`COUNT(*) OVER()` to obtain the exact total and page rows in one query. An
additional count query is used only when the requested page returns no rows,
which preserves exact empty-result and out-of-range page handling. Timing
excludes browser rendering.

PGroonga was chosen because PostgreSQL built-in full-text search does not
segment Chinese text properly. PGroonga provides PostgreSQL-local Chinese and
multilingual full-text search without introducing a separate search service.

## PGroonga Installation

PGroonga is not part of the standard PostgreSQL 16 distribution. It must be
installed on the PostgreSQL host before the database script can create the
extension.

For Ubuntu 24.04 / PostgreSQL 16:

```bash
sudo apt install -y -V ca-certificates lsb-release wget

wget https://packages.groonga.org/ubuntu/groonga-apt-source-latest-$(lsb_release --codename --short).deb

sudo apt install -y -V ./groonga-apt-source-latest-$(lsb_release --codename --short).deb

rm -f groonga-apt-source-latest-$(lsb_release --codename --short).deb

sudo apt update

sudo apt install -y -V postgresql-16-pgroonga
```

After the package is installed, run the project database script:

```bash
psql dogn -v ON_ERROR_STOP=1 -f scripts/add_post_pgroonga_search_indexes.sql
```

The script creates `CREATE EXTENSION IF NOT EXISTS pgroonga;` in the target
database and builds the search indexes. It does not change post data.

## Query Parameters

Current UI parameters:

| Parameter | Meaning |
| --- | --- |
| `subject` | Keyword searched only in `post.subject`. |
| `content` | Keyword searched only in `post.content`. |
| `user_name` | Keyword searched only in `post.user_name`. |
| `created_from`, `created_to` | Inclusive creation-date range from date inputs. |
| `replied_from`, `replied_to` | Inclusive reply-date range from date inputs. |
| `post_type` | Optional post type: `0` normal, `1` original, `2` forward, `3` announcement. |
| `has_image` | When `true`, require a non-empty `post.image_url`. |
| `order` | `id_desc` by default, or `id_asc`. |
| `page`, `page_size` | Paged result control; page size is clamped by the API. |

The backend also accepts `has_link=true`, which filters to posts with a non-empty
`post.link_url`. The current search page does not expose this condition because
the UI was simplified, but the API and index remain available for future use.

Date `*_to` values are treated as whole-day inclusive values by comparing with
the next date boundary. Date filters must use `YYYY-MM-DD`; invalid dates return
`400 invalid_search_filter` before PostgreSQL sees the query.

## Ordering And Pagination

Default ordering:

```text
id_desc
```

Optional ordering:

```text
id_asc
```

The API uses a controlled enum for `ORDER BY`, not user-provided SQL. Page size
is clamped between server-side minimum and maximum values.

## Result Display

The search page uses the existing post-card style with these adjustments:

- The result summary shows the SQL search method and measured server search
  time.
- Search result post links open in a new window.
- Reply indentation is not applied in search results.
- Board name appears as a right-side pill in each result card.
- Content excerpts are intentionally not displayed.
- Post status icons still show image/link/encrypted flags when the post data has
  them.

## Index Script

Production databases should run:

```bash
psql dogn -v ON_ERROR_STOP=1 -f scripts/add_post_pgroonga_search_indexes.sql
```

The script is safe to rerun. It creates the `pgroonga` extension and drops and
recreates only PGroonga search indexes; it does not change post data.

Indexes include:

- PGroonga index for `post.subject`
- PGroonga index for `post.content`
- PGroonga index for `post.user_name`

The application text predicates are written to match these PGroonga indexes.
Read-only `EXPLAIN` checks against the migrated database confirmed index scans
for representative subject, content, and user-name searches after the PGroonga
indexes were created.

Future production verification should repeat those checks with the current
active-filter query builder and representative combined text/date/type
conditions. See `docs/PERFORMANCE.md`.

The older `scripts/add_post_search_indexes.sql` belongs to the previous
PostgreSQL `simple` full-text/trigram hybrid implementation and is not the
current search setup.

## Security

Search follows the project security rules:

- Authentication is required.
- Deleted posts are excluded.
- User-supplied query values are bound through SQLx parameters.
- `ORDER BY` uses a controlled enum.
- Frontend rendering escapes dynamic values.
- Post links and board links use existing safe rendering patterns.

## Tests

Search API tests cover:

- authentication requirement
- subject filtering
- content filtering
- user-name filtering
- date-range filtering
- malformed date rejection
- post-type filtering
- image/link filters
- id ascending/descending behavior
- deleted-post exclusion
- Chinese PGroonga keyword matching

The route test also verifies `/search` returns the shared HTML shell.

## Future Vector Search

When semantic search is needed, use `pgvector` as a PostgreSQL extension rather
than adding a separate vector database service.

Recommended future structure:

```text
post_search_index
```

Possible fields:

```text
post_id
search_text
embedding
updated_at
```

Future vector work will need decisions about:

- embedding model or provider
- embedding dimension
- rebuild/update job
- handling post edits/deletions
- hybrid lexical + semantic ranking
- whether vector relevance or explicit id order wins when both are requested

Until those decisions are made, lexical search remains the source of truth.
