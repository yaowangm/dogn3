# Search Design

This document records the current post-search strategy and the planned path for
future semantic/vector search.

## Current Scope

Search is implemented as authenticated lexical post search.

Routes:

```text
/search
GET /api/search/posts
```

Only logged-in users can perform search. Anonymous users may load the shared
HTML shell at `/search`, but the JSON API returns `401 authentication_required`.
The browser then shows a login prompt that points back to `/search`.

Search results include all visible posts:

```sql
post.state IN (0, 1)
```

Deleted posts (`state = 2`) are excluded. Because login is required, encrypted
posts (`state = 1`) can be returned with normal metadata, related-resource flags,
links, image paths, and result display.

## Why Lexical Search First

The original long-term idea was vector search with `pgvector`, but the first
implementation intentionally uses PostgreSQL lexical search.

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

The API combines two text matching techniques for each keyword field:

- PostgreSQL full-text search with the `simple` configuration.
- Case-insensitive substring matching through `POSITION(LOWER(keyword) IN
  LOWER(column)) > 0`.

This hybrid approach is deliberate.

PostgreSQL full-text search can be efficient for tokenized text and normal
English-like terms, especially when the matching GIN indexes exist. However, the
built-in parser is not ideal for CJK text because Chinese does not require
spaces between words. Substring matching keeps Chinese keywords predictable
without requiring a Chinese tokenizer extension.

The production index script also creates `pg_trgm` indexes. These indexes help
substring-like matching on larger data sets when PostgreSQL can use them.

## Long Token Notice

When creating the content full-text index, PostgreSQL may emit notices like:

```text
NOTICE: word is too long to be indexed
DETAIL: Words longer than 2047 characters are ignored.
```

This is not an error. It means a single token longer than PostgreSQL full-text
search's token limit was ignored by the FTS index. It does not mean that a post
with content longer than 2047 characters is skipped. Normal long posts with
token breaks can still be indexed. Very long unbroken strings, long URLs,
base64-like data, minified text, or some unsegmented CJK text may not benefit
from the FTS index for that token.

Correctness is preserved by the substring matching path. The impact is mainly
index usefulness and performance for those special tokens.

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
the next date boundary.

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

- Search result post links open in a new window.
- Reply indentation is not applied in search results.
- Board name appears as a right-side pill in each result card.
- Content excerpts are intentionally not displayed.
- Post status icons still show image/link/encrypted flags when the post data has
  them.

## Index Script

Production databases should run:

```bash
psql dogn -v ON_ERROR_STOP=1 -f scripts/add_post_search_indexes.sql
```

The script is safe to rerun. It drops and recreates only search-related indexes;
it does not change post data.

Indexes include:

- visible-post id index
- post creation time
- reply time
- post type
- full-text GIN indexes for subject/content/user name
- trigram GIN indexes for subject/content/user name
- attachment flag helper indexes for image and related link

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
- post-type filtering
- image/link filters
- id ascending/descending behavior
- deleted-post exclusion
- Chinese substring matching

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
