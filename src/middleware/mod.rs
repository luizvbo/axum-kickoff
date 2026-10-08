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
{% if metrics %}#[cfg(feature = "metrics")]
pub mod metrics;
{% endif %}pub mod normalize_path;
pub mod real_ip;
pub mod request_format;
pub mod request_id;
pub mod require_user_agent;
pub mod security_headers;
pub mod session;

pub use api_token::ApiTokenAuth;
pub use auth::{authenticate, require_auth, require_login, CurrentUserId, OptionalCurrentUserId};
pub use block_traffic::middleware as block_traffic;
pub use csrf::{
    csrf_protect, get_or_create_csrf_token, protect, validate_csrf_token, verify_origin,
};
pub use error_handler::middleware as error_handler;
{% if metrics %}#[cfg(feature = "metrics")]
pub use metrics::update_metrics;
{% endif %}pub use real_ip::middleware as real_ip;
pub use real_ip::RealIp;
pub use request_format::middleware as request_format;
pub use request_id::{middleware as request_id, RequestId};
pub use require_user_agent::require_user_agent;
pub use security_headers::{middleware as security_headers, CspNonce};
pub use session::{middleware as session_middleware, SessionExtension, SessionState};

/// Apply infrastructure middleware to the root router.
///
/// These layers run for every route — operational endpoints (`/health`,
/// `/static`, `/metrics`, `/swagger-ui`, `/api-docs`), application routes, and
/// the 404 fallback — so they must stay cheap and free of session/auth/CSRF/
/// rate-limit work. Those application-specific concerns are applied per route
/// group in `apply_app_middleware` instead, mirroring crates.io's split stack.
///
/// `.layer()` calls run inside-out: the last call is the outermost middleware.
pub fn apply_axum_middleware(state: AppState, router: Router<()>) -> Router {
    let config = &state.config;
    let env = config.env();

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
        // Innermost infra layer. Traffic blocking is applied globally (as in
        // crates.io) so blocked IPs/routes/user agents cannot reach operational
        // endpoints either; it only reads config, headers, and `RealIp`.
        .layer(from_fn_with_state(state.clone(), self::block_traffic))
        .layer(from_fn(log_request))
        .layer(from_fn_with_state(state.clone(), self::real_ip::middleware))
        // `error_handler` is a pure logging layer; `request_format` scopes the
        // `REQUEST_FORMAT` task-local for everything inside it so error
        // responses and `HtmlTemplate` can negotiate HTML/HTMX/JSON.
        .layer(from_fn(self::error_handler::middleware))
        .layer(from_fn(self::request_format::middleware))
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
{% if metrics %}
    #[cfg(feature = "metrics")]
    let router = router.layer(from_fn_with_state(
        state.clone(),
        self::metrics::update_metrics,
    ));
{% endif %}
    // CORS is the outermost layer so preflights are answered before the user
    // agent check, session middleware, or rate limiting can run.
    let router = router.layer(cors);

    // Optionally print debug information for each request in development
    if env == Env::Development {
        router.layer(from_fn(debug_requests))
    } else {
        router
    }
}

/// Apply application middleware to a router of HTML/API routes.
///
/// Session cookies, authentication, and origin verification run only for
/// these routes. Operational endpoints are registered on the root router (see
/// `router::build_axum_router`) and never see this stack. Rate limiting and
/// CSRF validation are applied even more selectively via `route_layer`.
pub(crate) fn apply_app_middleware(state: AppState, router: Router<AppState>) -> Router<AppState> {
    let env = state.config.env();

    // Determine whether session cookies should have the Secure flag.
    // Enabled by default in production, and can be toggled via SESSION_COOKIE_SECURE.
    let session_cookie_secure = match dotenvy::var("SESSION_COOKIE_SECURE").ok().as_deref() {
        Some(v) if v.trim().eq_ignore_ascii_case("true") || v.trim() == "1" => true,
        Some(v) if v.trim().eq_ignore_ascii_case("false") || v.trim() == "0" => false,
        _ => env == Env::Production,
    };
    let session_state = self::session::SessionState {
        key: state.0.session_key.clone(),
        secure: session_cookie_secure,
    };

    // innermost -> outermost: verify_origin -> authenticate -> session.
    // `session` runs first to populate `SessionExtension`; `authenticate`
    // resolves the auth context lazily from it; `verify_origin` checks the
    // Origin header just before the route-specific layers and handler.
    router
        .layer(from_fn_with_state(
            state.config.allowed_origins.clone(),
            self::csrf::verify_origin,
        ))
        .layer(from_fn_with_state(state.clone(), self::authenticate))
        .layer(from_fn_with_state(session_state, self::session_middleware))
}

pub(crate) async fn rate_limit(
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
        // OAuth handshake endpoints share one parameterized route
        // (`/api/v1/auth/{provider}/...`); match by shape, not provider name.
        (&http::Method::GET, p) if p.starts_with("/api/v1/auth/") && p.ends_with("/authorize") => {
            LimitedAction::OAuthAuthorize
        }
        (&http::Method::GET, p) if p.starts_with("/api/v1/auth/") && p.ends_with("/callback") => {
            LimitedAction::OAuthCallback
        }
        (&http::Method::POST, "/examples/contact") => LimitedAction::FormSubmission,
        (&http::Method::POST, "/logout") | (&http::Method::POST, "/api/v1/auth/logout") => {
            LimitedAction::FormSubmission
        }
        // Anonymous reads on the public API are throttled per client IP; the
        // mutating variants below `/api/v1/posts` are protected routes and
        // keep the generic `ApiRequest` budget.
        (&http::Method::GET, p) if p == "/api/v1/posts" || p.starts_with("/api/v1/posts/") => {
            LimitedAction::PublicApiRead
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
    // Log only selected, non-sensitive headers — `{:?}` on the whole request
    // would leak Authorization and Cookie values into the logs.
    const LOGGED_HEADERS: &[header::HeaderName] = &[
        header::USER_AGENT,
        header::REFERER,
        header::CONTENT_TYPE,
        header::ACCEPT,
        header::ORIGIN,
    ];

    let headers: Vec<(&header::HeaderName, &header::HeaderValue)> = LOGGED_HEADERS
        .iter()
        .filter_map(|name| req.headers().get(name).map(|value| (name, value)))
        .collect();

    tracing::debug!(method = %req.method(), uri = %req.uri(), ?headers, "Request");

    next.run(req).await
}
