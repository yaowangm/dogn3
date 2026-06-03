-- Rebuild indexes for authenticated post search.
--
-- This script is safe to rerun. It drops and recreates only search-related
-- indexes. It does not change post data.
--
-- Usage:
--   psql dogn -v ON_ERROR_STOP=1 -f scripts/add_post_search_indexes.sql

CREATE EXTENSION IF NOT EXISTS pg_trgm;

DROP INDEX IF EXISTS idx_post_search_has_link;
DROP INDEX IF EXISTS idx_post_search_has_image;
DROP INDEX IF EXISTS idx_post_search_user_name_trgm;
DROP INDEX IF EXISTS idx_post_search_content_trgm;
DROP INDEX IF EXISTS idx_post_search_subject_trgm;
DROP INDEX IF EXISTS idx_post_search_user_name_tsv;
DROP INDEX IF EXISTS idx_post_search_content_tsv;
DROP INDEX IF EXISTS idx_post_search_subject_tsv;
DROP INDEX IF EXISTS idx_post_search_type;
DROP INDEX IF EXISTS idx_post_search_reply_time;
DROP INDEX IF EXISTS idx_post_search_post_time;
DROP INDEX IF EXISTS idx_post_search_visible_id;

CREATE INDEX IF NOT EXISTS idx_post_search_visible_id
    ON post(id)
    WHERE state IN (0, 1);

CREATE INDEX IF NOT EXISTS idx_post_search_post_time
    ON post(post_time)
    WHERE state IN (0, 1);

CREATE INDEX IF NOT EXISTS idx_post_search_reply_time
    ON post(reply_time)
    WHERE state IN (0, 1);

CREATE INDEX IF NOT EXISTS idx_post_search_type
    ON post(type)
    WHERE state IN (0, 1);

CREATE INDEX IF NOT EXISTS idx_post_search_subject_tsv
    ON post USING gin (to_tsvector('simple', COALESCE(subject, '')))
    WHERE state IN (0, 1);

CREATE INDEX IF NOT EXISTS idx_post_search_content_tsv
    ON post USING gin (to_tsvector('simple', COALESCE(content, '')))
    WHERE state IN (0, 1);

CREATE INDEX IF NOT EXISTS idx_post_search_user_name_tsv
    ON post USING gin (to_tsvector('simple', COALESCE(user_name, '')))
    WHERE state IN (0, 1);

CREATE INDEX IF NOT EXISTS idx_post_search_subject_trgm
    ON post USING gin (subject gin_trgm_ops)
    WHERE state IN (0, 1);

CREATE INDEX IF NOT EXISTS idx_post_search_content_trgm
    ON post USING gin (content gin_trgm_ops)
    WHERE state IN (0, 1);

CREATE INDEX IF NOT EXISTS idx_post_search_user_name_trgm
    ON post USING gin (user_name gin_trgm_ops)
    WHERE state IN (0, 1);

CREATE INDEX IF NOT EXISTS idx_post_search_has_image
    ON post(id)
    WHERE state IN (0, 1)
      AND NULLIF(BTRIM(image_url), '') IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_post_search_has_link
    ON post(id)
    WHERE state IN (0, 1)
      AND NULLIF(BTRIM(link_url), '') IS NOT NULL;
