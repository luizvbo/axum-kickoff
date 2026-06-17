use askama::Template;
use axum::extract::Extension;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use chrono::Utc;
use http::{Method, StatusCode};
use tower_http::services::ServeDir;

use crate::app::AppState;
use crate::controllers::auth::{github_authorize, github_callback, logout_api, logout_html};
use crate::controllers::token::{create_token, list_tokens, revoke_token};
use crate::middleware::{get_or_create_csrf_token, protect, SessionExtension};
use crate::Env;

pub fn build_axum_router(state: AppState) -> Router<()> {
    let mut router = Router::new()
        .route("/", get(home))
        .route("/health", get(health_check))
        .route("/api/server-time", get(server_time))
        .route("/api/v1/auth/github/authorize", get(github_authorize))
        .route("/api/v1/auth/github/callback", get(github_callback))
        .route("/api/v1/auth/logout", post(logout_api))
        .route("/logout", post(logout_html))
        .layer(axum::middleware::from_fn(protect))
        .route("/api/v1/tokens", post(create_token))
        .route("/api/v1/tokens", get(list_tokens))
        .route("/api/v1/tokens/{token_id}", post(revoke_token))
        .layer(axum::middleware::from_fn(protect))
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
mod tests {
    use super::*;

    #[test]
    fn test_index_template_fields() {
        let template = IndexTemplate {
            csrf_token: "test_token".to_string(),
        };
        // Just verify the struct can be created
        let _ = template;
    }

    #[test]
    fn test_server_time_template_fields() {
        let template = ServerTimeTemplate {
            time: "2024-01-01 00:00:00 UTC".to_string(),
        };
        assert_eq!(template.time, "2024-01-01 00:00:00 UTC");
    }

    #[test]
    fn test_html_template_creation() {
        let template = IndexTemplate {
            csrf_token: "test_token".to_string(),
        };
        let html_template = HtmlTemplate(template);
        let _ = html_template;
    }

    #[test]
    fn test_server_time_template_with_different_time() {
        let template = ServerTimeTemplate {
            time: "2024-12-31 23:59:59 UTC".to_string(),
        };
        assert_eq!(template.time, "2024-12-31 23:59:59 UTC");
    }

    #[test]
    fn test_server_time_template_empty_time() {
        let template = ServerTimeTemplate {
            time: "".to_string(),
        };
        assert_eq!(template.time, "");
    }

    #[test]
    fn test_server_time_template_with_timezone() {
        let template = ServerTimeTemplate {
            time: "2024-01-01 00:00:00 UTC".to_string(),
        };
        assert!(template.time.contains("UTC"));
    }

    #[test]
    fn test_server_time_template_with_milliseconds() {
        let template = ServerTimeTemplate {
            time: "2024-01-01 00:00:00.123 UTC".to_string(),
        };
        assert!(template.time.contains(".123"));
    }

    #[test]
    fn test_html_template_with_server_time() {
        let template = ServerTimeTemplate {
            time: "2024-12-31 23:59:59 UTC".to_string(),
        };
        let html_template = HtmlTemplate(template);
        let _ = html_template;
    }

    #[test]
    fn test_index_template_multiple() {
        let template1 = IndexTemplate {
            csrf_token: "test_token".to_string(),
        };
        let template2 = IndexTemplate {
            csrf_token: "test_token".to_string(),
        };
        let _ = (template1, template2);
    }

    #[test]
    fn test_server_time_template_unicode() {
        let template = ServerTimeTemplate {
            time: "2024-01-01 00:00:00 UTC 测试".to_string(),
        };
        assert!(template.time.contains("测试"));
    }

    #[test]
    fn test_server_time_template_very_long_time() {
        let long_time = "2024-01-01 00:00:00 UTC ".repeat(100);
        let template = ServerTimeTemplate {
            time: long_time.clone(),
        };
        assert_eq!(template.time.len(), long_time.len());
    }
}
