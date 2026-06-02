mod auth;
mod board;
mod health;
mod home;
mod images;
mod pages;
mod post;
mod post_update;
mod site;
mod user;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};

use crate::state::AppState;

pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/boards/{board_id}", get(board::board))
        .route("/posts/{post_id}", get(post::post))
        .route(
            "/post_upd",
            get(post_update::editor).post(post_update::save),
        )
        .route(
            "/posts/{post_id}/image",
            post(post_update::upload_image).layer(DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route("/posts/{post_id}/delete", post(post_update::delete))
        .route("/posts/{post_id}/favorite", post(post_update::favorite))
        .route("/posts/{post_id}/signature", post(post_update::signature))
        .route("/post_lists/{post_id}", get(post::post_list))
        .route("/post_prints/{post_id}", get(post::post_print))
        .route("/users/{user_id}", get(user::user))
        .route("/users", get(user::user_list).post(user::create_user))
        .route("/site_manager", get(site::manager))
        .route("/site_manager/categories", post(site::create_category))
        .route(
            "/site_manager/categories/{category_id}",
            post(site::update_category),
        )
        .route(
            "/site_manager/categories/{category_id}/delete",
            post(site::delete_category),
        )
        .route("/site_manager/boards", post(site::create_board))
        .route("/site_manager/boards/{board_id}", post(site::update_board))
        .route(
            "/site_manager/boards/{board_id}/delete",
            post(site::delete_board),
        )
        .route(
            "/site_manager/boards/{board_id}/masters",
            post(site::add_board_master),
        )
        .route(
            "/site_manager/boards/{board_id}/masters/{user_id}/remove",
            post(site::remove_board_master),
        )
        .route(
            "/site_manager/boards/statistics/recalculate",
            post(site::recalculate_board_statistics),
        )
        .route("/users/{user_id}/password", post(auth::change_password))
        .route(
            "/users/{user_id}/statistics/recalculate",
            post(user::recalculate_statistics),
        )
        .route("/users/{user_id}/profile", post(user::update_profile))
        .route("/users/{user_id}/role", post(user::set_role))
        .route("/auth/login", post(auth::login))
        .route("/auth/session", get(auth::session))
        .route("/auth/logout", post(auth::logout))
        .route("/health", get(health::health))
        .route("/home", get(home::home))
}

pub fn page_router() -> Router<AppState> {
    Router::new()
        .route("/", get(pages::index))
        .route("/board/{board_id}", get(pages::index))
        .route("/post/{post_id}", get(pages::index))
        .route("/post_list/{post_id}", get(pages::index))
        .route("/post_print/{post_id}", get(pages::print))
        .route("/post_upd", get(pages::index))
        .route("/login", get(pages::index))
        .route("/user/{user_id}", get(pages::index))
        .route("/user_list", get(pages::index))
        .route("/user_add", get(pages::index))
        .route("/site_mgr", get(pages::index))
}

pub fn media_router() -> Router<AppState> {
    Router::new().route("/images/{*path}", get(images::image))
}
