mod comment;
mod config;
mod error;
mod github;
mod merge;
mod webhook;

use std::sync::Arc;

use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::Router;
use sem_core::parser::plugins::create_default_registry;
use sem_core::parser::registry::ParserRegistry;

use crate::config::Config;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub registry: Arc<ParserRegistry>,
    /// What a merge run by this server may reach outside itself. Built once,
    /// here, and copied into every merge task.
    pub host: weave_core::host::Host,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("weave_github=info".parse().unwrap()),
        )
        .init();

    let config = Config::from_env().expect("invalid configuration");
    let registry = Arc::new(create_default_registry());

    let state = AppState {
        config: Arc::clone(&config),
        registry,
        host: weave_core::host::Host {
            line_merge: Some(weave_core::host::git_line_merge),
            ..Default::default()
        },
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/webhook", post(webhook::handle_webhook))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", config.port);
    tracing::info!("listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> StatusCode {
    StatusCode::OK
}
