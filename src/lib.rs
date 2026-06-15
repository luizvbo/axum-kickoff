//! This crate provides a kickstarter template for web applications using Axum.
//!
//! It follows best practices from the crates.io backend implementation.

pub use crate::app::App;
use std::sync::Arc;

use crate::app::AppState;
use crate::router::build_axum_router;
use tikv_jemallocator::Jemalloc;

#[global_allocator]
static ALLOC: Jemalloc = Jemalloc;

pub mod app;
pub mod config;
pub mod controllers;
pub mod db;
pub mod middleware;
#[cfg(feature = "metrics")]
pub mod metrics;
pub mod models;
pub mod rate_limiter;
mod router;
pub mod util;

#[cfg(test)]
pub mod tests;

/// Used for setting different values depending on whether the app is being run in production,
/// in development, or for testing.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Env {
    Development,
    Test,
    Production,
}

/// Configures routes, sessions, logging, and other middleware.
///
/// Called from the binary entry point (e.g., src/bin/server.rs).
pub fn build_handler(app: Arc<App>) -> axum::Router {
    let state = AppState(app);

    let axum_router = build_axum_router(state.clone());
    middleware::apply_axum_middleware(state, axum_router)
}
