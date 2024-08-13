use std::str::FromStr;
use std::sync::Arc;

use alloy::network::EthereumWallet;
use alloy::providers::fillers::{
    ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller, WalletFiller,
};
use alloy::providers::network::AnyNetwork;
use alloy::providers::{Identity, RootProvider};
use alloy::providers::{Provider, ReqwestProvider};
use alloy::signers::local::PrivateKeySigner;
use alloy::transports::http::{Client, Http};
use derive_getters::Getters;

use crate::config::AppConfig;
use crate::contracts::DisperseCollectContract;

pub type AppNetwork = AnyNetwork;

pub type DefaultProvider = FillProvider<
    JoinFill<
        JoinFill<JoinFill<JoinFill<Identity, GasFiller>, NonceFiller>, ChainIdFiller>,
        WalletFiller<EthereumWallet>,
    >,
    RootProvider<Http<Client>, AppNetwork>,
    Http<Client>,
    AppNetwork,
>;

#[derive(Clone, Getters)]
pub struct AppState {
    provider: DefaultProvider,
    contract: DisperseCollectContract,
}

impl AppState {
    pub fn init(config: AppConfig) -> anyhow::Result<Arc<Self>> {
        let signer = PrivateKeySigner::from_str(&config.tx_signer)?;
        let wallet = EthereumWallet::new(signer);
        let provider = ReqwestProvider::<AnyNetwork>::builder()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(config.rpc_url);
        let contract = DisperseCollectContract::new(config.contract_address, provider.clone());

        Ok(Self { provider, contract }.into())
    }
}
