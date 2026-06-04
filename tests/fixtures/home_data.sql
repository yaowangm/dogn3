INSERT INTO category (id, name, comment, order_id, board_count) VALUES
    (1, 'General', 'General discussion categories', 1, 2),
    (2, 'Tech', 'Technical discussion categories', 2, 1);

INSERT INTO board (
    id,
    name,
    comment,
    category_id,
    post_count,
    root_count,
    order_id
) VALUES
    (10, 'News', 'Announcements and updates', 1, 12, 5, 1),
    (11, 'Chat', 'General discussion', 1, 4, 2, 2),
    (20, 'Rust', 'Rust development', 2, 18, 8, 1);

SELECT setval(pg_get_serial_sequence('category', 'id'), (SELECT MAX(id) FROM category));
SELECT setval(pg_get_serial_sequence('board', 'id'), (SELECT MAX(id) FROM board));

INSERT INTO user_info (
    id, name, password, level, email, reg_time, post_count, doc_count, last_login,
    last_login_ip, last_origin, last_reship, last_post, login_count, intro_user_id,
    point, intro, favorite_count
) VALUES
    (1, 'Alice', '00000000000000000000000000000001', 10, 'alice@example.test', '2024-01-01 08:00:00', 4, 2, '2024-02-07 08:00:00', '127.0.0.1', NULL, NULL, '2024-02-04 10:00:00', 13, NULL, 50, 'Moderator profile.', 1),
    (2, 'Bob', '00000000000000000000000000000002', 5, 'bob@example.test', '2024-01-02 08:00:00', 6, 3, '2024-02-08 09:30:00', '192.0.2.2', '2024-02-02 09:00:00', NULL, '2024-02-02 09:00:00', 21, 1, 90, 'Rust reader.', 2),
    (3, 'Carol', '00000000000000000000000000000003', 5, 'carol@example.test', '2024-01-03 08:00:00', 2, 1, NULL, NULL, NULL, '2024-02-03 09:00:00', '2024-02-05 09:00:00', 1, NULL, 20, NULL, 0);

SELECT setval(pg_get_serial_sequence('user_info', 'id'), (SELECT MAX(id) FROM user_info));

INSERT INTO board_master (board_id, user_id, order_id) VALUES
    (10, 1, 1),
    (11, 2, 1),
    (11, 3, 2);

INSERT INTO post (
    id,
    subject,
    board_id,
    user_id,
    user_name,
    post_time,
    reply_time,
    size,
    reply_count,
    access_count,
    point,
    type,
    state,
    image_url,
    parent_id,
    root_id,
    level,
    order_num
) VALUES
    (100, 'Announcement', 10, 1, 'Alice', '2024-02-01 09:00:00', '2024-02-01 09:00:00', 420, 0, 12, 5, 3, 0, NULL, 0, 100, 0, 0),
    (101, 'Original root', 11, 2, 'Bob', '2024-02-02 09:00:00', '2024-02-02 09:10:00', 810, 1, 20, 10, 1, 0, 'https://example.test/image.png', 0, 101, 0, 0),
    (102, 'Original reply', 11, 3, 'Carol', '2024-02-02 09:10:00', '2024-02-02 09:10:00', 230, 0, 3, 1, 0, 0, NULL, 101, 101, 1, 1),
    (105, 'Nested reply', 11, 1, 'Alice', '2024-02-02 09:20:00', '2024-02-02 09:20:00', 125, 0, 2, 0, 0, 0, NULL, 102, 101, 2, 2),
    (103, 'Forward root', 20, 3, 'Carol', '2024-02-03 09:00:00', '2024-02-03 09:00:00', 612, 0, 30, 8, 2, 1, NULL, 0, 103, 0, 0),
    (104, 'Deleted root', 20, 1, 'Alice', '2024-02-04 09:00:00', '2024-02-04 09:00:00', 90, 0, 5, 0, 0, 2, NULL, 0, 104, 0, 0),
    (107, 'Unknown state root', 20, 1, 'Alice', '2024-02-04 10:00:00', '2024-02-04 10:00:00', 91, 0, 4, 0, 0, 9, 'unknown.JPG', 0, 107, 0, 0),
    (106, 'Second chat root', 11, 3, 'Carol', '2024-02-05 09:00:00', '2024-02-05 09:00:00', 356, 0, 9, 2, 0, 0, NULL, 0, 106, 0, 0);

SELECT setval(pg_get_serial_sequence('post', 'id'), (SELECT MAX(id) FROM post));

UPDATE post
SET content = 'Signature: keep learning.'
WHERE id = 100;

UPDATE post
SET content = E'A full original post.\nSecond paragraph.',
    link_name = 'Reference',
    link_url = 'https://example.test/reference',
    sign_id = 100
WHERE id = 101;

UPDATE post
SET content = 'Encrypted body.',
    link_name = 'Private reference',
    link_url = 'https://example.test/private',
    image_url = 'private.JPG',
    sign_id = 100
WHERE id = 103;

UPDATE post
SET content = 'Content with an unsupported visibility state.'
WHERE id = 107;

INSERT INTO point_log (id, post_id, user_id, point, post_time) VALUES
    (1, 101, 2, 8, '2024-02-02 10:00:00'),
    (2, 101, 3, 2, '2024-02-02 10:01:00'),
    (3, 103, 3, 8, '2024-02-03 10:00:00');

SELECT setval(pg_get_serial_sequence('point_log', 'id'), (SELECT MAX(id) FROM point_log));

INSERT INTO favorite (id, user_id, post_id, create_time) VALUES
    (1, 2, 103, '2024-02-04 10:00:00'),
    (2, 2, 101, '2024-02-03 10:00:00');

SELECT setval(pg_get_serial_sequence('favorite', 'id'), (SELECT MAX(id) FROM favorite));

INSERT INTO sign_log (id, user_id, sign_id, set_time) VALUES
    (1, 2, 100, '2024-02-01 10:00:00'),
    (2, 2, 101, '2024-02-02 10:00:00');
