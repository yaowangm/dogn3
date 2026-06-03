-- Add post.last_update_time for recording explicit post edits.
--
-- Run manually after reviewing the schema change:
--
--   psql dogn -v ON_ERROR_STOP=1 -f scripts/add_post_last_update_time.sql
--
-- The column is nullable on purpose. Existing posts keep NULL, which means
-- "no application-recorded edit time". Application code will set it when a
-- post is updated.
--
-- This script is safe to rerun.

BEGIN;

ALTER TABLE post
    ADD COLUMN IF NOT EXISTS last_update_time timestamp;

COMMIT;

SELECT column_name, data_type, is_nullable
FROM information_schema.columns
WHERE table_name = 'post'
  AND column_name = 'last_update_time';
