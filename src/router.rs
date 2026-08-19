use askama::Template;
use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::Router;
use http::{HeaderValue, Method, StatusCode};
use serde::Serialize;
use tower_http::services::ServeDir;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::app::AppState;
use crate::controllers::auth::{github_authorize, github_callback, logout_api, logout_html};
use crate::controllers::examples::{
    contact_page, contact_submit, counter_decrement, counter_increment, counter_page, example_json,
};
use crate::controllers::post::{
    create_post, delete_post, list_posts, publish_post, show_post, unpublish_post, update_post,
};
use crate::controllers::token::{create_token, list_tokens, revoke_token};
use crate::middleware::security_headers::current_csp_nonce;
use crate::middleware::{
    csrf_protect, get_or_create_csrf_token, require_auth, CspNonce, SessionExtension,
};
use crate::models::User;
use crate::util::errors::BoxedAppError;
use crate::Env;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::controllers::post::list_posts,
        crate::controllers::post::show_post,
        crate::controllers::post::create_post,
        crate::controllers::post::update_post,
        crate::controllers::post::delete_post,
        crate::controllers::post::publish_post,
        crate::controllers::post::unpublish_post,
        crate::controllers::token::create_token,
        crate::controllers::token::list_tokens,
        crate::controllers::token::revoke_token,
    ),
    components(
        schemas(
            crate::controllers::post::CreatePostRequest,
            crate::controllers::post::UpdatePostRequest,
            crate::controllers::post::PostResponse,
            crate::controllers::post::ListPostsResponse,
            crate::controllers::token::CreateTokenRequest,
            crate::controllers::token::CreateTokenResponse,
            crate::controllers::token::TokenListItem,
            crate::controllers::token::ListTokensResponse,
        )
    ),
    tags(
        (name = "Posts", description = "Blog post CRUD operations"),
        (name = "Tokens", description = "API token management")
    ),
    info(
        title = "axum-kickoff API",
        version = "0.1.0",
        description = "A pragmatic Axum + Askama + HTMX starter API"
    ),
    servers(
        (url = "http://localhost:8888", description = "Local development server")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
struct ApiDoc;

/// Common context passed to every full-page and partial HTML template.
#[derive(Clone, Debug)]
pub struct PageContext {
    /// CSRF token for the current session.
    pub csrf_token: String,
    /// CSP nonce for the current request.
    pub csp_nonce: String,
}

impl<S: Send + Sync> FromRequestParts<S> for PageContext {
    type Rejection = BoxedAppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let csp_nonce = parts
            .extensions
            .get::<CspNonce>()
            .map(|n| n.0.clone())
            .unwrap_or_else(current_csp_nonce);

        let csrf_token = parts
            .extensions
            .get::<SessionExtension>()
            .map(get_or_create_csrf_token)
            .unwrap_or_default();

        Ok(PageContext {
            csrf_token,
            csp_nonce,
        })
    }
}

pub fn build_axum_router(state: AppState) -> Router<()> {
    // Public HTML / example router - no authentication required
    let public_router = Router::new()
        .route("/", get(home))
        .route("/health", get(health_check))
        .route("/api/server-time", get(server_time))
        .route("/api/v1/auth/github/authorize", get(github_authorize))
        .route("/api/v1/auth/github/callback", get(github_callback))
        // Example routes for HTMX + Askama patterns
        .route("/examples/contact", get(contact_page))
        .route("/examples/contact", post(contact_submit))
        .route("/examples/counter", get(counter_page))
        .route("/examples/counter/increment", post(counter_increment))
        .route("/examples/counter/decrement", post(counter_decrement))
        .route("/examples/json", get(example_json));

    // Public API v1 read-only routes
    let api_v1_public = Router::new()
        .route("/api/v1/posts", get(list_posts))
        .route("/api/v1/posts/{id}", get(show_post));

    // Protected API v1 routes - requires authentication and CSRF for cookie sessions
    let api_v1_protected = Router::new()
        .route("/api/v1/auth/logout", post(logout_api))
        .route("/logout", post(logout_html))
        .route("/api/v1/tokens", post(create_token))
        .route("/api/v1/tokens", get(list_tokens))
        .route("/api/v1/tokens/{token_id}", post(revoke_token))
        // Post mutating routes
        .route("/api/v1/posts", post(create_post))
        .route("/api/v1/posts/{id}", patch(update_post))
        .route("/api/v1/posts/{id}", delete(delete_post))
        .route("/api/v1/posts/{id}/publish", post(publish_post))
        .route("/api/v1/posts/{id}/unpublish", post(unpublish_post))
        .route_layer(axum::middleware::from_fn(csrf_protect))
        .route_layer(axum::middleware::from_fn(require_auth));

    // Combine all stateful routes
    let api_router = Router::new()
        .merge(public_router)
        .merge(api_v1_public)
        .merge(api_v1_protected)
        .nest_service(
            "/static",
            ServeDir::new("static")
                .precompressed_gzip()
                .precompressed_br(),
        );

    // Add development-only routes
    let api_router = if state.config.env() == Env::Development {
        api_router.route("/debug", get(debug_info))
    } else {
        api_router
    };

    #[cfg(feature = "metrics")]
    let api_router = api_router
        .route("/metrics", get(crate::metrics::metrics_handler))
        .route("/api/private/metrics", get(crate::metrics::metrics_handler));

    let api_router = api_router
        .fallback(async |method: Method| match method {
            Method::HEAD => StatusCode::NOT_FOUND.into_response(),
            _ => {
                use crate::util::errors::not_found;
                not_found().into_response()
            }
        })
        .with_state(state);

    // Merge Swagger UI with stateless router, then merge the stateful API router
    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(api_router)
}

async fn home(ctx: PageContext) -> impl IntoResponse {
    let template = IndexTemplate { ctx };
    HtmlTemplate::new(template)
}

async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    let mut db = state.0.database.db_clone();
    let db_ok = User::filter(User::fields().id().eq(0))
        .first()
        .exec(&mut db)
        .await
        .is_ok();

    let status = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let body = HealthResponse {
        status: if db_ok { "ok" } else { "degraded" },
        database: db_ok,
    };
    (status, axum::Json(body))
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    database: bool,
}

async fn debug_info() -> &'static str {
    "Debug mode enabled"
}

async fn server_time(ctx: PageContext) -> impl IntoResponse {
    let time = jiff::Timestamp::now()
        .strftime("%Y-%m-%d %H:%M:%S UTC")
        .to_string();
    let full = ServerTimeTemplate {
        ctx: ctx.clone(),
        time: time.clone(),
    };
    let partial = ServerTimePartialTemplate { ctx, time };
    HtmlTemplate::new(full).with_partial(partial)
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    ctx: PageContext,
}

#[derive(Template)]
#[template(path = "server_time.html")]
#[allow(dead_code)]
struct ServerTimeTemplate {
    ctx: PageContext,
    time: String,
}

#[derive(Template)]
#[template(path = "server_time_partial.html")]
#[allow(dead_code)]
struct ServerTimePartialTemplate {
    ctx: PageContext,
    time: String,
}

/// Response wrapper that renders an Askama template, optionally swapping to a
/// partial template when the request was made by HTMX (`HX-Request: true`).
pub struct HtmlTemplate<T> {
    full: T,
    partial: Option<Box<dyn Fn() -> askama::Result<String> + Send>>,
}

impl<T: Template> HtmlTemplate<T> {
    /// Render the full template for both HTMX and non-HTMX requests.
    pub fn new(full: T) -> Self {
        Self {
            full,
            partial: None,
        }
    }

    /// Provide a partial template to render for HTMX requests.
    pub fn with_partial<P: Template + Send + 'static>(mut self, partial: P) -> Self {
        self.partial = Some(Box::new(move || partial.render()));
        self
    }
}

impl<T: Template> IntoResponse for HtmlTemplate<T> {
    fn into_response(self) -> Response {
        if crate::util::errors::is_hx_request() {
            if let Some(render) = self.partial {
                return match render() {
                    Ok(html) => Html(html).into_response(),
                    Err(err) => template_render_error("partial template", err),
                };
            }
        }

        match self.full.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => template_render_error("full template", err),
        }
    }
}

fn template_render_error(description: &str, err: askama::Error) -> Response {
    let mut response = (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Failed to render {}: {}", description, err),
    )
        .into_response();

    if crate::util::errors::is_hx_request() {
        response
            .headers_mut()
            .insert("HX-Reswap", HeaderValue::from_static("none"));
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_template_renders_with_page_context() {
        let template = ServerTimeTemplate {
            ctx: PageContext {
                csrf_token: "test-csrf".into(),
                csp_nonce: "test-nonce".into(),
            },
            time: "now".to_string(),
        };
        let response = HtmlTemplate::new(template).into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn test_html_template_with_partial_renders_full_by_default() {
        let full = ServerTimeTemplate {
            ctx: PageContext {
                csrf_token: "test-csrf".into(),
                csp_nonce: "test-nonce".into(),
            },
            time: "now".to_string(),
        };
        let partial = ServerTimePartialTemplate {
            ctx: PageContext {
                csrf_token: "test-csrf".into(),
                csp_nonce: "test-nonce".into(),
            },
            time: "now".to_string(),
        };
        let response = HtmlTemplate::new(full)
            .with_partial(partial)
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
