#[cfg(feature = "metrics")]
use axum::extract::State;
#[cfg(feature = "metrics")]
use axum::http::{header, StatusCode};
#[cfg(feature = "metrics")]
use axum::response::{IntoResponse, Response};
#[cfg(feature = "metrics")]
use prometheus::{Encoder, HistogramVec, IntCounter, IntCounterVec, IntGauge, Registry};

#[cfg(feature = "metrics")]
pub struct InstanceMetrics {
    pub registry: Registry,
    pub requests_total: IntCounter,
    pub requests_in_flight: IntGauge,
    pub response_times: HistogramVec,
    pub responses_by_status_code_total: IntCounterVec,
}

#[cfg(feature = "metrics")]
impl InstanceMetrics {
    pub fn new() -> Self {
        let registry = Registry::new();

        let requests_total = IntCounter::with_opts(prometheus::Opts::new(
            "requests_total",
            "Total number of requests processed",
        ))
        .unwrap();
        registry.register(Box::new(requests_total.clone())).unwrap();

        let requests_in_flight = IntGauge::with_opts(prometheus::Opts::new(
            "requests_in_flight",
            "Number of requests currently being processed",
        ))
        .unwrap();
        registry
            .register(Box::new(requests_in_flight.clone()))
            .unwrap();

        let response_times = HistogramVec::new(
            prometheus::HistogramOpts::new("response_time_seconds", "Response times of endpoints"),
            &["endpoint"],
        )
        .unwrap();
        registry.register(Box::new(response_times.clone())).unwrap();

        let responses_by_status_code_total = IntCounterVec::new(
            prometheus::Opts::new(
                "responses_by_status_code_total",
                "Number of responses per status code",
            ),
            &["status"],
        )
        .unwrap();
        registry
            .register(Box::new(responses_by_status_code_total.clone()))
            .unwrap();

        Self {
            registry,
            requests_total,
            requests_in_flight,
            response_times,
            responses_by_status_code_total,
        }
    }

    pub fn gather(&self) -> prometheus::Result<Vec<prometheus::proto::MetricFamily>> {
        Ok(self.registry.gather())
    }
}

#[cfg(feature = "metrics")]
impl Default for InstanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "metrics")]
/// Prometheus `/metrics` endpoint handler
pub async fn metrics_handler(State(state): State<crate::app::AppState>) -> Response {
    let metrics = &state.0.metrics;
    let encoder = prometheus::TextEncoder::new();

    match metrics.gather() {
        Ok(metric_families) => match encoder.encode_to_string(&metric_families) {
            Ok(body) => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, encoder.format_type())],
                body,
            )
                .into_response(),
            Err(err) => {
                tracing::error!("Failed to encode metrics: {}", err);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        },
        Err(err) => {
            tracing::error!("Failed to gather metrics: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
