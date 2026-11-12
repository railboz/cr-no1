//! Binary entry point: configuration, then a graceful HTTP server.

use std::net::SocketAddr;
use std::process::ExitCode;

use anyhow::Context;
use creno_relay::config::Config;
use creno_relay::{AppState, app, obs};
use tokio::net::TcpListener;
use tokio::signal;

#[tokio::main]
async fn main() -> ExitCode {
    obs::init();

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("creno-relay: invalid configuration: {error}");
            return ExitCode::from(2);
        }
    };

    match run(config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("creno-relay: {error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run(config: Config) -> anyhow::Result<()> {
    let bind = config.bind;
    let state = AppState::new(config);

    let listener = TcpListener::bind(bind)
        .await
        .with_context(|| format!("cannot bind {bind}"))?;
    let local = listener.local_addr().unwrap_or(bind);
    tracing::info!(target: obs::TARGET, address = %local, "relay listening");

    axum::serve(
        listener,
        app(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("server error")?;

    tracing::info!(target: obs::TARGET, "relay stopped");
    Ok(())
}

/// Resolves on SIGINT or SIGTERM; never logs the signal payload.
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = signal::ctrl_c().await {
            tracing::warn!(target: obs::TARGET, %error, "cannot listen for SIGINT");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(error) => {
                tracing::warn!(target: obs::TARGET, %error, "cannot listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => tracing::info!(target: obs::TARGET, "shutdown requested"),
        () = terminate => tracing::info!(target: obs::TARGET, "shutdown requested"),
    }
}
