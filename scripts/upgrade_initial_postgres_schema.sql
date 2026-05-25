-- Upgrade an initial PostgreSQL database produced by
-- scripts/migrate_mysql_to_postgres.sh to the current application naming
-- convention.
--
-- Run after loading the initial PostgreSQL database:
--
--   psql dogn -v ON_ERROR_STOP=1 -f scripts/upgrade_initial_postgres_schema.sql
--
-- This script renames the legacy schema and converts board-master names into
-- board/user relationships. It does not otherwise update content row values.
--
-- Summary of changes:
--
--   Tables:
--     article -> post
--     forum   -> board
--     user    -> user_info
--     board.MasterName* values -> board_master relationships
--
--   Naming convention:
--     mixed-case legacy column names -> lower snake_case
--     legacy index names             -> idx_<table>_<column_or_purpose>
--     legacy identity sequences      -> <table>_id_seq
--
-- Note about upd_log:
--   The current documented database notes say upd_log is not present in the
--   current public schema, but there is no explicit recorded decision in this
--   repo to delete it. Because dropping a table is destructive, this script
--   leaves upd_log intact. Review manually if the table should be archived or
--   removed.

BEGIN;

-- Table renames.
ALTER TABLE "article" RENAME TO "post";
ALTER TABLE "forum" RENAME TO "board";
ALTER TABLE "user" RENAME TO "user_info";

-- board columns, formerly forum.
ALTER TABLE "board" RENAME COLUMN "Id" TO id;
ALTER TABLE "board" RENAME COLUMN "Name" TO name;
ALTER TABLE "board" RENAME COLUMN "Comment" TO comment;
ALTER TABLE "board" RENAME COLUMN "CategoryId" TO category_id;
ALTER TABLE "board" RENAME COLUMN "ArtCount" TO post_count;
ALTER TABLE "board" RENAME COLUMN "RootCount" TO root_count;
ALTER TABLE "board" RENAME COLUMN "MasterName" TO master_name;
ALTER TABLE "board" RENAME COLUMN "MasterName2" TO master_name_2;
ALTER TABLE "board" RENAME COLUMN "MasterName3" TO master_name_3;
ALTER TABLE "board" RENAME COLUMN "MasterName4" TO master_name_4;
ALTER TABLE "board" RENAME COLUMN "MasterId" TO master_id;
ALTER TABLE "board" RENAME COLUMN "OrderId" TO order_id;

-- category columns.
ALTER TABLE "category" RENAME COLUMN "Id" TO id;
ALTER TABLE "category" RENAME COLUMN "Name" TO name;
ALTER TABLE "category" RENAME COLUMN "Comment" TO comment;
ALTER TABLE "category" RENAME COLUMN "OrderId" TO order_id;
ALTER TABLE "category" RENAME COLUMN "ForumCount" TO board_count;

-- favorite columns.
ALTER TABLE "favorite" RENAME COLUMN "Id" TO id;
ALTER TABLE "favorite" RENAME COLUMN "UserId" TO user_id;
ALTER TABLE "favorite" RENAME COLUMN "ArticleId" TO post_id;
ALTER TABLE "favorite" RENAME COLUMN "CreateTime" TO create_time;

-- info_bak columns.
ALTER TABLE "info_bak" RENAME COLUMN "ID" TO id;
ALTER TABLE "info_bak" RENAME COLUMN "usrlevel" TO user_level;
ALTER TABLE "info_bak" RENAME COLUMN "Email" TO email;
ALTER TABLE "info_bak" RENAME COLUMN "regtime" TO reg_time;
ALTER TABLE "info_bak" RENAME COLUMN "artcount" TO post_count;
ALTER TABLE "info_bak" RENAME COLUMN "doccount" TO doc_count;
ALTER TABLE "info_bak" RENAME COLUMN "lastlogin" TO last_login;
ALTER TABLE "info_bak" RENAME COLUMN "lastloginip" TO last_login_ip;
ALTER TABLE "info_bak" RENAME COLUMN "lastOrigin" TO last_origin;
ALTER TABLE "info_bak" RENAME COLUMN "lastReship" TO last_reship;
ALTER TABLE "info_bak" RENAME COLUMN "lastPost" TO last_post;
ALTER TABLE "info_bak" RENAME COLUMN "logincount" TO login_count;
ALTER TABLE "info_bak" RENAME COLUMN "introuser" TO intro_user_id;
ALTER TABLE "info_bak" RENAME COLUMN "signatureid" TO signature_id;
ALTER TABLE "info_bak" RENAME COLUMN "FavoCount" TO favorite_count;
ALTER TABLE "info_bak" RENAME COLUMN "logerrtime" TO log_error_time;
ALTER TABLE "info_bak" RENAME COLUMN "logerrcount" TO log_error_count;

-- point_log columns.
ALTER TABLE "point_log" RENAME COLUMN "Id" TO id;
ALTER TABLE "point_log" RENAME COLUMN "ArticleId" TO post_id;
ALTER TABLE "point_log" RENAME COLUMN "UserId" TO user_id;
ALTER TABLE "point_log" RENAME COLUMN "Point" TO point;
ALTER TABLE "point_log" RENAME COLUMN "PostTime" TO post_time;

-- post columns, formerly article.
ALTER TABLE "post" RENAME COLUMN "Id" TO id;
ALTER TABLE "post" RENAME COLUMN "ForumId" TO board_id;
ALTER TABLE "post" RENAME COLUMN "ParentId" TO parent_id;
ALTER TABLE "post" RENAME COLUMN "RootId" TO root_id;
ALTER TABLE "post" RENAME COLUMN "OrderNum" TO order_num;
ALTER TABLE "post" RENAME COLUMN "OrderNum2" TO order_num_2;
ALTER TABLE "post" RENAME COLUMN "Level" TO level;
ALTER TABLE "post" RENAME COLUMN "Subject" TO subject;
ALTER TABLE "post" RENAME COLUMN "Size" TO size;
ALTER TABLE "post" RENAME COLUMN "AccessCount" TO access_count;
ALTER TABLE "post" RENAME COLUMN "UserName" TO user_name;
ALTER TABLE "post" RENAME COLUMN "UserId" TO user_id;
ALTER TABLE "post" RENAME COLUMN "PostTime" TO post_time;
ALTER TABLE "post" RENAME COLUMN "ReplyCount" TO reply_count;
ALTER TABLE "post" RENAME COLUMN "ReplyTime" TO reply_time;
ALTER TABLE "post" RENAME COLUMN "Type" TO type;
ALTER TABLE "post" RENAME COLUMN "Content" TO content;
ALTER TABLE "post" RENAME COLUMN "LinkName" TO link_name;
ALTER TABLE "post" RENAME COLUMN "LinkStr" TO link_url;
ALTER TABLE "post" RENAME COLUMN "ImageLink" TO image_url;
ALTER TABLE "post" RENAME COLUMN "State" TO state;
ALTER TABLE "post" RENAME COLUMN "FolderId" TO folder_id;
ALTER TABLE "post" RENAME COLUMN "Point" TO point;
ALTER TABLE "post" RENAME COLUMN "SignId" TO sign_id;

-- sign_log columns.
ALTER TABLE "sign_log" RENAME COLUMN "Id" TO id;
ALTER TABLE "sign_log" RENAME COLUMN "UserId" TO user_id;
ALTER TABLE "sign_log" RENAME COLUMN "SignId" TO sign_id;
ALTER TABLE "sign_log" RENAME COLUMN "SetTime" TO set_time;

-- user_info columns, formerly user.
ALTER TABLE "user_info" RENAME COLUMN "Id" TO id;
ALTER TABLE "user_info" RENAME COLUMN "Name" TO name;
ALTER TABLE "user_info" RENAME COLUMN "Password" TO password;
ALTER TABLE "user_info" RENAME COLUMN "State" TO state;
ALTER TABLE "user_info" RENAME COLUMN "Level" TO level;
ALTER TABLE "user_info" RENAME COLUMN "Email" TO email;
ALTER TABLE "user_info" RENAME COLUMN "Intro" TO intro;
ALTER TABLE "user_info" RENAME COLUMN "RegTime" TO reg_time;
ALTER TABLE "user_info" RENAME COLUMN "ArticleCount" TO post_count;
ALTER TABLE "user_info" RENAME COLUMN "DocCount" TO doc_count;
ALTER TABLE "user_info" RENAME COLUMN "LastLogin" TO last_login;
ALTER TABLE "user_info" RENAME COLUMN "LastLoginIp" TO last_login_ip;
ALTER TABLE "user_info" RENAME COLUMN "LastOrigin" TO last_origin;
ALTER TABLE "user_info" RENAME COLUMN "LastReship" TO last_reship;
ALTER TABLE "user_info" RENAME COLUMN "LastPost" TO last_post;
ALTER TABLE "user_info" RENAME COLUMN "LoginCount" TO login_count;
ALTER TABLE "user_info" RENAME COLUMN "Point" TO point;
ALTER TABLE "user_info" RENAME COLUMN "IntroUserId" TO intro_user_id;
ALTER TABLE "user_info" RENAME COLUMN "SignId" TO sign_id;
ALTER TABLE "user_info" RENAME COLUMN "FavoCount" TO favorite_count;
ALTER TABLE "user_info" RENAME COLUMN "LogErrTime" TO log_error_time;
ALTER TABLE "user_info" RENAME COLUMN "LogErrCount" TO log_error_count;

-- Board managers are users related to boards, not names embedded in boards.
CREATE TABLE board_master (
    board_id integer NOT NULL REFERENCES board(id) ON DELETE CASCADE,
    user_id integer NOT NULL REFERENCES user_info(id) ON DELETE RESTRICT,
    order_id integer NOT NULL DEFAULT 0,
    CONSTRAINT board_master_pkey PRIMARY KEY (board_id, user_id)
);

CREATE INDEX idx_board_master_user_id ON board_master(user_id);

DO $$
DECLARE
    unmatched_count integer;
    ambiguous_count integer;
    duplicate_count integer;
BEGIN
    WITH master_names AS (
        SELECT b.id AS board_id, value.position, BTRIM(value.name) AS user_name
        FROM board b
        CROSS JOIN LATERAL (
            VALUES
                (1, b.master_name),
                (2, b.master_name_2),
                (3, b.master_name_3),
                (4, b.master_name_4)
        ) AS value(position, name)
        WHERE NULLIF(BTRIM(value.name), '') IS NOT NULL
    ),
    matches AS (
        SELECT master_names.board_id, master_names.position, master_names.user_name, COUNT(u.id) AS user_count
        FROM master_names
        LEFT JOIN user_info u ON BTRIM(u.name) = master_names.user_name
        GROUP BY master_names.board_id, master_names.position, master_names.user_name
    )
    SELECT
        COUNT(*) FILTER (WHERE user_count = 0),
        COUNT(*) FILTER (WHERE user_count > 1)
    INTO unmatched_count, ambiguous_count
    FROM matches;

    WITH master_names AS (
        SELECT b.id AS board_id, BTRIM(value.name) AS user_name
        FROM board b
        CROSS JOIN LATERAL (
            VALUES (b.master_name), (b.master_name_2), (b.master_name_3), (b.master_name_4)
        ) AS value(name)
        WHERE NULLIF(BTRIM(value.name), '') IS NOT NULL
    )
    SELECT COUNT(*)
    INTO duplicate_count
    FROM (
        SELECT board_id, user_name
        FROM master_names
        GROUP BY board_id, user_name
        HAVING COUNT(*) > 1
    ) duplicates;

    IF unmatched_count > 0 OR ambiguous_count > 0 OR duplicate_count > 0 THEN
        RAISE EXCEPTION
            'Cannot migrate board masters: % unmatched name(s), % ambiguous name(s), % duplicate board/name relation(s).',
            unmatched_count,
            ambiguous_count,
            duplicate_count;
    END IF;
END $$;

INSERT INTO board_master (board_id, user_id, order_id)
SELECT b.id, u.id, value.position
FROM board b
CROSS JOIN LATERAL (
    VALUES
        (1, b.master_name),
        (2, b.master_name_2),
        (3, b.master_name_3),
        (4, b.master_name_4)
) AS value(position, name)
JOIN user_info u ON BTRIM(u.name) = BTRIM(value.name)
WHERE NULLIF(BTRIM(value.name), '') IS NOT NULL
ORDER BY b.id, value.position;

ALTER TABLE board
    DROP COLUMN master_name,
    DROP COLUMN master_name_2,
    DROP COLUMN master_name_3,
    DROP COLUMN master_name_4;

-- Primary key and secondary index renames.
ALTER INDEX "article_pkey" RENAME TO post_pkey;
ALTER INDEX "forum_pkey" RENAME TO board_pkey;
ALTER INDEX "user_pkey" RENAME TO user_info_pkey;

ALTER INDEX "IdxForumId" RENAME TO idx_post_board_id;
ALTER INDEX "IdxParentId" RENAME TO idx_post_parent_id;
ALTER INDEX "IdxAccessCount" RENAME TO idx_post_access_count;
ALTER INDEX "IdxPoint_article" RENAME TO idx_post_point;
ALTER INDEX "IdxType" RENAME TO idx_post_type;
ALTER INDEX "IdxUserId_article" RENAME TO idx_post_user_id;
ALTER INDEX "IdxTreeOrder" RENAME TO idx_post_tree_order;
ALTER INDEX "IdxPostTimeAccessCount" RENAME TO idx_post_post_time_access_count;

-- Authorization lookup for local media attached to posts.
CREATE INDEX IF NOT EXISTS idx_post_normalized_image_url_state
ON post (
    (regexp_replace(regexp_replace(BTRIM(image_url), '^/+', ''), '^images/', '')),
    state
)
WHERE NULLIF(BTRIM(image_url), '') IS NOT NULL;

ALTER INDEX "IdxCategoryId" RENAME TO idx_board_category_id;

ALTER INDEX "IdxUserId_favorite" RENAME TO idx_favorite_user_id;
ALTER INDEX "IdxArticleId" RENAME TO idx_favorite_post_id;
ALTER INDEX "IdxCreateTime" RENAME TO idx_favorite_create_time;

ALTER INDEX "IdxPostTime" RENAME TO idx_point_log_post_time;

ALTER INDEX "IDX_USERID" RENAME TO idx_sign_log_user_id;
ALTER INDEX "IDX_SETTIME" RENAME TO idx_sign_log_set_time;

ALTER INDEX "IdxPoint_user" RENAME TO idx_user_info_point;
ALTER INDEX "IdxName" RENAME TO idx_user_info_name;
ALTER INDEX "IdxRegTime" RENAME TO idx_user_info_reg_time;

-- Identity sequence renames.
ALTER SEQUENCE "article_Id_seq" RENAME TO post_id_seq;
ALTER SEQUENCE "forum_Id_seq" RENAME TO board_id_seq;
ALTER SEQUENCE "user_Id_seq" RENAME TO user_info_id_seq;
ALTER SEQUENCE "category_Id_seq" RENAME TO category_id_seq;
ALTER SEQUENCE "favorite_Id_seq" RENAME TO favorite_id_seq;
ALTER SEQUENCE "point_log_Id_seq" RENAME TO point_log_id_seq;
ALTER SEQUENCE "sign_log_Id_seq" RENAME TO sign_log_id_seq;

COMMIT;
