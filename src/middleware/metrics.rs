use axum::extract::Request;
#[cfg(feature = "metrics")]
use axum::extract::{MatchedPath, State};
use axum::middleware::Next;
use axum::response::Response;

#[cfg(feature = "metrics")]
use crate::app::AppState;

#[cfg(feature = "metrics")]
pub async fn update_metrics(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let metrics = state.0.metrics.clone();

    metrics.requests_total.inc();
    metrics.requests_in_flight.inc();

    // Label by the matched route template (`/api/v1/posts/{id}`), never the
    // raw request path — per-entity URLs would give `endpoint` unbounded
    // cardinality. Requests that match no route (404 fallback, nested
    // services) share a single catch-all series.
    let endpoint = req
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or("unmatched");

    let start = std::time::Instant::now();
    let response = next.run(req).await;
    let elapsed = start.elapsed().as_secs_f64();

    metrics
        .response_times
        .with_label_values(&[endpoint])
        .observe(elapsed);
    metrics
        .responses_by_status_code_total
        .with_label_values(&[response.status().as_str()])
        .inc();
    metrics.requests_in_flight.dec();

    response
}

#[cfg(not(feature = "metrics"))]
pub async fn update_metrics(req: Request, next: Next) -> Response {
    next.run(req).await
}
