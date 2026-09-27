//! Request format detection middleware
//!
//! Scopes the `REQUEST_FORMAT` task-local for the duration of each request so
//! that `IntoResponse` and `AppError::response` implementations — which cannot
//! access the request — can negotiate HTML, HTMX-partial, or JSON bodies.
//!
//! This lives in its own middleware rather than inside `error_handler` so that
//! reordering or removing the error handler cannot silently break HTMX
//! partial rendering and error content-negotiation.

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

use crate::util::errors::{with_request_format, RequestFormat};

/// Derives the preferred response format from the request headers and runs the
/// rest of the stack inside the `REQUEST_FORMAT` task-local scope.
pub async fn middleware(req: Request, next: Next) -> Response {
    let format = RequestFormat::from_headers(req.headers());
    with_request_format(format, next.run(req)).await
}
