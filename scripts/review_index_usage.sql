-- Review index usage and likely redundant legacy indexes.
--
-- This script is read-only. Use it after representative traffic has run on the
-- current application. Do not drop indexes only because idx_scan is zero on a
-- freshly reset statistics collector.
--
-- Usage:
--   psql dogn -f scripts/review_index_usage.sql

\echo 'Index usage for application tables'
SELECT
    schemaname,
    relname AS table_name,
    indexrelname AS index_name,
    pg_size_pretty(pg_relation_size(indexrelid)) AS index_size,
    idx_scan,
    idx_tup_read,
    idx_tup_fetch
FROM pg_stat_user_indexes
WHERE relname IN (
    'post',
    'favorite',
    'sign_log',
    'point_log',
    'board_master',
    'board',
    'category',
    'user_info',
    'password_reset_token'
)
ORDER BY relname, idx_scan, indexrelname;

\echo 'Index definitions for application tables'
SELECT
    tablename AS table_name,
    indexname AS index_name,
    indexdef
FROM pg_indexes
WHERE tablename IN (
    'post',
    'favorite',
    'sign_log',
    'point_log',
    'board_master',
    'board',
    'category',
    'user_info',
    'password_reset_token'
)
ORDER BY tablename, indexname;

\echo 'Candidate legacy indexes to inspect manually before dropping'
SELECT
    candidate.index_name,
    candidate.reason,
    pg_size_pretty(pg_relation_size(indexes.oid)) AS index_size,
    COALESCE(stats.idx_scan, 0) AS idx_scan
FROM (
    VALUES
        ('idx_post_tree_order', 'Legacy index uses order_num_2; current code orders trees by order_num.'),
        ('idx_post_access_count', 'Current code increments access_count by primary key but does not filter/order by access_count alone.'),
        ('idx_post_point', 'Current code updates post.point by primary key and does not filter/order posts by point alone.'),
        ('idx_sign_log_set_time', 'Current code reads latest signatures by user plus time/id, not global set_time alone.')
) AS candidate(index_name, reason)
LEFT JOIN pg_class indexes ON indexes.relname = candidate.index_name
LEFT JOIN pg_stat_user_indexes stats ON stats.indexrelid = indexes.oid
ORDER BY candidate.index_name;

\echo 'Duplicate favorite relationships; should be zero before considering a unique favorite(user_id, post_id) constraint'
SELECT user_id, post_id, COUNT(*) AS duplicate_count
FROM favorite
GROUP BY user_id, post_id
HAVING COUNT(*) > 1
ORDER BY duplicate_count DESC, user_id, post_id
LIMIT 20;
