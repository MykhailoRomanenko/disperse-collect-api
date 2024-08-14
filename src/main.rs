use disperse_collect_api::AppConfig;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(EnvFilter::from_env("RUST_LOG"))
        .with(fmt::layer().compact())
        .init();

    let config = AppConfig::load()?;

    disperse_collect_api::run(config).await?.await?;

    Ok(())
}
