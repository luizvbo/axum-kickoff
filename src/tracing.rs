//! Tracing and Sentry integration
//!
//! Provides a single `init_tracing` entry point that configures the global
//! `tracing` subscriber based on the current environment. When the `sentry`
//! feature is enabled, a `sentry-tracing` layer is attached so that
//! `tracing::error!` events are reported to Sentry.

use crate::config::LogFormat;
use crate::Env;

use tracing_subscriber::prelude::*;

/// Initialize the global tracing subscriber.
///
/// Development and test environments use a human-readable pretty formatter;
/// production uses JSON. The `RUST_LOG` environment variable is honored,
/// defaulting to `info`.
pub fn init_tracing(env: Env) {
    init_tracing_with_format(env, default_log_format(env));
}

fn default_log_format(env: Env) -> LogFormat {
    match env {
        Env::Production => LogFormat::Json,
        Env::Development | Env::Test => LogFormat::Pretty,
    }
}

#[cfg(not(feature = "sentry"))]
pub fn init_tracing_with_format(_env: Env, format: LogFormat) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    match format {
        LogFormat::Pretty => {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().pretty())
                .init();
        }
        LogFormat::Json => {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().json())
                .init();
        }
        LogFormat::Full => {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer())
                .init();
        }
        LogFormat::Compact => {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().compact())
                .init();
        }
    }
}

#[cfg(feature = "sentry")]
fn sentry_layer<S>() -> sentry_tracing::SentryLayer<S>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    use sentry_tracing::EventFilter;

    // Only capture ERROR-level tracing events as Sentry events. Other levels
    // are ignored to avoid creating noise in Sentry.
    sentry_tracing::layer().event_filter(|md| match *md.level() {
        tracing::Level::ERROR => EventFilter::Event,
        _ => EventFilter::Ignore,
    })
}

#[cfg(feature = "sentry")]
pub fn init_tracing_with_format(_env: Env, format: LogFormat) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    match format {
        LogFormat::Pretty => {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().pretty())
                .with(sentry_layer())
                .init();
        }
        LogFormat::Json => {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().json())
                .with(sentry_layer())
                .init();
        }
        LogFormat::Full => {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer())
                .with(sentry_layer())
                .init();
        }
        LogFormat::Compact => {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().compact())
                .with(sentry_layer())
                .init();
        }
    }
}

#[cfg(feature = "sentry")]
/// Initialize Sentry when a DSN is configured.
///
/// Returns a guard that must be kept alive for the Sentry client to remain
/// active. This function is a no-op (returning `None`) when `SENTRY_DSN` is
/// not set.
pub fn init_sentry(dsn: Option<&secrecy::SecretString>) -> Option<sentry::ClientInitGuard> {
    use secrecy::ExposeSecret;

    let dsn = dsn?;
    let parsed = dsn.expose_secret().parse::<sentry::types::Dsn>().ok()?;

    let mut options = sentry::ClientOptions::default();
    options.dsn = Some(parsed);

    Some(sentry::init(options))
}
