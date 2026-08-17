use anyhow::Result;
use axum_kickoff::{build_handler, App};
use clap::{Parser, Subcommand};
use secrecy::ExposeSecret;
use std::net::SocketAddr;
use std::sync::Arc;
use toasty_cli::{Config as ToastyConfig, ToastyCli};
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const CORE_THREADS: usize = 4;

#[derive(Parser)]
#[command(name = "axum-kickoff")]
#[command(about = "axum-kickoff - web server, background worker, and migrations")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the HTTP server
    Server,
    /// Run the background worker (placeholder)
    BackgroundWorker,
    /// Run database migrations
    Migrate {
        #[arg(
            value_name = "ARGS",
            trailing_var_arg = true,
            allow_hyphen_values = true,
            num_args = 0..
        )]
        args: Vec<String>,
    },
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Server => {
            let config = axum_kickoff::config::Server::from_environment()?;

            let mut builder = tokio::runtime::Builder::new_multi_thread();
            builder.enable_all();
            builder.worker_threads(CORE_THREADS);
            if let Some(threads) = config.max_blocking_threads {
                builder.max_blocking_threads(threads);
            }

            let rt = builder.build()?;
            rt.block_on(run_server(config))?;
        }
        Command::BackgroundWorker => {
            println!("background-worker placeholder");
        }
        Command::Migrate { args } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(run_migrate(args))?;
        }
    }

    Ok(())
}

async fn run_server(config: axum_kickoff::config::Server) -> Result<()> {
    // Load database configuration
    let db_config = axum_kickoff::config::DatabaseConfig::from_environment()?;

    // Initialize database connection
    info!("Connecting to database...");
    let database = axum_kickoff::db::Database::from_config(&db_config).await?;
    info!("Database connected successfully");

    // Create the application instance
    let app = App::new(config, database)?;
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
        .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;

    info!("Server has gracefully shutdown!");
    Ok(())
}

async fn run_migrate(args: Vec<String>) -> Result<()> {
    let config = ToastyConfig::load()?;

    // Load database configuration from environment
    let db_config = axum_kickoff::config::DatabaseConfig::from_environment()?;

    let db = toasty::Db::builder()
        .models(toasty::models!(axum_kickoff::*))
        .connect(db_config.connect_url()?.expose_secret())
        .await?;

    // Default `cargo run -- migrate` to `migration apply`
    let toasty_args = if args.is_empty() {
        vec!["migration".to_string(), "apply".to_string()]
    } else {
        args
    };

    // The Toasty CLI expects the first argument to be the binary name, so
    // prepend a placeholder and pass the remaining arguments through.
    let args = std::iter::once("toasty".to_string()).chain(toasty_args);

    ToastyCli::with_config(db, config).parse_from(args).await?;

    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let interrupt = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                .expect("failed to install signal handler")
                .recv()
                .await;
        };

        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
        };

        tokio::select! {
            _ = interrupt => {},
            _ = terminate => {},
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_server_config_from_environment_compiles() {
        let _ = || axum_kickoff::config::Server::from_environment;
    }

    #[test]
    fn test_database_config_from_environment_compiles() {
        let _ = || axum_kickoff::config::DatabaseConfig::from_environment;
    }

    #[test]
    fn test_app_new_compiles() {
        let _ = || axum_kickoff::App::new;
    }

    #[test]
    fn test_build_handler_compiles() {
        let _ = || axum_kickoff::build_handler;
    }

    #[test]
    fn test_toasty_config_load_compiles() {
        let _ = || toasty_cli::Config::load;
    }

    #[test]
    fn test_tokio_main_attribute() {
        fn assert_async_main<T>() {}
        assert_async_main::<fn() -> anyhow::Result<()>>();
    }
}
