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

\echo 'Performance database migrations completed'

