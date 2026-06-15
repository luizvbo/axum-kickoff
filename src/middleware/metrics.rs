use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

pub async fn update_metrics(req: Request, next: Next) -> Response {
    // Note: Metrics collection disabled for now due to state access issues
    // To enable, we need to refactor to use a different middleware pattern
    // or pass state through request extensions
    next.run(req).await
}
