#[cfg(feature = "metrics")]
use axum::extract::{Query, State};
#[cfg(feature = "metrics")]
use axum::http::{header, HeaderMap, StatusCode};
#[cfg(feature = "metrics")]
use axum::response::{IntoResponse, Response};
#[cfg(feature = "metrics")]
use prometheus::{Encoder, Histogram, IntCounter, IntCounterVec, IntGauge, Registry};
#[cfg(feature = "metrics")]
use secrecy::ExposeSecret;
#[cfg(feature = "metrics")]
use serde::Deserialize;

#[cfg(feature = "metrics")]
#[derive(Debug, Default, Deserialize)]
pub struct MetricsQuery {
    /// The kind of metrics to return: `instance` (default) or `service`.
    pub kind: Option<String>,
}

#[cfg(feature = "metrics")]
pub struct InstanceMetrics {
    pub registry: Registry,
    pub requests_total: IntCounter,
    pub requests_in_flight: IntGauge,
    pub response_times: prometheus::HistogramVec,
    pub responses_by_status_code_total: IntCounterVec,

    // Database pool metrics
    pub db_pool_connections_total: IntGauge,
    pub db_pool_connections_idle: IntGauge,
    pub db_pool_wait_time: Histogram,
    pub db_pool_timeouts_total: IntCounter,
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

        let response_times = prometheus::HistogramVec::new(
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

        let db_pool_connections_total = IntGauge::with_opts(prometheus::Opts::new(
            "db_pool_connections_total",
            "Total number of connections in the database pool",
        ))
        .unwrap();
        registry
            .register(Box::new(db_pool_connections_total.clone()))
            .unwrap();

        let db_pool_connections_idle = IntGauge::with_opts(prometheus::Opts::new(
            "db_pool_connections_idle",
            "Number of idle connections in the database pool",
        ))
        .unwrap();
        registry
            .register(Box::new(db_pool_connections_idle.clone()))
            .unwrap();

        let db_pool_wait_time = Histogram::with_opts(prometheus::HistogramOpts::new(
            "db_pool_wait_time_seconds",
            "Time spent waiting for a database connection from the pool",
        ))
        .unwrap();
        registry
            .register(Box::new(db_pool_wait_time.clone()))
            .unwrap();

        let db_pool_timeouts_total = IntCounter::with_opts(prometheus::Opts::new(
            "db_pool_timeouts_total",
            "Total number of database connection pool timeouts",
        ))
        .unwrap();
        registry
            .register(Box::new(db_pool_timeouts_total.clone()))
            .unwrap();

        Self {
            registry,
            requests_total,
            requests_in_flight,
            response_times,
            responses_by_status_code_total,
            db_pool_connections_total,
            db_pool_connections_idle,
            db_pool_wait_time,
            db_pool_timeouts_total,
        }
    }

    pub fn gather(&self) -> prometheus::Result<Vec<prometheus::proto::MetricFamily>> {
        Ok(self.registry.gather())
    }

    /// Update database pool metrics by sampling the current pool status and
    /// timing how long it takes to acquire a connection.
    pub async fn update_db_pool_metrics(&self, database: &crate::db::Database) {
        let db = database.db();

        let status = db.pool().status();
        self.db_pool_connections_total.set(status.size as i64);
        self.db_pool_connections_idle.set(status.available as i64);

        let start = std::time::Instant::now();
        match db.connection().await {
            Ok(conn) => {
                let elapsed = start.elapsed();
                self.db_pool_wait_time.observe(elapsed.as_secs_f64());
                drop(conn);
            }
            Err(err) => {
                if err.to_string().contains("Timeout occurred while waiting") {
                    self.db_pool_timeouts_total.inc();
                }
                tracing::debug!("Failed to acquire database connection for metrics: {}", err);
            }
        }
    }
}

#[cfg(feature = "metrics")]
impl Default for InstanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "metrics")]
fn unauthorized() -> Response {
    StatusCode::UNAUTHORIZED.into_response()
}

#[cfg(feature = "metrics")]
fn bearer_token_matches(expected: &secrecy::SecretString, provided: &str) -> bool {
    use subtle::ConstantTimeEq;
    let expected = expected.expose_secret().as_bytes();
    let provided = provided.as_bytes();
    if expected.len() != provided.len() {
        return false;
    }
    expected.ct_eq(provided).into()
}

#[cfg(feature = "metrics")]
/// Prometheus metrics endpoint handler
pub async fn metrics_handler(
    State(state): State<crate::app::AppState>,
    Query(query): Query<MetricsQuery>,
    headers: HeaderMap,
) -> Response {
    // Enforce the optional metrics bearer token, if one is configured.
    if let Some(expected_token) = &state.config.metrics_token {
        let Some(auth_header) = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
        else {
            return unauthorized();
        };

        let Some((scheme, token)) = auth_header.split_once(' ') else {
            return unauthorized();
        };

        if !scheme.eq_ignore_ascii_case("Bearer") {
            return unauthorized();
        }

        if !bearer_token_matches(expected_token, token.trim()) {
            return unauthorized();
        }
    }

    let kind = query.kind.as_deref().unwrap_or("instance");

    match kind {
        "instance" => {
            let metrics = &state.0.metrics;
            metrics.update_db_pool_metrics(&state.0.database).await;

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
        "service" => StatusCode::NOT_IMPLEMENTED.into_response(),
        _ => (StatusCode::BAD_REQUEST, "Invalid `kind` parameter").into_response(),
    }
}
