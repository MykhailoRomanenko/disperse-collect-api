use alloy::sol;
use alloy::transports::http::{Client, Http};
use DisperseCollect::DisperseCollectInstance;
use IERC20::IERC20Instance;

use crate::state::{AppNetwork, DefaultProvider};

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    IERC20,
    "abi/IERC20.json"
);

pub type Erc20Contract = IERC20Instance<Http<Client>, DefaultProvider, AppNetwork>;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    DisperseCollect,
    "abi/DisperseCollect.json"
);

pub type DisperseCollectContract =
    DisperseCollectInstance<Http<Client>, DefaultProvider, AppNetwork>;
