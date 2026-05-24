\set ON_ERROR_STOP on

-- Supports authorization checks for /images/{path}. Run once after the post
-- table has been renamed and its columns converted to snake_case.
CREATE INDEX IF NOT EXISTS idx_post_normalized_image_url_state
ON post (
    (regexp_replace(regexp_replace(BTRIM(image_url), '^/+', ''), '^images/', '')),
    state
)
WHERE NULLIF(BTRIM(image_url), '') IS NOT NULL;
