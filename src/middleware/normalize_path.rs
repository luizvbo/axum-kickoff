//! Path normalization middleware
//!
//! Normalizes incoming request paths before they reach the router by:
//! - Collapsing multiple consecutive slashes into one
//! - Removing `.` segments
//! - Resolving `..` segments and rejecting paths that escape the root
//! - Trimming trailing slashes (except for the root path `/`)

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use http::Uri;
use std::str::FromStr;

use crate::util::errors::bad_request;

/// Middleware that normalizes the request URI path before it reaches the router.
pub async fn middleware(req: Request, next: Next) -> Response {
    match normalize_request_uri(req.uri()) {
        Ok(Some(new_uri)) => {
            let (mut parts, body) = req.into_parts();
            parts.uri = new_uri;
            next.run(Request::from_parts(parts, body)).await
        }
        Ok(None) => next.run(req).await,
        Err(_) => bad_request("Invalid path: path escapes root directory").into_response(),
    }
}

/// Normalize a request URI, returning `Some` if the path changed.
fn normalize_request_uri(uri: &Uri) -> Result<Option<Uri>, NormalizeError> {
    let original_path = uri.path();
    let normalized = normalize_path(original_path)?;
    if normalized == original_path {
        return Ok(None);
    }

    let path_and_query = if let Some(query) = uri.query() {
        format!("{}?{}", normalized, query)
    } else {
        normalized
    };

    let path_and_query = http::uri::PathAndQuery::from_str(&path_and_query)
        .map_err(|_| NormalizeError::InvalidUri)?;

    let mut parts = uri.clone().into_parts();
    parts.path_and_query = Some(path_and_query);
    Uri::from_parts(parts)
        .map_err(|_| NormalizeError::InvalidUri)
        .map(Some)
}

/// Normalize an absolute path.
///
/// Returns the normalized path or an error if the path attempts to escape the
/// root directory.
pub(crate) fn normalize_path(path: &str) -> Result<String, NormalizeError> {
    let mut stack: Vec<&str> = Vec::new();

    for segment in path.split('/') {
        match segment {
            "" | "." => continue,
            ".." => {
                if stack.pop().is_none() {
                    return Err(NormalizeError::PathEscapesRoot);
                }
            }
            _ => stack.push(segment),
        }
    }

    if stack.is_empty() {
        Ok("/".to_string())
    } else {
        let mut normalized = String::with_capacity(path.len());
        for segment in &stack {
            normalized.push('/');
            normalized.push_str(segment);
        }
        Ok(normalized)
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum NormalizeError {
    PathEscapesRoot,
    InvalidUri,
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::StatusCode;

    #[test]
    fn test_collapse_multiple_slashes() {
        assert_eq!(normalize_path("/foo//bar").unwrap(), "/foo/bar");
        assert_eq!(normalize_path("/foo///bar").unwrap(), "/foo/bar");
        assert_eq!(normalize_path("//foo//bar//").unwrap(), "/foo/bar");
    }

    #[test]
    fn test_remove_dot_segments() {
        assert_eq!(normalize_path("/foo/./bar").unwrap(), "/foo/bar");
        assert_eq!(normalize_path("/foo/././bar").unwrap(), "/foo/bar");
        assert_eq!(normalize_path("/./foo").unwrap(), "/foo");
    }

    #[test]
    fn test_resolve_dotdot_segments() {
        assert_eq!(normalize_path("/foo/../bar").unwrap(), "/bar");
        assert_eq!(normalize_path("/foo/bar/../baz").unwrap(), "/foo/baz");
        assert_eq!(normalize_path("/foo/bar/../..").unwrap(), "/");
    }

    #[test]
    fn test_reject_path_escaping_root() {
        assert!(matches!(
            normalize_path("/foo/../../etc/passwd"),
            Err(NormalizeError::PathEscapesRoot)
        ));
        assert!(matches!(
            normalize_path("/../foo"),
            Err(NormalizeError::PathEscapesRoot)
        ));
        assert!(matches!(
            normalize_path("/.."),
            Err(NormalizeError::PathEscapesRoot)
        ));
    }

    #[test]
    fn test_trim_trailing_slash() {
        assert_eq!(normalize_path("/foo/").unwrap(), "/foo");
        assert_eq!(normalize_path("/foo/bar/").unwrap(), "/foo/bar");
        assert_eq!(normalize_path("/").unwrap(), "/");
        assert_eq!(normalize_path("//").unwrap(), "/");
    }

    #[test]
    fn test_preserve_query_string() {
        let uri: Uri = "/foo//bar?baz=qux".parse().unwrap();
        let new_uri = normalize_request_uri(&uri).unwrap().unwrap();
        assert_eq!(new_uri.path(), "/foo/bar");
        assert_eq!(new_uri.query(), Some("baz=qux"));
    }

    #[test]
    fn test_preserve_query_string_with_multiple_slashes() {
        let uri: Uri = "/foo//.//bar/?baz=qux&quux=corge".parse().unwrap();
        let new_uri = normalize_request_uri(&uri).unwrap().unwrap();
        assert_eq!(new_uri.path(), "/foo/bar");
        assert_eq!(new_uri.query(), Some("baz=qux&quux=corge"));
    }

    #[test]
    fn test_no_change_for_normalized_path() {
        let uri: Uri = "/foo/bar".parse().unwrap();
        assert_eq!(normalize_request_uri(&uri).unwrap(), None);
    }

    #[test]
    fn test_path_escaping_root_request() {
        let uri: Uri = "/foo/../../etc/passwd".parse().unwrap();
        assert!(matches!(
            normalize_request_uri(&uri),
            Err(NormalizeError::PathEscapesRoot)
        ));
    }

    #[tokio::test]
    async fn test_middleware_normalizes_request_path() {
        use axum::body::Body;
        use axum::http::Request;
        use axum::routing::get;
        use tower::{Layer, ServiceExt};

        let app = axum::middleware::from_fn(middleware)
            .layer(axum::Router::new().route("/foo/bar", get(|| async { "ok" })));

        let request = Request::builder()
            .uri("/foo//bar")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_middleware_resolves_dotdot_segments() {
        use axum::body::Body;
        use axum::http::Request;
        use axum::routing::get;
        use tower::{Layer, ServiceExt};

        let app = axum::middleware::from_fn(middleware)
            .layer(axum::Router::new().route("/bar", get(|| async { "ok" })));

        let request = Request::builder()
            .uri("/foo/../bar")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_middleware_rejects_path_escaping_root() {
        use axum::body::Body;
        use axum::http::Request;
        use axum::routing::get;
        use tower::{Layer, ServiceExt};

        let app = axum::middleware::from_fn(middleware)
            .layer(axum::Router::new().route("/etc/passwd", get(|| async { "ok" })));

        let request = Request::builder()
            .uri("/foo/../../etc/passwd")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_middleware_trims_trailing_slash() {
        use axum::body::Body;
        use axum::http::Request;
        use axum::routing::get;
        use tower::{Layer, ServiceExt};

        let app = axum::middleware::from_fn(middleware)
            .layer(axum::Router::new().route("/foo", get(|| async { "ok" })));

        let request = Request::builder().uri("/foo/").body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_middleware_preserves_root_path() {
        use axum::body::Body;
        use axum::http::Request;
        use axum::routing::get;
        use tower::{Layer, ServiceExt};

        let app = axum::middleware::from_fn(middleware)
            .layer(axum::Router::new().route("/", get(|| async { "ok" })));

        let request = Request::builder().uri("//").body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
