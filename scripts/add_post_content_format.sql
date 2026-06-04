-- Add a post body format marker for client-side Markdown rendering.
--
-- Values:
--   0 = plain text, the legacy/default format
--   1 = Markdown, rendered by the client after sanitization
--
-- Existing rows are initialized as plain text. This script only changes schema
-- and metadata; it does not rewrite existing post content.

ALTER TABLE post
    ADD COLUMN IF NOT EXISTS content_format smallint NOT NULL DEFAULT 0;

ALTER TABLE post
    DROP CONSTRAINT IF EXISTS post_content_format_check;

ALTER TABLE post
    ADD CONSTRAINT post_content_format_check
    CHECK (content_format IN (0, 1));

COMMENT ON COLUMN post.content_format IS
    'Post body format: 0=plain text, 1=Markdown rendered client-side after sanitization.';
