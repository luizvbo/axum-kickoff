use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use http::{Method, StatusCode};

use crate::Env;
use crate::app::AppState;

pub fn build_axum_router(state: AppState) -> Router<()> {
    let mut router = Router::new()
        .route("/health", get(health_check));

    // Add development-only routes
    if state.config.env() == Env::Development {
        router = router.route("/debug", get(debug_info));
    }

    router
        .fallback(async |method: Method| match method {
            Method::HEAD => StatusCode::NOT_FOUND.into_response(),
            _ => not_found().into_response(),
        })
        .with_state(state)
}

async fn health_check() -> &'static str {
    "OK"
}

async fn debug_info() -> &'static str {
    "Debug mode enabled"
}

fn not_found() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}
