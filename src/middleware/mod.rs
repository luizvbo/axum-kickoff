use axum::Router;
use axum::middleware::from_fn;
use http::StatusCode;
use std::time::Duration;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::{CompressionLayer, CompressionLevel};
use tower_http::timeout::{RequestBodyTimeoutLayer, TimeoutLayer};

use crate::Env;
use crate::app::AppState;

pub mod block_traffic;
#[cfg(feature = "metrics")]
pub mod metrics;
pub mod real_ip;
pub mod require_user_agent;
pub mod session;

pub use block_traffic::middleware as block_traffic;
#[cfg(feature = "metrics")]
pub use metrics::update_metrics;
pub use real_ip::middleware as real_ip;
pub use require_user_agent::require_user_agent;
pub use session::{attach_session, SessionExtension};

pub fn apply_axum_middleware(state: AppState, router: Router<()>) -> Router {
    let config = &state.config;
    let env = config.env();

    let router = router
        .layer(from_fn(log_request))
        .layer(CatchPanicLayer::new())
        .layer(from_fn(self::require_user_agent::require_user_agent))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(RequestBodyTimeoutLayer::new(Duration::from_secs(30)))
        .layer(CompressionLayer::new().quality(CompressionLevel::Fastest));

    // Optionally print debug information for each request in development
    if env == Env::Development {
        router.layer(from_fn(debug_requests))
    } else {
        router
    }
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
