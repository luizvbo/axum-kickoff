use axum_kickoff::{build_handler, App};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal::unix::{signal, SignalKind};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const CORE_THREADS: usize = 4;

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration from environment
    let config = axum_kickoff::config::Server::from_environment()?;

    // Load database configuration
    let db_config = axum_kickoff::config::DatabaseConfig::from_environment()?;

    // Configure tokio runtime
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    builder.worker_threads(CORE_THREADS);
    if let Some(threads) = config.max_blocking_threads {
        builder.max_blocking_threads(threads);
    }

    let rt = builder.build()?;

    // Block the main thread until the server has shutdown
    rt.block_on(async move {
        // Initialize database connection
        info!("Connecting to database...");
        let database = axum_kickoff::db::Database::from_config(&db_config).await?;
        info!("Database connected successfully");

        // Create the application instance
        let app = App::new(config, database);
        let app = Arc::new(app);

        // Build the axum router with middleware
        let axum_router = build_handler(app.clone());

        let make_service = axum_router.into_make_service_with_connect_info::<SocketAddr>();

        // Create a `TcpListener` using tokio
        let listener = TcpListener::bind((app.config.ip, app.config.port))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to bind to address: {}", e))?;

        let addr = listener
            .local_addr()
            .map_err(|e| anyhow::anyhow!("Failed to get local address: {}", e))?;

        info!("Listening at http://{}", addr);

        // Run the server with graceful shutdown
        axum::serve(listener, make_service)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|e| anyhow::anyhow!("Server error: {}", e))
    })?;

    info!("Server has gracefully shutdown!");
    Ok(())
}

async fn shutdown_signal() {
    let interrupt = async {
        signal(SignalKind::interrupt())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    let terminate = async {
        signal(SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = interrupt => {},
        _ = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    // Note: Server binary tests are typically integration tests
    // Unit tests for binary entry points are limited
    // Consider adding integration tests in tests/ directory

    #[test]
    fn test_server_config_from_environment_compiles() {
        // Verify that Server::from_environment compiles
        // This is a compile-time check
        let _ = || axum_kickoff::config::Server::from_environment;
    }

    #[test]
    fn test_database_config_from_environment_compiles() {
        // Verify that DatabaseConfig::from_environment compiles
        // This is a compile-time check
        let _ = || axum_kickoff::config::DatabaseConfig::from_environment;
    }

    #[test]
    fn test_app_new_compiles() {
        // Verify that App::new compiles
        // This is a compile-time check
        let _ = || axum_kickoff::App::new;
    }

    #[test]
    fn test_build_handler_compiles() {
        // Verify that build_handler compiles
        // This is a compile-time check
        let _ = || axum_kickoff::build_handler;
    }
}
