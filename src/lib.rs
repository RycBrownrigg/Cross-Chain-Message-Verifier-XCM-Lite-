pub mod config;
pub mod crypto;
pub mod domain;
pub mod state;

use config::AppConfig;
use crypto::KeyRegistry;
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
}

pub async fn run() -> Result<(), ServiceError> {
    let config = AppConfig::load()?;
    let state = ServiceState::initialize(&config.parachains)?;
    let key_registry = KeyRegistry::from_config(&config.parachains)?;

    tracing::info!(
        target: "xcm_lite",
        host = %config.server.host,
        port = config.server.port,
        parachains = state.parachain_count(),
        xcm_version = %config.parachains.xcm_version,
        keys = key_registry.len(),
        "configuration and state initialised"
    );

    // TODO: continue wiring subsystems before starting HTTP server

    Ok(())
}
