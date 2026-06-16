use askama::Template;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use chrono::Utc;
use http::{Method, StatusCode};
use tower_http::services::ServeDir;

use crate::app::AppState;
use crate::controllers::auth::{github_authorize, github_callback, logout};
use crate::Env;

pub fn build_axum_router(state: AppState) -> Router<()> {
    let mut router = Router::new()
        .route("/", get(home))
        .route("/health", get(health_check))
        .route("/api/server-time", get(server_time))
        .route("/api/v1/auth/github/authorize", get(github_authorize))
        .route("/api/v1/auth/github/callback", get(github_callback))
        .route("/api/v1/auth/logout", post(logout))
        .nest_service("/static", ServeDir::new("static"));

    // Add development-only routes
    if state.config.env() == Env::Development {
        router = router.route("/debug", get(debug_info));
    }

    router
        .fallback(async |method: Method| match method {
            Method::HEAD => StatusCode::NOT_FOUND.into_response(),
            _ => {
                use crate::util::errors::not_found;
                not_found().into_response()
            }
        })
        .with_state(state)
}

async fn home() -> impl IntoResponse {
    let template = IndexTemplate {};
    HtmlTemplate(template)
}

async fn health_check() -> &'static str {
    "OK"
}

async fn debug_info() -> &'static str {
    "Debug mode enabled"
}

async fn server_time() -> impl IntoResponse {
    let time = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let template = ServerTimeTemplate { time };
    HtmlTemplate(template)
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {}

#[derive(Template)]
#[template(path = "server_time.html")]
struct ServerTimeTemplate {
    time: String,
}

struct HtmlTemplate<T>(T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template: {}", err),
            )
                .into_response(),
        }
    }
}
