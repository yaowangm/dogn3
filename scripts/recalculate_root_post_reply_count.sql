-- Recalculate reply_count for root posts as the total stored number of posts
-- in each tree, including the root post itself.
--
-- Run manually after reviewing the counting rule:
--
--   psql dogn -v ON_ERROR_STOP=1 -f scripts/recalculate_root_post_reply_count.sql
--
-- Counting rule:
--   - Only root post rows are updated.
--   - A member belongs to tree COALESCE(member.root_id, member.id).
--   - Every stored member is counted, regardless of post.state.
--   - Non-root reply_count values are left unchanged.
--
-- This script is safe to rerun: roots whose stored value is already correct
-- are not updated.

BEGIN;

WITH tree_counts AS (
    SELECT
        COALESCE(root_id, id) AS root_id,
        COUNT(*)::integer AS post_count
    FROM post
    GROUP BY COALESCE(root_id, id)
),
updated_roots AS (
    UPDATE post AS root
    SET reply_count = tree_counts.post_count
    FROM tree_counts
    WHERE root.id = tree_counts.root_id
      AND COALESCE(root.parent_id, 0) = 0
      AND root.reply_count IS DISTINCT FROM tree_counts.post_count
    RETURNING root.id
)
SELECT COUNT(*) AS updated_root_posts
FROM updated_roots;

COMMIT;

WITH tree_counts AS (
    SELECT
        COALESCE(root_id, id) AS root_id,
        COUNT(*)::integer AS post_count
    FROM post
    GROUP BY COALESCE(root_id, id)
)
SELECT COUNT(*) AS inconsistent_root_posts_after_update
FROM post AS root
JOIN tree_counts ON tree_counts.root_id = root.id
WHERE COALESCE(root.parent_id, 0) = 0
  AND root.reply_count IS DISTINCT FROM tree_counts.post_count;
