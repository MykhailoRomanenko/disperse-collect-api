use alloy::{primitives::Address, transports::http::reqwest::Url};
use config::{Config, ConfigError, Environment};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct AppConfig {
    pub rpc_url: Url,
    pub contract_address: Address,
    pub tx_signer: String,
    pub port: u16,
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        Config::builder()
            .add_source(Environment::default())
            .build()?
            .try_deserialize()
    }
}
