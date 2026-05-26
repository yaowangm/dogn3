# Database Notes

This document describes the current PostgreSQL schema for the forum application.
It is a working draft based on the migrated database, table names, column names,
and current product understanding.

Most relationships below began as inferences from the migrated schema. The
new `board_master` relation is created with explicit foreign key constraints.

## Table Summary

| Table | Rows | Purpose |
| --- | ---: | --- |
| `category` | 3 | Category/grouping for boards. |
| `board` | 15 | Forum board. Belongs to a category. |
| `board_master` | 27 | Ordered board-to-manager user relationships. |
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
| `board` N:N `user_info` | `board_master.board_id`, `board_master.user_id` | A board has manager users; a user may manage multiple boards. |
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
- `board_count`: denormalized count of boards; board creation, movement, and
  deletion refresh it transactionally, and site-manager statistics
  recalculation repairs it from live `board` relationships.

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
- `master_id`: retained legacy column whose meaning is not yet verified; it is
  not used as the board-manager relationship.
- `order_id`: display/order value.

Indexes:

- `board_pkey` on `id`.
- `idx_board_category_id` on `category_id`.

### `board_master`

Ordered relationship between boards and users who manage them.

Important columns:

- `board_id`: explicit foreign key to `board.id`, deleted with its board.
- `user_id`: explicit foreign key to `user_info.id`; a referenced manager
  cannot be deleted while the relationship remains.
- `order_id`: controls manager display order within one board.

Constraints and indexes:

- Composite primary key on (`board_id`, `user_id`) prevents duplicate manager
  assignments.
- `idx_board_master_user_id` supports locating boards managed by a user.

### Board Master Migration

#### Purpose

Legacy databases store board managers as up to four text values on `board`:

```text
master_name, master_name_2, master_name_3, master_name_4
```

This is unsuitable for current application behavior because names are mutable,
free-text values do not guarantee a user exists, and the fixed slots prevent a
proper many-to-many relationship. The current model stores an ordered
relationship in `board_master` and uses `user_info.id` as the stable manager
identity.

#### Resulting Schema Change

The migration:

1. Creates `board_master (board_id, user_id, order_id)`.
2. Adds foreign keys to `board.id` and `user_info.id`.
3. Adds a composite primary key on (`board_id`, `user_id`).
4. Adds `idx_board_master_user_id`.
5. Converts each populated legacy manager-name slot to a user relationship.
6. Preserves slot order as `order_id` values `1` through `4`.
7. Drops `board.master_name`, `master_name_2`, `master_name_3`, and
   `master_name_4`.

`board.master_id` is deliberately retained. Its legacy purpose has not been
verified, and it is not read or written as the board-manager relationship.

#### Preconditions

Before running the migration on another real database:

1. Back up the database.
2. Ensure the database already has the snake_case application table/column
   naming if using `scripts/migrate_board_masters.sql`.
3. Deploy or prepare application code that reads `board_master`; code using
   `board.master_name*` will fail after the transaction commits.
4. Check that every non-empty legacy manager name uniquely resolves to one
   `user_info.name`, after trimming whitespace.
5. Check that one board does not list the same manager name in more than one
   legacy slot.

The migration script performs checks 4 and 5 inside the transaction and aborts
without dropping legacy columns if either condition is violated.

#### Scripts And Starting Points

For a database already upgraded to current snake_case naming but still using
the legacy manager columns:

```bash
psql dogn -v ON_ERROR_STOP=1 -f scripts/migrate_board_masters.sql
```

For a newly imported PostgreSQL database produced directly by the MySQL
migration script, run the full schema upgrade instead. It includes this
board-master conversion:

```bash
psql dogn -v ON_ERROR_STOP=1 -f scripts/upgrade_initial_postgres_schema.sql
```

Run only the script appropriate for the database starting state. These are
one-time upgrade scripts and are not intended to be rerun after the legacy
columns have been removed.

#### Conversion Rule

Each non-empty legacy field is converted as follows:

| Legacy value | New relationship |
| --- | --- |
| `board.master_name` | `board_master.order_id = 1` |
| `board.master_name_2` | `board_master.order_id = 2` |
| `board.master_name_3` | `board_master.order_id = 3` |
| `board.master_name_4` | `board_master.order_id = 4` |

`board_master.board_id` is the original board ID. `board_master.user_id` is
the ID of the unique `user_info` row whose trimmed name matches the trimmed
legacy field.

#### Post-Migration Verification

After a migration, run read-only checks:

```sql
SELECT COUNT(*) FROM board_master;

SELECT column_name
FROM information_schema.columns
WHERE table_name = 'board'
  AND column_name LIKE 'master_name%';

SELECT COUNT(*)
FROM board_master bm
LEFT JOIN board b ON b.id = bm.board_id
LEFT JOIN user_info u ON u.id = bm.user_id
WHERE b.id IS NULL OR u.id IS NULL;
```

Expected results:

- The `board_master` count equals the number of populated, distinct legacy
  board-manager assignments identified before migration.
- The legacy-column query returns no rows.
- The orphaned-relationship count is `0`.

For the first migrated `dogn` database, `27` relationships were converted,
the four legacy name columns were removed, and orphaned relationships were
verified as `0`.

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
- `reply_count`: for a root post, denormalized total count of stored posts in
  that tree, including the root post itself. The legacy column name is
  retained although this is an inclusive tree count.
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
- `idx_post_normalized_image_url_state` on normalized `image_url`, `state`;
  supports local attachment authorization and can be added to an already
  upgraded database with `scripts/add_post_image_visibility_index.sql`.

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

Application reads treat these states as an allowlist. A post whose `state`
does not equal `0` or `1` is not returned until its meaning and access rule
are explicitly defined.

`reply_count` maintenance rule:

- Only the root post's `reply_count` is authoritative for current display.
- Its value is the number of stored posts with the same effective tree root
  (`COALESCE(root_id, id)`), including the root itself.
- This stored tree count includes posts in every visibility state. It may be
  greater than the posts currently exposed by a visibility-filtered page.
- After loading or replacing migrated post data, recalculate root values with:

```bash
psql dogn -v ON_ERROR_STOP=1 -f scripts/recalculate_root_post_reply_count.sql
```

The script updates only root rows whose value differs, leaves non-root
`reply_count` values unchanged, and reports the number of remaining
inconsistent root rows after the update.

Application post-write maintenance rule:

- Creating a new root post sets `parent_id = 0`, `root_id = id`, `level = 0`,
  `order_num = 0`, and `reply_count = 1`.
- Creating a reply inserts a normal-type child at
  `parent.order_num + 1`. In the same transaction, later posts in that tree
  have `order_num` incremented, and the root's `reply_count` and
  `reply_time` are refreshed. The root row is locked before tree-order
  changes so concurrent replies in one tree are serialized.
- A reply is accepted only while the discussion tree's root post is no older
  than `POST_REPLY_MAX_AGE_DAYS`, defaulting to 10 days. This is checked on
  both editor entry and reply insertion.
- Creating or editing a post recalculates the affected board's visible
  `post_count` and `root_count`, and the author's visible `post_count` and
  original-post `doc_count`, in the same transaction.
- Editing does not change authorship, tree placement, point history, or
  signature relationships. Existing legacy link metadata is preserved but is
  not editable in the current post editor.
- New or replacement image attachments are uploaded into
  `IMAGE_DIRECTORY/uploads` and the generated relative path is stored in
  `post.image_url`; the editor does not accept arbitrary image URLs. Upload
  size is constrained by `IMAGE_UPLOAD_MAX_BYTES`, defaulting to 2 MB.
  Uploaded images larger than 500 KB are stored as compressed JPEG files
  reduced below 500 KB; smaller accepted images retain their input format.

### `user_info`

User account/profile information.

Important columns:

- `id`: primary identifier.
- `name`: user name.
- `password`: password hash; active migrated accounts use Argon2id-wrapped
  legacy verification data and new/changed passwords use direct Argon2id.
- `password_scheme`: identifies how `password` must be verified
  (`argon2id-md5-v1` for transformed legacy credentials or `argon2id-v1` for
  new/changed passwords).
- `state`: legacy account flag; it does not control authentication eligibility.
- `level`: user level; `0` identifies a frozen account for authentication.
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
| `User_Guest` | 0 | Legacy name for an account treated as frozen; cannot authenticate. |
| `User_Normal` | 1 | Normal user. |
| `User_Adv` | 5 | Advanced user. |
| `User_Admin` | 10 | Administrator. |

Application role maintenance rule:

- New users are created as `User_Normal`.
- An administrator may explicitly set a user to frozen, normal, or
  administrator.
- When a normal user is assigned as a `board_master`, the application updates
  the user to `User_Adv`.
- When an advanced user is removed from their final `board_master`
  relationship, the application updates the user back to `User_Normal`.
- Deleting a board removes its board-master assignments through the foreign-key
  cascade and immediately reconciles affected derived roles.
- The site-manager all-board statistics recalculation repairs board-master
  role drift in both directions: normal users with assignments become
  `User_Adv`, while advanced users without assignments return to
  `User_Normal`.
- Automatic board-master transitions do not alter frozen or administrator
  users. Requesting the normal role for a user who remains a board master
  resolves to `User_Adv`.

Application account/profile maintenance rule:

- Administrator account creation inserts a new `User_Normal` record with a
  direct `argon2id-v1` credential, current registration time, optional
  `email`, `intro`, and `intro_user_id`, and zero-initialized statistics.
- Profile update is restricted to `email` and `intro`. Owners may update their
  own row; administrators may update any row. It does not implicitly modify
  role, password, introducer, or derived counts.
- `email` is confidential application data returned only to the owner or an
  administrator. `intro` is public profile content.
- Statistics recalculation derives `post_count`, `doc_count`, and
  `favorite_count` from visible post relationships rather than accepting
  arbitrary client values.

`user_info.state` was previously interpreted as a normal/frozen marker. That
interpretation is incorrect; its meaning is currently unspecified and it must
not be used to allow or deny login.

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
