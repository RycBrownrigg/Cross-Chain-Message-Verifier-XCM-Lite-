pub mod api;
pub mod config;
pub mod crypto;
pub mod domain;
pub mod execution;
pub mod processor;
pub mod state;

use std::sync::Arc;

use api::{router as build_router, ApiContext};
use config::AppConfig;
use crypto::KeyRegistry;
use execution::{DefaultExecutionEngine, ExecutionEngine};
use processor::{run_relay_loop, MessageProcessor};
use state::ServiceState;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error(transparent)]
    Config(#[from] config::ConfigError),
    #[error(transparent)]
    State(#[from] state::StateInitError),
    #[error(transparent)]
    Crypto(#[from] crypto::CryptoError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Server(#[from] hyper::Error),
}

pub async fn run() -> Result<(), ServiceError> {
    let config = Arc::new(AppConfig::load()?);
    let state = ServiceState::initialize(&config.parachains)?;
    let key_registry = KeyRegistry::from_config(&config.parachains)?;
    let (processor, relay_rx) = MessageProcessor::new(
        state.clone(),
        key_registry.clone(),
        &config.parachains.xcm_version,
    );
    let execution_engine: Arc<dyn ExecutionEngine> =
        Arc::new(DefaultExecutionEngine::new(state.clone()));

    tracing::info!(
        target: "xcm_lite",
        host = %config.server.host,
        port = config.server.port,
        parachains = state.parachain_count(),
        xcm_version = %config.parachains.xcm_version,
        keys = key_registry.len(),
        "configuration and state initialised"
    );

    tokio::spawn(run_relay_loop(
        state.clone(),
        execution_engine.clone(),
        relay_rx,
    ));

    let app = build_router(ApiContext {
        config: config.clone(),
        state: state.clone(),
        processor: processor.clone(),
    });

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(target: "xcm_lite", %addr, "HTTP server listening");

    axum::serve(listener, app).await?;

    Ok(())
}
