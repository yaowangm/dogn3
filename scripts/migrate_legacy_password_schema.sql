-- Schema fragment for the one-time active credential transformation.
--
-- Do not execute this file independently. It is included and executed by:
--
--   DATABASE_URL=postgres:///dogn cargo run --bin migrate_legacy_passwords -- --execute
--
-- The Rust command executes this DDL and all credential value replacements in
-- one PostgreSQL transaction. This prevents a committed intermediate state in
-- which the schema changed but legacy MD5-only values remain in user_info.
--
-- This migration covers active credentials in user_info only. The legacy
-- info_bak table may contain password material and must be handled by a
-- separate archive/deletion/security decision.

ALTER TABLE user_info
    ALTER COLUMN password TYPE text;

ALTER TABLE user_info
    ADD COLUMN IF NOT EXISTS password_scheme text;

COMMENT ON COLUMN user_info.password IS
    'PHC-formatted password hash; legacy active values migrated to Argon2id over their MD5 digest';

COMMENT ON COLUMN user_info.password_scheme IS
    'Credential input scheme; initial migrated value is argon2id-md5-v1';
