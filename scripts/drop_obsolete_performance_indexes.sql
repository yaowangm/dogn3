-- Drop indexes that are obsolete or duplicated by the current application
-- search/performance strategy.
--
-- This script changes database schema only. It does not change table data.
--
-- Usage:
--   psql dogn -v ON_ERROR_STOP=1 -f scripts/drop_obsolete_performance_indexes.sql
--
-- Notes:
--   - DROP INDEX CONCURRENTLY cannot run inside an explicit transaction.
--   - Run this script from psql without wrapping it in BEGIN/COMMIT.
--   - The tsvector/trigram post search indexes are obsolete because current
--     code uses PGroonga `&@` predicates for subject/content/user_name.
--   - The *_visible indexes below duplicate older partial btree search indexes
--     with the same predicates. PostgreSQL can scan btree indexes in either
--     direction, so a separate DESC copy is unnecessary.

DROP INDEX CONCURRENTLY IF EXISTS idx_post_search_subject_tsv;
DROP INDEX CONCURRENTLY IF EXISTS idx_post_search_content_tsv;
DROP INDEX CONCURRENTLY IF EXISTS idx_post_search_user_name_tsv;
DROP INDEX CONCURRENTLY IF EXISTS idx_post_search_subject_trgm;
DROP INDEX CONCURRENTLY IF EXISTS idx_post_search_content_trgm;
DROP INDEX CONCURRENTLY IF EXISTS idx_post_search_user_name_trgm;

DROP INDEX CONCURRENTLY IF EXISTS idx_post_search_visible_id_desc;
DROP INDEX CONCURRENTLY IF EXISTS idx_post_search_post_time_visible;
DROP INDEX CONCURRENTLY IF EXISTS idx_post_search_reply_time_visible;
DROP INDEX CONCURRENTLY IF EXISTS idx_post_search_has_image_visible;
DROP INDEX CONCURRENTLY IF EXISTS idx_post_search_has_link_visible;

ANALYZE post;
