\set ON_ERROR_STOP on

-- One-time migration for an already upgraded PostgreSQL dogn database.
-- Converts board manager names into ordered board/user relationships.
-- board.master_id is retained because its legacy purpose is separate from the
-- four name slots and has not yet been defined for removal.

BEGIN;

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

COMMIT;
