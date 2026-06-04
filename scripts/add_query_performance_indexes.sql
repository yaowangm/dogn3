-- Add indexes for current application query paths.
--
-- This script changes database schema only. It does not change table data.
-- It is intentionally additive: index drops should be decided after reviewing
-- live index usage with scripts/review_index_usage.sql.
--
-- Usage:
--   psql dogn -v ON_ERROR_STOP=1 -f scripts/add_query_performance_indexes.sql
--
-- Notes:
--   - CREATE INDEX CONCURRENTLY cannot run inside an explicit transaction.
--   - Run this script from psql without wrapping it in BEGIN/COMMIT.
--   - The PGroonga text-search indexes remain managed by
--     scripts/add_post_pgroonga_search_indexes.sql.

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_post_visible_board_tree_order
    ON post (board_id, (COALESCE(root_id, id)) DESC, order_num)
    WHERE state IN (0, 1);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_post_visible_tree_order
    ON post ((COALESCE(root_id, id)), order_num)
    WHERE state IN (0, 1);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_post_tree_order_mutation
    ON post (root_id, order_num)
    WHERE state IN (0, 1);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_post_visible_tree_post_time
    ON post ((COALESCE(root_id, id)), post_time ASC NULLS LAST, id ASC)
    WHERE state IN (0, 1);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_post_visible_user_type_id
    ON post (user_id, type, id DESC)
    WHERE state IN (0, 1);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_post_visible_user_post_time
    ON post (user_id, post_time DESC)
    WHERE state IN (0, 1);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_post_visible_user_type_post_time
    ON post (user_id, type, post_time DESC)
    WHERE state IN (0, 1);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_post_visible_board_roots
    ON post (board_id, id)
    WHERE state IN (0, 1)
      AND COALESCE(parent_id, 0) = 0;

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_post_visible_type_id
    ON post (type, id DESC)
    WHERE state IN (0, 1);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_post_visible_roots_home
    ON post ((COALESCE(root_id, id)) DESC, order_num)
    WHERE state IN (0, 1)
      AND COALESCE(parent_id, 0) = 0;

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_favorite_user_post
    ON favorite (user_id, post_id);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_favorite_user_id_desc
    ON favorite (user_id, id DESC);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_sign_log_user_latest
    ON sign_log (user_id, set_time DESC NULLS LAST, id DESC);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_sign_log_user_id_desc
    ON sign_log (user_id, id DESC);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_sign_log_sign_id
    ON sign_log (sign_id);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_point_log_post_time_id
    ON point_log (post_id, post_time, id);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_board_master_board_order
    ON board_master (board_id, order_id, user_id);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_board_category_order
    ON board (category_id, order_id, id);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_category_order
    ON category (order_id, id);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_user_info_point_id
    ON user_info (point DESC NULLS LAST, id);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_user_info_level_id
    ON user_info (level, id);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_user_info_trimmed_name
    ON user_info (BTRIM(name));

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_user_info_trimmed_email_level
    ON user_info (LOWER(BTRIM(email)), level)
    WHERE NULLIF(BTRIM(email), '') IS NOT NULL;

ANALYZE post;
ANALYZE favorite;
ANALYZE sign_log;
ANALYZE point_log;
ANALYZE board_master;
ANALYZE board;
ANALYZE category;
ANALYZE user_info;
