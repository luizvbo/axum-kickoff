use axum::extract::Request;
#[cfg(feature = "metrics")]
use axum::extract::{MatchedPath, State};
use axum::middleware::Next;
use axum::response::Response;

#[cfg(feature = "metrics")]
use crate::app::AppState;

/// Decrements `requests_in_flight` on drop so client disconnects and
/// `TimeoutLayer` cancellations can't leak the gauge upward permanently.
#[cfg(feature = "metrics")]
struct InFlightGuard(prometheus::IntGauge);

#[cfg(feature = "metrics")]
impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.0.dec();
    }
}

#[cfg(feature = "metrics")]
pub async fn update_metrics(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let metrics = state.0.metrics.clone();

    metrics.requests_total.inc();
    metrics.requests_in_flight.inc();
    let _in_flight = InFlightGuard(metrics.requests_in_flight.clone());

    // Label by the matched route template (`/api/v1/posts/{id}`), never the
    // raw request path — per-entity URLs would give `endpoint` unbounded
    // cardinality. Requests that match no route (404 fallback, nested
    // services) share a single catch-all series.
    let endpoint = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "unmatched".to_string());

    let start = std::time::Instant::now();
    let response = next.run(req).await;
    let elapsed = start.elapsed().as_secs_f64();

    metrics
        .response_times
        .with_label_values(&[endpoint.as_str()])
        .observe(elapsed);
    metrics
        .responses_by_status_code_total
        .with_label_values(&[response.status().as_str()])
        .inc();

    response
}

#[cfg(not(feature = "metrics"))]
pub async fn update_metrics(req: Request, next: Next) -> Response {
    next.run(req).await
}
