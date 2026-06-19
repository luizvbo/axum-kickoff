use askama::Template;
use axum::extract::Extension;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::Router;
use chrono::Utc;
use http::{Method, StatusCode};
use tower_http::services::ServeDir;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::app::AppState;
use crate::controllers::auth::{github_authorize, github_callback, logout_api, logout_html};
use crate::controllers::post::{
    create_post, delete_post, list_posts, publish_post, show_post, unpublish_post, update_post,
};
use crate::controllers::token::{create_token, list_tokens, revoke_token};
use crate::middleware::{
    csrf_protect, get_or_create_csrf_token, require_session_user, SessionExtension,
};
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
            crate::controllers::token::CreateTokenRequest,
            crate::controllers::token::CreateTokenResponse,
            crate::controllers::token::TokenListItem,
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

pub fn build_axum_router(state: AppState) -> Router<()> {
    // Public router - no authentication required
    let public_router = Router::new()
        .route("/", get(home))
        .route("/health", get(health_check))
        .route("/api/server-time", get(server_time))
        .route("/api/v1/auth/github/authorize", get(github_authorize))
        .route("/api/v1/auth/github/callback", get(github_callback));

    // Session + CSRF protected router - requires both session auth and CSRF token
    // All session-authenticated unsafe methods (POST, PUT, PATCH, DELETE) are CSRF-protected
    let session_csrf_router = Router::new()
        .route("/api/v1/auth/logout", post(logout_api))
        .route("/logout", post(logout_html))
        .route("/api/v1/tokens", post(create_token))
        .route("/api/v1/tokens", get(list_tokens))
        .route("/api/v1/tokens/{token_id}", post(revoke_token))
        // Post CRUD routes
        .route("/api/v1/posts", get(list_posts))
        .route("/api/v1/posts", post(create_post))
        .route("/api/v1/posts/{id}", get(show_post))
        .route("/api/v1/posts/{id}", patch(update_post))
        .route("/api/v1/posts/{id}", delete(delete_post))
        .route("/api/v1/posts/{id}/publish", post(publish_post))
        .route("/api/v1/posts/{id}/unpublish", post(unpublish_post))
        .route_layer(axum::middleware::from_fn(csrf_protect))
        .route_layer(axum::middleware::from_fn(require_session_user));

    // Combine all stateful routes
    let api_router = Router::new()
        .merge(public_router)
        .merge(session_csrf_router)
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

async fn home(Extension(session): Extension<SessionExtension>) -> impl IntoResponse {
    let csrf_token = get_or_create_csrf_token(&session);
    let template = IndexTemplate { csrf_token };
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
struct IndexTemplate {
    #[allow(dead_code)]
    csrf_token: String,
}

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

#[cfg(test)]
mod tests {}
