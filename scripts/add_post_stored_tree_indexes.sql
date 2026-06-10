-- Add indexes for post-tree operations that intentionally include every stored
-- post in a tree, including deleted rows.
--
-- This script changes database schema only. It does not change table data.
--
-- Usage:
--   psql dogn -v ON_ERROR_STOP=1 -f scripts/add_post_stored_tree_indexes.sql
--
-- Notes:
--   - CREATE INDEX CONCURRENTLY cannot run inside an explicit transaction.
--   - Run this script from psql without wrapping it in BEGIN/COMMIT.
--   - These indexes complement the visible-post partial indexes. They are for
--     mutation/count paths whose SQL does not filter by state.

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_post_tree_order_all
    ON post (root_id, order_num);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_post_effective_root_all
    ON post ((COALESCE(root_id, id)));

ANALYZE post;
