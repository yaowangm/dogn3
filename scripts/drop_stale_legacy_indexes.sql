-- Drop legacy indexes that are not used by current application query paths.
--
-- This script changes database schema only. It does not change table data.
--
-- Usage:
--   psql dogn -v ON_ERROR_STOP=1 -f scripts/drop_stale_legacy_indexes.sql
--
-- Notes:
--   - DROP INDEX CONCURRENTLY cannot run inside an explicit transaction.
--   - Run this script from psql without wrapping it in BEGIN/COMMIT.
--   - These indexes were retained from the legacy migration. Current runtime
--     code does not filter/order posts by access_count or point alone, does
--     not use order_num_2 for tree ordering, and reads signatures by user
--     rather than by global set_time.
--   - idx_post_tree_order_mutation is superseded by idx_post_tree_order_all.

DROP INDEX CONCURRENTLY IF EXISTS idx_post_access_count;
DROP INDEX CONCURRENTLY IF EXISTS idx_post_point;
DROP INDEX CONCURRENTLY IF EXISTS idx_post_tree_order;
DROP INDEX CONCURRENTLY IF EXISTS idx_post_tree_order_mutation;
DROP INDEX CONCURRENTLY IF EXISTS idx_sign_log_set_time;

ANALYZE post;
ANALYZE sign_log;
