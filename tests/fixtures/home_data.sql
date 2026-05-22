INSERT INTO category (id, name, order_id) VALUES
    (1, 'General', 1),
    (2, 'Tech', 2);

INSERT INTO board (
    id,
    name,
    comment,
    category_id,
    post_count,
    root_count,
    master_name,
    master_name_2,
    master_name_3,
    master_name_4,
    order_id
) VALUES
    (10, 'News', 'Announcements and updates', 1, 12, 5, 'Alice', '', NULL, NULL, 1),
    (11, 'Chat', 'General discussion', 1, 4, 2, 'Bob', 'Carol', NULL, NULL, 2),
    (20, 'Rust', 'Rust development', 2, 18, 8, '', NULL, NULL, NULL, 1);

INSERT INTO user_info (id, name, reg_time, post_count, point) VALUES
    (1, 'Alice', '2024-01-01 08:00:00', 4, 50),
    (2, 'Bob', '2024-01-02 08:00:00', 6, 90),
    (3, 'Carol', '2024-01-03 08:00:00', 2, 20);

INSERT INTO post (
    id,
    subject,
    board_id,
    user_id,
    user_name,
    post_time,
    reply_time,
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
    (100, 'Announcement', 10, 1, 'Alice', '2024-02-01 09:00:00', '2024-02-01 09:00:00', 0, 12, 5, 3, 0, NULL, 0, 100, 0, 0),
    (101, 'Original root', 11, 2, 'Bob', '2024-02-02 09:00:00', '2024-02-02 09:10:00', 1, 20, 10, 1, 0, 'https://example.test/image.png', 0, 101, 0, 0),
    (102, 'Original reply', 11, 3, 'Carol', '2024-02-02 09:10:00', '2024-02-02 09:10:00', 0, 3, 1, 0, 0, NULL, 101, 101, 1, 1),
    (105, 'Nested reply', 11, 1, 'Alice', '2024-02-02 09:20:00', '2024-02-02 09:20:00', 0, 2, 0, 0, 0, NULL, 102, 101, 2, 2),
    (103, 'Forward root', 20, 3, 'Carol', '2024-02-03 09:00:00', '2024-02-03 09:00:00', 0, 30, 8, 2, 1, NULL, 0, 103, 0, 0),
    (104, 'Deleted root', 20, 1, 'Alice', '2024-02-04 09:00:00', '2024-02-04 09:00:00', 0, 5, 0, 0, 2, NULL, 0, 104, 0, 0),
    (106, 'Second chat root', 11, 3, 'Carol', '2024-02-05 09:00:00', '2024-02-05 09:00:00', 0, 9, 2, 0, 0, NULL, 0, 106, 0, 0);
