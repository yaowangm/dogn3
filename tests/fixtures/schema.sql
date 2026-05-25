CREATE TABLE category (
    id integer PRIMARY KEY,
    name text NOT NULL,
    comment text,
    order_id integer NOT NULL DEFAULT 0,
    board_count integer
);

CREATE TABLE board (
    id integer PRIMARY KEY,
    name text NOT NULL,
    comment text,
    category_id integer NOT NULL REFERENCES category(id),
    post_count integer NOT NULL DEFAULT 0,
    root_count integer,
    master_name text,
    master_name_2 text,
    master_name_3 text,
    master_name_4 text,
    order_id integer NOT NULL DEFAULT 0
);

CREATE TABLE post (
    id integer PRIMARY KEY,
    subject text,
    board_id integer REFERENCES board(id),
    user_id integer,
    user_name text,
    post_time timestamp,
    reply_time timestamp,
    size integer,
    reply_count integer,
    access_count integer NOT NULL DEFAULT 0,
    point integer,
    type integer,
    state integer NOT NULL DEFAULT 0,
    content text,
    link_name text,
    link_url text,
    image_url text,
    sign_id integer,
    parent_id integer,
    root_id integer,
    level integer NOT NULL DEFAULT 0,
    order_num integer
);

CREATE TABLE user_info (
    id integer PRIMARY KEY,
    name text NOT NULL,
    password text NOT NULL,
    password_scheme text,
    state integer NOT NULL DEFAULT 0,
    level integer NOT NULL DEFAULT 1,
    email text,
    reg_time timestamp,
    post_count integer NOT NULL DEFAULT 0,
    doc_count integer,
    last_login timestamp,
    last_login_ip text,
    login_count integer,
    intro_user_id integer,
    point integer,
    intro text,
    favorite_count integer
);

CREATE TABLE point_log (
    id integer PRIMARY KEY,
    post_id integer NOT NULL REFERENCES post(id),
    user_id integer NOT NULL REFERENCES user_info(id),
    point integer NOT NULL,
    post_time timestamp
);

CREATE TABLE favorite (
    id integer PRIMARY KEY,
    user_id integer NOT NULL REFERENCES user_info(id),
    post_id integer NOT NULL REFERENCES post(id),
    create_time timestamp
);

CREATE TABLE sign_log (
    id integer PRIMARY KEY,
    user_id integer NOT NULL REFERENCES user_info(id),
    sign_id integer NOT NULL REFERENCES post(id),
    set_time timestamp
);
