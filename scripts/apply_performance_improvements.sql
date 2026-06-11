\set ON_ERROR_STOP on

-- Apply database changes required by server performance improvements.
--
-- This is the cumulative deployment script for performance work. Add future
-- performance-related schema or data migrations to this file in execution
-- order. Every section must be safe to run against an already upgraded
-- database.
--
-- Usage:
--   psql dogn -v ON_ERROR_STOP=1 -f scripts/apply_performance_improvements.sql

\echo 'Applying performance database migrations'

-- ---------------------------------------------------------------------------
-- 1. Prepare sign_log.id for concurrent sequence-backed inserts.
-- ---------------------------------------------------------------------------
--
-- This section:
--   - reuses the existing serial/identity sequence when one is already attached
--   - creates public.sign_log_id_seq only when sign_log.id has no sequence
--   - makes a newly created sequence the default for sign_log.id
--   - advances the sequence to the current maximum sign_log.id
--
-- It does not update, delete, or renumber any sign_log row.

BEGIN;

-- Prevent concurrent legacy MAX(id) + 1 inserts while the sequence is aligned.
LOCK TABLE sign_log IN ACCESS EXCLUSIVE MODE;

DO $migration$
DECLARE
    sequence_name text;
    maximum_id bigint;
BEGIN
    SELECT pg_get_serial_sequence('public.sign_log', 'id')
    INTO sequence_name;

    IF sequence_name IS NULL THEN
        CREATE SEQUENCE IF NOT EXISTS public.sign_log_id_seq AS integer;
        ALTER SEQUENCE public.sign_log_id_seq OWNED BY public.sign_log.id;
        ALTER TABLE public.sign_log
            ALTER COLUMN id SET DEFAULT nextval('public.sign_log_id_seq'::regclass);
        sequence_name := 'public.sign_log_id_seq';
    END IF;

    SELECT MAX(id)::bigint
    INTO maximum_id
    FROM public.sign_log;

    IF maximum_id IS NULL THEN
        PERFORM setval(sequence_name::regclass, 1, false);
    ELSE
        PERFORM setval(sequence_name::regclass, maximum_id, true);
    END IF;
END
$migration$;

COMMIT;

\echo 'sign_log.id sequence configuration'
SELECT
    column_name,
    is_identity,
    identity_generation,
    column_default,
    pg_get_serial_sequence('public.sign_log', 'id') AS sequence_name
FROM information_schema.columns
WHERE table_schema = 'public'
  AND table_name = 'sign_log'
  AND column_name = 'id';

\echo 'sign_log.id range'
SELECT COUNT(*) AS row_count, MIN(id) AS minimum_id, MAX(id) AS maximum_id
FROM public.sign_log;

-- ---------------------------------------------------------------------------
-- 2. Normalize the legacy zero root id representation.
-- ---------------------------------------------------------------------------
--
-- Runtime tree queries and expression indexes use COALESCE(root_id, id).
-- Legacy root_id = 0 rows have the same meaning as NULL but cannot use those
-- expression indexes consistently.

BEGIN;

UPDATE public.post
SET root_id = NULL
WHERE root_id = 0;

COMMIT;

-- ---------------------------------------------------------------------------
-- 3. Enforce unique normalized user names.
-- ---------------------------------------------------------------------------
--
-- The migrated database contains two frozen, unreferenced legacy placeholder
-- accounts whose names both became "?". Preserve both rows under stable,
-- non-login placeholder names before enforcing uniqueness. The repair requires
-- the verified legacy account state and no relationship from an active table;
-- otherwise the row is left unchanged and the duplicate preflight below stops
-- the migration for manual review.

BEGIN;

UPDATE public.user_info
SET name = 'legacy-user-535'
WHERE id = 535
  AND BTRIM(name) = '?'
  AND state = 1
  AND level = 0
  AND COALESCE(post_count, 0) = 0
  AND COALESCE(doc_count, 0) = 0
  AND COALESCE(login_count, 0) = 0
  AND NOT EXISTS (SELECT 1 FROM public.post WHERE user_id = 535)
  AND NOT EXISTS (SELECT 1 FROM public.favorite WHERE user_id = 535)
  AND NOT EXISTS (SELECT 1 FROM public.point_log WHERE user_id = 535)
  AND NOT EXISTS (SELECT 1 FROM public.sign_log WHERE user_id = 535)
  AND NOT EXISTS (SELECT 1 FROM public.board_master WHERE user_id = 535)
  AND NOT EXISTS (SELECT 1 FROM public.password_reset_token WHERE user_id = 535)
  AND NOT EXISTS (SELECT 1 FROM public.user_info WHERE intro_user_id = 535);

UPDATE public.user_info
SET name = 'legacy-user-536'
WHERE id = 536
  AND BTRIM(name) = '?'
  AND state = 1
  AND level = 0
  AND COALESCE(post_count, 0) = 0
  AND COALESCE(doc_count, 0) = 0
  AND COALESCE(login_count, 0) = 0
  AND NOT EXISTS (SELECT 1 FROM public.post WHERE user_id = 536)
  AND NOT EXISTS (SELECT 1 FROM public.favorite WHERE user_id = 536)
  AND NOT EXISTS (SELECT 1 FROM public.point_log WHERE user_id = 536)
  AND NOT EXISTS (SELECT 1 FROM public.sign_log WHERE user_id = 536)
  AND NOT EXISTS (SELECT 1 FROM public.board_master WHERE user_id = 536)
  AND NOT EXISTS (SELECT 1 FROM public.password_reset_token WHERE user_id = 536)
  AND NOT EXISTS (SELECT 1 FROM public.user_info WHERE intro_user_id = 536);

DO $migration$
DECLARE
    unresolved_placeholders text;
BEGIN
    SELECT string_agg(id::text, ', ' ORDER BY id)
    INTO unresolved_placeholders
    FROM public.user_info
    WHERE id IN (535, 536)
      AND BTRIM(name) = '?';

    IF unresolved_placeholders IS NOT NULL THEN
        RAISE EXCEPTION
            'Known legacy placeholder account(s) require manual review before renaming: user_info.id %',
            unresolved_placeholders;
    END IF;
END
$migration$;

COMMIT;

-- Abort with a useful error before replacing the old non-unique expression
-- index. Duplicate names must be resolved manually because choosing an account
-- to rename is a product/data decision.

DO $migration$
DECLARE
    duplicate_names text;
BEGIN
    SELECT string_agg(format('%L (%s rows)', normalized_name, duplicate_count), ', ')
    INTO duplicate_names
    FROM (
        SELECT BTRIM(name) AS normalized_name, COUNT(*) AS duplicate_count
        FROM public.user_info
        GROUP BY BTRIM(name)
        HAVING COUNT(*) > 1
        ORDER BY COUNT(*) DESC, BTRIM(name)
        LIMIT 20
    ) duplicates;

    IF duplicate_names IS NOT NULL THEN
        RAISE EXCEPTION
            'Cannot create unique normalized user-name index. Resolve duplicates first: %',
            duplicate_names;
    END IF;
END
$migration$;

SELECT EXISTS (
    SELECT 1
    FROM pg_catalog.pg_index index_metadata
    WHERE index_metadata.indexrelid =
          to_regclass('public.idx_user_info_trimmed_name')
      AND index_metadata.indisunique
      AND index_metadata.indisvalid
      AND index_metadata.indisready
) AS normalized_user_name_index_ready
\gset

\if :normalized_user_name_index_ready
    \echo 'Unique normalized user-name index already configured'
    DROP INDEX CONCURRENTLY IF EXISTS public.idx_user_info_trimmed_name_replacement;
\else
    SELECT EXISTS (
        SELECT 1
        FROM pg_catalog.pg_index index_metadata
        WHERE index_metadata.indexrelid =
              to_regclass('public.idx_user_info_trimmed_name_replacement')
          AND index_metadata.indisunique
          AND index_metadata.indisvalid
          AND index_metadata.indisready
    ) AS normalized_user_name_replacement_ready
    \gset

    \if :normalized_user_name_replacement_ready
        \echo 'Reusing prepared unique normalized user-name replacement index'
    \else
        DROP INDEX CONCURRENTLY IF EXISTS public.idx_user_info_trimmed_name_replacement;

        CREATE UNIQUE INDEX CONCURRENTLY idx_user_info_trimmed_name_replacement
            ON public.user_info (BTRIM(name));
    \endif

    DROP INDEX CONCURRENTLY IF EXISTS public.idx_user_info_trimmed_name;

    ALTER INDEX public.idx_user_info_trimmed_name_replacement
        RENAME TO idx_user_info_trimmed_name;
\endif

-- ---------------------------------------------------------------------------
-- 4. Enforce one favorite relationship per user/root post.
-- ---------------------------------------------------------------------------

DO $migration$
DECLARE
    duplicate_favorites text;
BEGIN
    SELECT string_agg(
        format('(user_id=%s, post_id=%s, %s rows)', user_id, post_id, duplicate_count),
        ', '
    )
    INTO duplicate_favorites
    FROM (
        SELECT user_id, post_id, COUNT(*) AS duplicate_count
        FROM public.favorite
        GROUP BY user_id, post_id
        HAVING COUNT(*) > 1
        ORDER BY COUNT(*) DESC, user_id, post_id
        LIMIT 20
    ) duplicates;

    IF duplicate_favorites IS NOT NULL THEN
        RAISE EXCEPTION
            'Cannot create unique favorite index. Resolve duplicates first: %',
            duplicate_favorites;
    END IF;
END
$migration$;

SELECT EXISTS (
    SELECT 1
    FROM pg_catalog.pg_index index_metadata
    WHERE index_metadata.indexrelid =
          to_regclass('public.idx_favorite_user_post')
      AND index_metadata.indisunique
      AND index_metadata.indisvalid
      AND index_metadata.indisready
) AS favorite_user_post_index_ready
\gset

\if :favorite_user_post_index_ready
    \echo 'Unique favorite user/post index already configured'
    DROP INDEX CONCURRENTLY IF EXISTS public.idx_favorite_user_post_replacement;
\else
    SELECT EXISTS (
        SELECT 1
        FROM pg_catalog.pg_index index_metadata
        WHERE index_metadata.indexrelid =
              to_regclass('public.idx_favorite_user_post_replacement')
          AND index_metadata.indisunique
          AND index_metadata.indisvalid
          AND index_metadata.indisready
    ) AS favorite_user_post_replacement_ready
    \gset

    \if :favorite_user_post_replacement_ready
        \echo 'Reusing prepared unique favorite user/post replacement index'
    \else
        DROP INDEX CONCURRENTLY IF EXISTS public.idx_favorite_user_post_replacement;

        CREATE UNIQUE INDEX CONCURRENTLY idx_favorite_user_post_replacement
            ON public.favorite (user_id, post_id);
    \endif

    DROP INDEX CONCURRENTLY IF EXISTS public.idx_favorite_user_post;

    ALTER INDEX public.idx_favorite_user_post_replacement
        RENAME TO idx_favorite_user_post;
\endif

-- ---------------------------------------------------------------------------
-- 5. Add indexed literal-substring search for the administrator user list.
-- ---------------------------------------------------------------------------

CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_user_info_name_trgm
    ON public.user_info
    USING gin (LOWER(BTRIM(name)) gin_trgm_ops);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_user_info_email_trgm
    ON public.user_info
    USING gin (LOWER(COALESCE(BTRIM(email), '')) gin_trgm_ops);

-- ---------------------------------------------------------------------------
-- 6. Support the once-per-day root-post award lookup.
-- ---------------------------------------------------------------------------

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_post_root_award_lookup
    ON public.post (user_id, post_time, type)
    WHERE COALESCE(parent_id, 0) = 0;

-- ---------------------------------------------------------------------------
-- 7. Remove indexes superseded by current composite indexes/query paths.
-- ---------------------------------------------------------------------------

DROP INDEX CONCURRENTLY IF EXISTS public.idx_board_category_id;
DROP INDEX CONCURRENTLY IF EXISTS public.idx_favorite_create_time;
DROP INDEX CONCURRENTLY IF EXISTS public.idx_favorite_user_id;
DROP INDEX CONCURRENTLY IF EXISTS public.idx_point_log_post_time;
DROP INDEX CONCURRENTLY IF EXISTS public.idx_post_post_time_access_count;
DROP INDEX CONCURRENTLY IF EXISTS public.idx_post_type;
DROP INDEX CONCURRENTLY IF EXISTS public.idx_sign_log_user_id;
DROP INDEX CONCURRENTLY IF EXISTS public.idx_user_info_name;
DROP INDEX CONCURRENTLY IF EXISTS public.idx_user_info_point;
DROP INDEX CONCURRENTLY IF EXISTS public.idx_user_info_reg_time;

ANALYZE public.post;
ANALYZE public.favorite;
ANALYZE public.user_info;
ANALYZE public.sign_log;
ANALYZE public.board;
ANALYZE public.point_log;

\echo 'Performance database migrations completed'
