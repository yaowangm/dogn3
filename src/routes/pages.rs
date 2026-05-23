use axum::response::Html;

pub async fn index() -> Html<&'static str> {
    Html(include_str!("../../static/index.html"))
}

pub async fn print() -> Html<&'static str> {
    Html(include_str!("../../static/post_print.html"))
}
