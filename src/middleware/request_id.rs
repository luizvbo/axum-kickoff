//! Request ID middleware
//!
//! Generates a unique request ID for each request, includes it in tracing spans,
//! and adds it to the response headers as `X-Request-ID`.

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use http::HeaderValue;
use tracing::Instrument;

const REQUEST_ID_HEADER: &str = "x-request-id";

/// Request ID extracted from request extensions
#[derive(Clone, Debug)]
pub struct RequestId(pub String);

/// Middleware that generates a request ID, adds it to extensions and tracing span,
/// and includes it in the response headers.
pub async fn middleware(mut req: Request, next: Next) -> Response {
    let request_id = req
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    req.extensions_mut().insert(RequestId(request_id.clone()));

    let span = tracing::info_span!("request", id = %request_id);

    let mut response = async { next.run(req).await }.instrument(span).await;

    if let Ok(header_value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert(REQUEST_ID_HEADER, header_value);
    }

    response
}
