use disperse_collect_api::AppConfig;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let _ = dotenvy::dotenv();

    let config = AppConfig::load()?;

    disperse_collect_api::run(config).await?.await?;

    Ok(())
}
