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
- `idx_category_order` on `order_id, id`; supports navigation/category
  ordering.

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
- `idx_board_category_order` on `category_id, order_id, id`; supports
  category-grouped board lists and navigation ordering.

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
- `idx_board_master_board_order` supports ordered manager display by board.

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
- `content_format`: post body format marker. `0` means legacy plain text; `1`
  means Markdown rendered by the client after sanitization. Existing migrated
  posts default to `0`.
- `size`: content size or display size.
- `access_count`: view/access count. The current application increments it
  only when an authenticated session opens the normal post detail endpoint,
  once per session/post. Legacy values may include older counting behavior.
- `user_id`: inferred reference to `user_info.id`.
- `user_name`: denormalized author name.
- `post_time`: creation time.
- `last_update_time`: latest application-recorded edit time. `NULL` means the
  post has no recorded edit after the column was introduced.
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
  - The referenced signature post follows normal post visibility. A signature
    whose post has encrypted state is rendered only for authenticated viewers.

Indexes:

- `post_pkey` on `id`.
- `idx_post_board_id` on `board_id`.
- `idx_post_parent_id` on `parent_id`.
- `idx_post_user_id` on `user_id`.
- `idx_post_root_award_lookup` on `user_id, post_time, type` for root rows;
  supports the once-per-day root-post point-award lookup.
- `idx_post_normalized_image_url_state` on normalized `image_url`, `state`;
  supports local attachment authorization and can be added to an already
  upgraded database with `scripts/add_post_image_visibility_index.sql`.
- `idx_post_visible_board_tree_order` on `board_id`, effective root id, and
  `order_num`; supports board pages ordered by tree and in-tree display order.
- `idx_post_visible_tree_order` on effective root id and `order_num`; supports
  visible post-tree display.
- `idx_post_tree_order_all` on `root_id, order_num`; supports reply insertion
  order maintenance for all stored rows in a tree, including deleted rows.
- `idx_post_effective_root_all` on effective root id; supports tree counts and
  other stored-tree operations that intentionally include all visibility states.
- `idx_post_visible_tree_post_time` on effective root id, `post_time`, and
  `id`; supports post-list pages ordered by creation time.
- `idx_post_visible_user_type_id`, `idx_post_visible_user_post_time`, and
  `idx_post_visible_user_type_post_time`; support user activity lists and
  derived user-statistic refreshes.
- `idx_post_visible_board_roots`, `idx_post_visible_type_id`, and
  `idx_post_visible_roots_home`; support board counts, home-page post cards,
  and type-filtered post lists.
- PGroonga search indexes on normalized `subject`, `content`, and `user_name`
  support current Chinese/multilingual lexical post search. They are managed by
  `scripts/add_post_pgroonga_search_indexes.sql`.

The one-time performance scripts under `scripts/` are operational aids for
upgraded databases:

- `scripts/add_query_performance_indexes.sql` adds application-path indexes.
- `scripts/add_post_stored_tree_indexes.sql` adds follow-up indexes for
  all-stored-tree mutation/count paths whose queries do not filter by state.
- `scripts/drop_obsolete_performance_indexes.sql` removes obsolete pre-PGroonga
  tsvector/trigram search indexes and duplicate btree search-index copies.
- `scripts/drop_stale_legacy_indexes.sql` removes legacy indexes that current
  runtime code no longer uses, including the old `order_num_2` tree-order
  index.
- `scripts/review_index_usage.sql` is a read-only diagnostic script for
  checking live index usage before deciding future cleanup.
- `scripts/apply_performance_improvements.sql` is the cumulative remote
  deployment script for all database changes required by performance work. It
  configures `sign_log.id` generation, normalizes legacy zero root ids,
  assigns stable names to the two known frozen `?` placeholder accounts,
  enforces normalized user-name and favorite uniqueness, adds administrator
  user-search and root-award indexes, and removes superseded indexes.

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
- A reply may transfer `0..=POST_REPLY_MAX_POINTS` points, with a default
  maximum of `100`, from its author to the owner of the post being replied
  to only when the direct reply target is a root post owned by another user.
  Any amount greater than the sender's current balance is rejected.
- A positive reply transfer atomically decrements `user_info.point` for the
  sender, increments it for the replied-to root post owner, increments
  `post.point` on the replied-to root post, and adds a `point_log` event for
  that root post and sender. A zero transfer performs none of these point
  writes.
- Creating a root post may award points directly to the author once per
  PostgreSQL `CURRENT_DATE` and award category. Normal and announcement root
  posts share the regular-post category and award
  `ROOT_POST_REGULAR_AWARD_POINTS` points, default `2`; forward root posts
  award `ROOT_POST_FORWARD_AWARD_POINTS`, default `5`; original root posts
  award `ROOT_POST_ORIGINAL_AWARD_POINTS`, default `10`. Additional root posts
  in the same category on the same database date do not award more points.
  This updates only `user_info.point`; it does not create `point_log` rows and
  does not change `post.point`.
- Creating or editing a post recalculates the affected board's visible
  `post_count` and `root_count`, and the author's visible `post_count`,
  original-post `doc_count`, and legacy activity timestamps in the same
  transaction.
- New or edited post subjects are limited by `POST_SUBJECT_MAX_LENGTH`,
  defaulting to 50 characters. Body content is limited by
  `POST_CONTENT_MAX_BYTES`, defaulting to 131072 UTF-8 bytes (128 KB);
  `post.size` records the accepted content byte count.
- Root posts may be edited by their owner or an administrator. Non-root posts
  may be edited only by an administrator and retain `type = 0`. Any post that
  has appeared in `sign_log` is locked against non-admin edits, even when it is
  no longer a user's latest signature.
- The author may soft-delete their own root post only when the tree contains
  no child posts. A board master may soft-delete any post in their board, and
  an administrator may soft-delete any post. Soft-deleting a root sets every
  stored post with that effective root to `state = 2`; deleting a non-root
  post changes only that post. Rows remain stored and become unavailable
  through normal application reads. The affected board counts, affected author
  visible counts/activity timestamps, and favorite counts for users who had
  deleted posts favorited are refreshed in the same transaction.
- Editing does not change authorship, tree placement, point history, signature
  relationships, existing legacy link metadata, or an attached image.
- Creating a new root post or reply currently does not snapshot the author's
  current signature into `post.sign_id`. Existing `post.sign_id` values are
  display references captured in migrated data or earlier workflows.
- Initial image attachments are uploaded into a `YYYYMM` subdirectory under
  `IMAGE_DIRECTORY` with a random 128-bit lowercase hex file name, and the
  generated relative path is stored in `post.image_url`; an already attached
  image cannot be replaced. Existing migrated images and new uploads should
  share the same configured image root, for example
  `IMAGE_DIRECTORY=/home/wy/pic/dogn_pic`. Canonical monthly image paths do not
  include `pic/`; existing stored values that still contain the legacy `pic/`
  prefix are resolved by stripping the prefix before reading from the canonical
  image root. The editor does not accept arbitrary image URLs. Upload size is
  constrained by
  `IMAGE_UPLOAD_MAX_BYTES`, defaulting to 2 MB.
  Uploaded images larger than 500 KB are stored as compressed JPEG files
  reduced below 500 KB; smaller accepted images retain their input format.

Derived activity timestamp maintenance:

- `user_info.last_post` is derived from the latest visible authored post time.
- `user_info.last_origin` is derived from the latest visible authored original
  post (`type = 1`) time.
- `user_info.last_reship` is derived from the latest visible authored forward
  post (`type = 2`) time.
- Post creation, reply creation, post update, post soft deletion, and manual
  user-statistics recalculation refresh these fields from visible posts in
  states normal or encrypted.

Deferred post-write maintenance decisions:

- `post.reply_count` currently represents the number of stored tree members,
  including deleted posts. It is undecided whether deletion should instead
  make `reply_count` and `reply_time` reflect visible posts and the latest
  visible reply only.
- Post editing and soft deletion do not change `post.point`, `point_log`, or
  `user_info.point`. Reply creation changes these fields only through the
  explicit point-transfer operation above. Root-post creation may award the
  author directly in `user_info.point` as described above, without touching
  `post.point` or `point_log`. Other point workflows remain undecided.
- The legacy migrated `upd_log` table is not part of current application
  writes. It is undecided whether post edits/deletions should append audit
  entries there or whether the table should remain unused.

No post-write implementation should assume answers to these decisions until
they are explicitly approved.

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
- `last_origin`, `last_reship`, `last_post`: legacy activity timestamps
  maintained from visible authored posts. `last_post` is the latest visible
  post time, `last_origin` is the latest visible original-post (`type = 1`)
  time, and `last_reship` is the latest visible forward-post (`type = 2`) time.
- `login_count`: login counter.
- `point`: user point balance. Administrator-created accounts start from the
  configured `NEW_USER_INITIAL_POINTS` value, default `100`.
- `intro_user_id`: inferred inviter/referrer user id.
- `sign_id`: current signature id.
- `favorite_count`: denormalized favorite count.
- `log_error_time`, `log_error_count`: login error tracking fields.

Indexes:

- `user_info_pkey` on `id`.
- Unique `idx_user_info_trimmed_name` on `BTRIM(name)`; guarantees the
  application user-name identity used during account creation.
- `idx_user_info_name_trgm` and `idx_user_info_email_trgm` GIN indexes on
  normalized lowercase values; support literal case-insensitive substring
  search in the administrator user directory.
- `idx_user_info_level_id` on `level, id`; supports role-filtered directory
  paging.
- `idx_user_info_point_id` on `point DESC NULLS LAST, id`; supports the
  home-page top-points list.
- `idx_user_info_trimmed_email_level` on normalized email and level for rows
  with an email address; supports authentication and password-reset lookup.

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
- Successful authentication updates `last_login`, stores the application's
  direct TCP peer address in `last_login_ip`, and increments `login_count`.
  Failed authentication leaves these successful-login values unchanged.
- A failed authentication attempt for an existing user-name match updates
  `log_error_time` and increments `log_error_count`. An unknown user name has
  no account row to update. Successful login does not currently clear these
  historical failure fields.

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
- `user_id`: inferred reference to `user_info.id`; for reply point transfers,
  this identifies the user who gave the points.
- `post_id`: inferred reference to `post.id`.
- `create_time`: time the favorite was created.

Indexes:

- `favorite_pkey` on `id`.
- `idx_favorite_post_id` on `post_id`.
- Unique `idx_favorite_user_post` on `user_id, post_id`; guarantees one
  relationship per user/root-post pair and supports idempotent favorite writes.
- `idx_favorite_user_id_desc` on `user_id, id DESC`; supports paged favorite
  activity lists.

Application write rule:

- A user may set or unset a favorite only for a visible root post.
- `POST /api/posts/{post_id}/favorite` applies the requested selected state
  idempotently with the unique relationship index and refreshes
  `user_info.favorite_count` in the same transaction.

### `point_log`

Point history table. This is an event/history table, not just a join table,
because it records a point value and timestamp.

Important columns:

- `id`: primary identifier.
- `post_id`: inferred reference to `post.id`.
- `user_id`: inferred reference to `user_info.id`.
- `point`: point value for the event.
- `post_time`: event time.

Application write rule:

- A positive points value submitted with a reply creates one event for the
  root post being replied to; `user_id` identifies the user who gave the
  points. The replied-to root post owner is credited in `user_info.point`, and
  the replied-to root post is credited in `post.point`. Replies to non-root
  posts may not transfer points.

Indexes:

- `point_log_pkey` on `id`.
- `idx_point_log_post_time_id` on `post_id, post_time, id`; supports displaying
  point events for one post or a list of posts in stable event order.

### `sign_log`

Signature history table.

Each `sign_log` row records a user choosing a signature. `sign_id` is a
reference to the `post.id` of the post used as the signature. The latest
`sign_log` record for a specific user represents the signature currently used by
that user.

A post may be chosen as a signature only when its `post.size` is no greater
than `POST_SIGNATURE_MAX_BYTES`, which defaults to 1000 bytes. Re-selecting the
current signature is a no-op; selecting a different eligible post appends a new
history row.

Any post that appears in `sign_log` is considered signature history and is
locked against normal user edits, even if it is not the latest signature for any
user. Administrators may still update those posts.

Important columns:

- `id`: primary identifier.
- `user_id`: inferred reference to `user_info.id`.
- `sign_id`: inferred reference to `post.id`; the post used as the signature.
- `set_time`: time the signature was set.

Indexes:

- `sign_log_pkey` on `id`.
- `idx_sign_log_user_latest` on `user_id, set_time DESC NULLS LAST, id DESC`;
  supports finding a user's latest signature.
- `idx_sign_log_user_id_desc` on `user_id, id DESC`; supports legacy/latest
  signature queries ordered by id.
- `idx_sign_log_sign_id` on `sign_id`; supports checking whether a post has
  appeared in signature history.

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
- Should `info_bak` remain in the active application database or move to an
  archive-only path?
- The previously migrated `upd_log` table is not currently present in the public
  schema. Determine whether it was intentionally removed or should be restored.
