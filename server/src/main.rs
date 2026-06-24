use crate::config::Config;
use axum::Router;
use std::net::SocketAddr;
use tower_http::services::ServeDir;
use tracing::info;
use tracing_subscriber::EnvFilter;

mod config;
mod error;
mod state;

#[tokio::main]
async fn main() {
    init_logging();
    info!("Starting server...");

    let config = Config::from_env().unwrap();
    let state = state::ServerState::new(config);
    let addr = SocketAddr::from(([0, 0, 0, 0], state.config.port));

    let router = Router::new()
        .fallback_service(ServeDir::new(state.config.static_dir.clone()))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    info!("Listening on {}", addr);

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutting down...");
}

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
}
