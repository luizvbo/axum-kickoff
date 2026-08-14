use axum::extract::Request;
#[cfg(feature = "metrics")]
use axum::extract::State;
use axum::middleware::Next;
use axum::response::Response;

#[cfg(feature = "metrics")]
use crate::app::AppState;

#[cfg(feature = "metrics")]
pub async fn update_metrics(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let metrics = state.0.metrics.clone();

    metrics.requests_total.inc();
    metrics.requests_in_flight.inc();

    let endpoint = req.uri().path().trim_end_matches('/').to_string();
    let endpoint = if endpoint.is_empty() { "/" } else { &endpoint };

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
