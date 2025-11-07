use std::error::Error;

pub async fn run() -> Result<(), Box<dyn Error>> {
    tracing::info!(target: "xcm_lite", "Starting Cross-Chain Message Verifier (XCM Lite) service");

    // TODO: initialize configuration, state, and HTTP server

    Ok(())
}
