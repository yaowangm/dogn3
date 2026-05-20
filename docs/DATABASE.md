# Database Notes

This document describes the current PostgreSQL schema for the forum application.
It is a working draft based on the migrated database, table names, column names,
and current product understanding.

No foreign key constraints are currently documented here as guaranteed database
constraints. Relationships below are inferred from column names and application
meaning.

## Table Summary

| Table | Rows | Purpose |
| --- | ---: | --- |
| `category` | 3 | Category/grouping for boards. |
| `board` | 15 | Forum board. Belongs to a category. |
| `post` | 568046 | Forum content. Posts are organized as single-root trees within boards. |
| `user_info` | 632 | User account/profile information. |
| `info_bak` | 627 | Backup/archive of user information. Not expected to be part of the active domain model. |
| `favorite` | 2916 | User favorites for posts. Join-style table between users and posts. |
| `point_log` | 21598 | History of points users received from specific posts. Event/history table. |
| `sign_log` | 1737 | History of users setting signatures. |

## Inferred Relationships

These relationships are inferred by naming convention. For example,
`post.board_id` is treated as a reference to `board.id`.

| Relationship | Column | Meaning |
| --- | --- | --- |
| `category` 1:N `board` | `board.category_id -> category.id` | A category contains many boards. |
| `board` 1:N `post` | `post.board_id -> board.id` | A board contains many posts. |
| `post` 1:N `post` | `post.parent_id -> post.id` | A post may have child posts. `NULL` means the post is a root. |
| `post` 1:N `post` | `post.root_id -> post.id` | Posts with the same `root_id` belong to one single-root tree. |
| `user_info` 1:N `post` | `post.user_id -> user_info.id` | A user can author many posts. |
| `user_info` N:N `post` | `favorite.user_id`, `favorite.post_id` | A user can favorite many posts, and a post can be favorited by many users. |
| `user_info` 1:N `point_log` | `point_log.user_id -> user_info.id` | A user can have many point log entries. |
| `post` 1:N `point_log` | `point_log.post_id -> post.id` | A post can be associated with many point log entries. |
| `user_info` 1:N `sign_log` | `sign_log.user_id -> user_info.id` | A user can have many signature change log entries. |
| `post` 1:N `sign_log` | `sign_log.sign_id -> post.id` | A post can be used as a signature by many signature log entries. |

## Table Details

### `category`

Board category.

Important columns:

- `id`: primary identifier.
- `name`: category name.
- `comment`: short description.
- `order_id`: display/order value.
- `board_count`: denormalized count of boards.

Indexes:

- `category_pkey` on `id`.

### `board`

Forum board. Belongs to a `category`.

Important columns:

- `id`: primary identifier.
- `name`: board name.
- `comment`: short description.
- `category_id`: inferred reference to `category.id`.
- `post_count`: denormalized post count.
- `root_count`: likely denormalized count of root posts/threads.
- `master_name`, `master_name_2`, `master_name_3`, `master_name_4`: moderator names or board manager names.
- `master_id`: moderator/manager user id, inferred but not yet verified.
- `order_id`: display/order value.

Indexes:

- `board_pkey` on `id`.
- `idx_board_category_id` on `category_id`.

### `post`

Forum post data. All posts are organized by trees.

Tree model:

- Posts with the same `root_id` belong to one specific single-root tree.
- `parent_id` points to the parent post inside the tree.
- A post without `parent_id` is a root post.
- `level` stores the depth of the current post.
- Root posts have `level = 0`.
- Direct children of a root post have `level = 1`, and so on.

Display order:

- Posts are displayed in a depth-first traversal order.
- The root post appears first.
- Then the first direct child of the root appears, followed by that child's
  descendants.
- Then the second direct child of the root appears, followed by that child's
  descendants, and so on.
- Among sibling posts, newer posts appear first. In practice, this means the
  sibling with the larger `id` appears earlier.
- For performance, many trees can be displayed in the correct order with one
  query:

```sql
SELECT ...
FROM post
ORDER BY root_id DESC, order_num;
```

`root_id DESC` places newer trees first. `order_num` stores the display order
inside each tree.

`order_num` maintenance:

- The root post always has `order_num = 0`.
- The first direct child of the root has `order_num = 1`.
- When a new child is added to a parent post, the new child is inserted
  immediately after the parent in display order.
- Before inserting the new child, every post in the same tree with
  `order_num > parent.order_num` must have `order_num` incremented by `1`.
- The new child then receives `order_num = parent.order_num + 1`.

Example:

- A tree has 10 posts with `order_num` values `0` through `9`.
- A new post replies to the post whose `order_num` is `5`.
- All posts in the same tree with `order_num > 5` are incremented by `1`.
- The new post gets `order_num = 6`.
- The new post becomes the first direct child of the parent in display order.

`order_num` is therefore a denormalized tree traversal key and must be
maintained carefully whenever the tree changes.

`order_num_2` is currently not used.

Rendering rule:

- Tree display should indent each post according to `level`.
- This makes the depth of replies visible while preserving the single-query
  display order.

Important columns:

- `id`: primary identifier.
- `board_id`: inferred reference to `board.id`.
- `parent_id`: inferred reference to parent `post.id`; `NULL` for root posts.
- `root_id`: inferred reference to root `post.id`; groups all posts in the same tree.
- `level`: depth of the current post in its tree; used for visual indentation.
- `order_num`: denormalized depth-first display order inside a tree.
- `order_num_2`: currently unused.
- `subject`: post title/subject.
- `content`: post body.
- `size`: content size or display size.
- `access_count`: view/access count.
- `user_id`: inferred reference to `user_info.id`.
- `user_name`: denormalized author name.
- `post_time`: creation time.
- `reply_count`: denormalized reply count.
- `reply_time`: latest reply time or reply-related timestamp.
- `type`: post status/type used to choose the display icon.
- `link_name`, `link_url`, `image_url`: link/image metadata.
- `state`: post visibility state.
- `folder_id`: legacy grouping/folder field; exact meaning unknown.
- `point`: point value associated with the post.
- `sign_id`: signature id captured with the post.

Indexes:

- `post_pkey` on `id`.
- `idx_post_board_id` on `board_id`.
- `idx_post_parent_id` on `parent_id`.
- `idx_post_access_count` on `access_count`.
- `idx_post_point` on `point`.
- `idx_post_type` on `type`.
- `idx_post_user_id` on `user_id`.
- `idx_post_tree_order` on `root_id, order_num_2`.
- `idx_post_post_time_access_count` on `post_time, access_count`.

Known `post.type` values from the legacy PHP code:

| Constant | Value | Meaning |
| --- | ---: | --- |
| `ART_TYPE_NORMAL` | 0 | Normal post. |
| `ART_TYPE_ORIGINAL` | 1 | Original post. |
| `ART_TYPE_FORWARD` | 2 | Forwarded/reposted post. |
| `ART_TYPE_ANNOUNCE` | 3 | Announcement post. |

When a post is displayed, the UI should choose the corresponding icon according
to `post.type`.

Known `post.state` values from the legacy PHP code:

| Constant | Value | Meaning |
| --- | ---: | --- |
| `ART_STATE_NORMAL` | 0 | Normal post; can be read by anybody. |
| `ART_STATE_ENCRYPTED` | 1 | Encrypted/restricted post; can be read only by logged-in users. |
| `ART_STATE_DELETED` | 2 | Deleted post; should not be readable by anyone. |

### `user_info`

User account/profile information.

Important columns:

- `id`: primary identifier.
- `name`: user name.
- `password`: password hash or legacy password value.
- `state`: account state/status flag.
- `level`: user level.
- `email`: email address.
- `intro`: profile introduction.
- `reg_time`: registration time.
- `post_count`: denormalized post count.
- `doc_count`: document count or legacy content count.
- `last_login`, `last_login_ip`: latest login information.
- `last_origin`, `last_reship`, `last_post`: legacy activity timestamps.
- `login_count`: login counter.
- `point`: user point balance.
- `intro_user_id`: inferred inviter/referrer user id.
- `sign_id`: current signature id.
- `favorite_count`: denormalized favorite count.
- `log_error_time`, `log_error_count`: login error tracking fields.

Indexes:

- `user_info_pkey` on `id`.
- `idx_user_info_name` on `name`.
- `idx_user_info_point` on `point`.
- `idx_user_info_reg_time` on `reg_time`.

Known `user_info.level` values from the legacy PHP code:

| Constant | Value | Meaning |
| --- | ---: | --- |
| `User_Guest` | 0 | Guest user. |
| `User_Normal` | 1 | Normal user. |
| `User_Adv` | 5 | Advanced user. |
| `User_Admin` | 10 | Administrator. |

Known `user_info.state` values:

| Value | Meaning |
| ---: | --- |
| 0 | Normal user. |
| 1 | Frozen user. |

### `info_bak`

Backup/archive of user information.

This table closely resembles `user_info`, but should be treated as legacy backup
data unless a real application workflow needs it. It is not expected to be part
of the active domain model.

Important columns:

- `id`, `name`, `password`, `state`, `user_level`, `email`, `intro`.
- `reg_time`, `post_count`, `doc_count`.
- `last_login`, `last_login_ip`, `last_origin`, `last_reship`, `last_post`.
- `login_count`, `points`, `intro_user_id`, `signature_id`.
- `favorite_count`, `log_error_time`, `log_error_count`.

Indexes:

- No indexes are currently present.

### `favorite`

User favorites for posts.

Important columns:

- `id`: primary identifier.
- `user_id`: inferred reference to `user_info.id`.
- `post_id`: inferred reference to `post.id`.
- `create_time`: time the favorite was created.

Indexes:

- `favorite_pkey` on `id`.
- `idx_favorite_user_id` on `user_id`.
- `idx_favorite_post_id` on `post_id`.
- `idx_favorite_create_time` on `create_time`.

Potential future constraint:

- Consider a uniqueness rule on `(user_id, post_id)` if duplicate favorites are
  invalid. Existing data should be checked before adding the constraint.

### `point_log`

Point history table. This is an event/history table, not just a join table,
because it records a point value and timestamp.

Important columns:

- `id`: primary identifier.
- `post_id`: inferred reference to `post.id`.
- `user_id`: inferred reference to `user_info.id`.
- `point`: point value for the event.
- `post_time`: event time.

Indexes:

- `point_log_pkey` on `id`.
- `idx_point_log_post_time` on `post_time`.

Potential future indexes:

- Consider indexes on `user_id` and `post_id` if point history is queried by
  user or post.

### `sign_log`

Signature history table.

Each `sign_log` row records a user choosing a signature. `sign_id` is a
reference to the `post.id` of the post used as the signature. The latest
`sign_log` record for a specific user represents the signature currently used by
that user.

Important columns:

- `id`: primary identifier.
- `user_id`: inferred reference to `user_info.id`.
- `sign_id`: inferred reference to `post.id`; the post used as the signature.
- `set_time`: time the signature was set.

Indexes:

- `sign_log_pkey` on `id`.
- `idx_sign_log_user_id` on `user_id`.
- `idx_sign_log_set_time` on `set_time`.

## Naming Convention

The current PostgreSQL schema uses:

- lower snake_case table names;
- lower snake_case column names;
- primary key column name `id`;
- foreign-key-style names ending in `_id`;
- index names in the form `idx_<table>_<column_or_purpose>`.

## Open Questions

- Should inferred relationships become real PostgreSQL foreign keys?
- Do any legacy rows contain dangling references that would block foreign keys?
- Should `favorite` enforce uniqueness on `(user_id, post_id)`?
- Should `info_bak` remain in the active application database or move to an
  archive-only path?
- The previously migrated `upd_log` table is not currently present in the public
  schema. Determine whether it was intentionally removed or should be restored.
