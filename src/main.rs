use xcm_lite::ServiceError;

#[tokio::main]
async fn main() -> Result<(), ServiceError> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    xcm_lite::run().await
}
