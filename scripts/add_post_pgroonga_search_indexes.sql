-- Add PGroonga indexes for Chinese/multilingual post search.
--
-- This script changes database schema only:
--   - creates the PGroonga extension in this database
--   - drops/recreates PGroonga search indexes
--
-- It does not change post data.
--
-- Prerequisite on Ubuntu 24.04 / PostgreSQL 16:
--   sudo apt install -y -V ca-certificates lsb-release wget
--   wget https://packages.groonga.org/ubuntu/groonga-apt-source-latest-$(lsb_release --codename --short).deb
--   sudo apt install -y -V ./groonga-apt-source-latest-$(lsb_release --codename --short).deb
--   rm -f groonga-apt-source-latest-$(lsb_release --codename --short).deb
--   sudo apt update
--   sudo apt install -y -V postgresql-16-pgroonga
--
-- Usage:
--   psql dogn -v ON_ERROR_STOP=1 -f scripts/add_post_pgroonga_search_indexes.sql

CREATE EXTENSION IF NOT EXISTS pgroonga;

DROP INDEX IF EXISTS idx_post_search_subject_pgroonga;
DROP INDEX IF EXISTS idx_post_search_content_pgroonga;
DROP INDEX IF EXISTS idx_post_search_user_name_pgroonga;

CREATE INDEX idx_post_search_subject_pgroonga
    ON post
    USING pgroonga ((COALESCE(subject, '')::text))
    WHERE state IN (0, 1);

CREATE INDEX idx_post_search_content_pgroonga
    ON post
    USING pgroonga ((COALESCE(content, '')::text))
    WHERE state IN (0, 1);

CREATE INDEX idx_post_search_user_name_pgroonga
    ON post
    USING pgroonga ((COALESCE(user_name, '')::text))
    WHERE state IN (0, 1);
