use axum::Router;
use axum::middleware::from_fn;
use http::StatusCode;
use std::time::Duration;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::{CompressionLayer, CompressionLevel};
use tower_http::timeout::{RequestBodyTimeoutLayer, TimeoutLayer};

use crate::Env;
use crate::app::AppState;

pub fn apply_axum_middleware(state: AppState, router: Router<()>) -> Router {
    let config = &state.config;
    let env = config.env();

    let middlewares = tower::ServiceBuilder::new()
        .layer(from_fn(log_request))
        .layer(CatchPanicLayer::new())
        // Optionally print debug information for each request in development
        .layer(conditional_layer(env == Env::Development, || {
            from_fn(debug_requests)
        }));

    router
        .layer(middlewares)
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(RequestBodyTimeoutLayer::new(Duration::from_secs(30)))
        .layer(CompressionLayer::new().quality(CompressionLevel::Fastest))
}

pub fn conditional_layer<L, F: FnOnce() -> L>(
    condition: bool,
    layer: F,
) -> axum_extra::either::Either<(axum::middleware::ResponseAxumBodyLayer, L), tower::layer::util::Identity> {
    axum_extra::middleware::option_layer(condition.then(layer))
}

async fn log_request(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    
    tracing::info!("{} {}", method, uri);
    
    next.run(req).await
}

async fn debug_requests(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    tracing::debug!("Request: {:?}", req);
    
    next.run(req).await
}
