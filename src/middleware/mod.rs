use axum::extract::State;
use axum::middleware::from_fn;
use axum::middleware::from_fn_with_state;
use axum::Router;
use http::{header, StatusCode};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::{CompressionLayer, CompressionLevel};
use tower_http::cors::{Any, CorsLayer};
use tower_http::timeout::{RequestBodyTimeoutLayer, TimeoutLayer};
use tracing::{info, info_span, Instrument};

use crate::app::AppState;
use crate::Env;

pub mod api_token;
pub mod auth;
pub mod block_traffic;
pub mod csrf;
pub mod error_handler;
#[cfg(feature = "metrics")]
pub mod metrics;
pub mod normalize_path;
pub mod real_ip;
pub mod request_id;
pub mod require_user_agent;
pub mod security_headers;
pub mod session;

pub use api_token::ApiTokenAuth;
pub use auth::{authenticate, require_auth, require_login, CurrentUserId, OptionalCurrentUserId};
pub use block_traffic::middleware as block_traffic;
pub use csrf::{
    csrf_protect, ensure_token, get_or_create_csrf_token, protect, validate_csrf_token,
    verify_origin,
};
pub use error_handler::middleware as error_handler;
#[cfg(feature = "metrics")]
pub use metrics::update_metrics;
pub use real_ip::middleware as real_ip;
pub use real_ip::RealIp;
pub use request_id::{middleware as request_id, RequestId};
pub use require_user_agent::require_user_agent;
pub use security_headers::{middleware as security_headers, CspNonce};
pub use session::{middleware as session_middleware, SessionExtension};

pub fn apply_axum_middleware(state: AppState, router: Router<()>) -> Router {
    let config = &state.config;
    let env = config.env();
    let session_key = state.0.session_key.clone();
    let security_headers_config = self::security_headers::SecurityHeadersConfig::for_env(env);

    // Build CORS layer from allowed origins
    let cors = CorsLayer::new()
        .allow_origin(
            config
                .allowed_origins
                .origins()
                .iter()
                .map(|s| s.parse().unwrap())
                .collect::<Vec<_>>(),
        )
        .allow_methods(Any)
        .allow_headers(Any);

    let router = router
        .layer(cors)
        .layer(from_fn_with_state(
            config.allowed_origins.clone(),
            self::csrf::verify_origin,
        ))
        // Core auth + rate limiting stack (innermost first):
        // rate_limit -> authenticate -> block_traffic -> ensure_token -> session -> log_request
        // real_ip and request_id run outside this group so they are available to middleware below.
        .layer(from_fn_with_state(state.clone(), self::rate_limit))
        .layer(from_fn_with_state(state.clone(), self::authenticate))
        .layer(from_fn_with_state(state.clone(), self::block_traffic))
        .layer(from_fn(self::csrf::ensure_token))
        .layer(from_fn_with_state(session_key, self::session_middleware))
        .layer(from_fn(log_request))
        .layer(from_fn_with_state(state.clone(), self::real_ip::middleware))
        .layer(from_fn(self::error_handler::middleware))
        .layer(from_fn(self::request_id::middleware))
        .layer(CatchPanicLayer::new())
        .layer(from_fn(self::require_user_agent::require_user_agent))
        .layer(from_fn_with_state(
            security_headers_config,
            self::security_headers::middleware,
        ))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(RequestBodyTimeoutLayer::new(Duration::from_secs(30)))
        .layer(CompressionLayer::new().quality(CompressionLevel::Fastest));

    #[cfg(feature = "metrics")]
    let router = router.layer(from_fn_with_state(
        state.clone(),
        self::metrics::update_metrics,
    ));

    // Optionally print debug information for each request in development
    if env == Env::Development {
        router.layer(from_fn(debug_requests))
    } else {
        router
    }
}

async fn rate_limit(
    State(state): State<AppState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use crate::util::errors::rate_limited;

    let real_ip = req
        .extensions()
        .get::<RealIp>()
        .map(|ip| ip.0.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let bucket_id = req
        .extensions()
        .get::<CurrentUserId>()
        .map(|user| user.0.to_string())
        .or_else(|| {
            req.extensions()
                .get::<SessionExtension>()
                .and_then(|s| s.get("user_id"))
        })
        .unwrap_or(real_ip);

    let action = determine_limited_action(req.method(), req.uri().path());

    match state
        .0
        .rate_limiter
        .check_rate_limit(&bucket_id, action)
        .await
    {
        Ok(()) => next.run(req).await,
        Err(e) => rate_limited(e.action.error_message(), e.retry_after).response(),
    }
}

fn determine_limited_action(
    method: &http::Method,
    path: &str,
) -> crate::rate_limiter::LimitedAction {
    use crate::rate_limiter::LimitedAction;

    let path = path.trim_end_matches('/');
    match (method, path) {
        (&http::Method::POST, "/api/v1/tokens") => LimitedAction::TokenCreation,
        (&http::Method::GET, "/api/v1/auth/github/authorize") => LimitedAction::OAuthAuthorize,
        (&http::Method::GET, "/api/v1/auth/github/callback") => LimitedAction::OAuthCallback,
        (&http::Method::POST, "/examples/contact") => LimitedAction::FormSubmission,
        (&http::Method::POST, "/logout") | (&http::Method::POST, "/api/v1/auth/logout") => {
            LimitedAction::FormSubmission
        }
        _ => LimitedAction::ApiRequest,
    }
}

async fn log_request(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let method = req.method().clone();
    let uri = req.uri().path().to_string();
    let user_agent = req
        .headers()
        .get(header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("<unknown>")
        .to_string();

    let request_id = req
        .extensions()
        .get::<self::request_id::RequestId>()
        .map(|r| r.0.clone())
        .unwrap_or_else(|| "<unknown>".to_string());

    let client_ip = req
        .extensions()
        .get::<self::real_ip::RealIp>()
        .map(|r| r.0.to_string())
        .unwrap_or_else(|| "<unknown>".to_string());

    let hashed_authorization = hash_header(req.headers(), header::AUTHORIZATION);
    let hashed_cookie = hash_header(req.headers(), header::COOKIE);

    let span = info_span!("http_request");

    async move {
        let start = Instant::now();
        let response = next.run(req).await;
        let duration = start.elapsed();
        let status = response.status();

        info!(
            target: "http",
            {
                http.method = %method,
                http.url = %uri,
                http.status_code = status.as_u16(),
                duration_ms = duration.as_millis() as u64,
                http.request.id = %request_id,
                network.client.ip = %client_ip,
                http.user_agent = %user_agent,
                http.request.headers.hashed_authorization = %hashed_authorization,
                http.request.headers.hashed_cookie = %hashed_cookie,
            },
            "{method} {uri} -> {status} ({duration:?})",
        );

        response
    }
    .instrument(span)
    .await
}

fn hash_header(headers: &http::HeaderMap, name: http::header::HeaderName) -> String {
    headers
        .get(name)
        .map(|h| hex::encode(Sha256::digest(h.as_bytes())))
        .unwrap_or_default()
}

async fn debug_requests(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    tracing::debug!("Request: {:?}", req);

    next.run(req).await
}
