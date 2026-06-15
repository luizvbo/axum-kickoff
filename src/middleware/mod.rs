use axum::Router;
use axum::middleware::from_fn;
use http::StatusCode;
use std::time::Duration;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::{CompressionLayer, CompressionLevel};
use tower_http::timeout::{RequestBodyTimeoutLayer, TimeoutLayer};
use tracing::Instrument;

use crate::Env;
use crate::app::AppState;

pub mod block_traffic;
pub mod error_handler;
#[cfg(feature = "metrics")]
pub mod metrics;
pub mod real_ip;
pub mod require_user_agent;
pub mod security_headers;
pub mod session;

pub use block_traffic::middleware as block_traffic;
pub use error_handler::middleware as error_handler;
#[cfg(feature = "metrics")]
pub use metrics::update_metrics;
pub use real_ip::middleware as real_ip;
pub use require_user_agent::require_user_agent;
pub use security_headers::middleware as security_headers;
pub use session::{middleware as session_middleware, SessionExtension};

pub fn apply_axum_middleware(state: AppState, router: Router<()>) -> Router {
    let config = &state.config;
    let env = config.env();

    let router = router
        .layer(from_fn(self::real_ip::middleware))
        .layer(from_fn(log_request))
        .layer(from_fn(self::error_handler::middleware))
        .layer(from_fn(self::session_middleware))
        .layer(CatchPanicLayer::new())
        .layer(from_fn(self::require_user_agent::require_user_agent))
        .layer(from_fn(self::security_headers::middleware))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(RequestBodyTimeoutLayer::new(Duration::from_secs(30)))
        .layer(CompressionLayer::new().quality(CompressionLevel::Fastest));

    #[cfg(feature = "metrics")]
    let router = router.layer(from_fn_with_state(state.clone(), self::metrics::update_metrics));

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
    let user_agent = req
        .headers()
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("<unknown>");

    // Create a tracing span for structured logging
    let span = tracing::info_span!(
        "http_request",
        method = %method,
        uri = %uri,
        user_agent = %user_agent,
    );

    async move {
        tracing::info!("{} {}", method, uri);
        next.run(req).await
    }
    .instrument(span)
    .await
}

async fn debug_requests(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    tracing::debug!("Request: {:?}", req);
    
    next.run(req).await
}
