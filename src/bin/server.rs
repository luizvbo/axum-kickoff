use axum_kickoff::{App, build_handler};
use std::sync::Arc;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
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

    // Create the application instance
    let app = App::new(config);

    let app = Arc::new(app);

    // Build the axum router with middleware
    let axum_router = build_handler(app.clone());

    // Configure tokio runtime
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    builder.worker_threads(CORE_THREADS);
    if let Some(threads) = app.config.max_blocking_threads {
        builder.max_blocking_threads(threads);
    }

    let rt = builder.build()?;

    let make_service = axum_router.into_make_service_with_connect_info::<SocketAddr>();

    // Block the main thread until the server has shutdown
    rt.block_on(async {
        // Create a `TcpListener` using tokio
        let listener = TcpListener::bind((app.config.ip, app.config.port)).await?;

        let addr = listener.local_addr()?;

        info!("Listening at http://{}", addr);

        // Run the server with graceful shutdown
        axum::serve(listener, make_service)
            .with_graceful_shutdown(shutdown_signal())
            .await
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
