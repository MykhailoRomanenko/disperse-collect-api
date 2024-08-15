use std::{borrow::Cow, collections::BTreeMap, fmt::Display};

use alloy::primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisperseCollectResponse {
    #[serde(flatten)]
    pub tx: TransactionResponse,
    pub transfers: BTreeMap<Address, U256>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionResponse {
    pub tx_hash: B256,
}

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase", untagged)]
pub enum FractionOrAmount {
    Fraction(FractionalAmount),
    Amount { amount: U256 },
}

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct FractionalAmount {
    pub fraction: U256,
    #[serde(default = "default_units")]
    pub units: U256,
}

impl Display for FractionalAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.fraction, self.units)
    }
}

fn default_units() -> U256 {
    U256::from(100)
}

impl FractionalAmount {
    /// Calculates `fraction * total / units`
    pub fn to_absolute(self, total: U256) -> Option<U256> {
        total.checked_mul(self.fraction)?.checked_div(self.units)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectErc20Request {
    pub caller: Address,
    pub recipient: Address,
    pub token: Address,
    pub spenders: BTreeMap<Address, FractionOrAmount>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectErc20Response(pub DisperseCollectResponse);

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisperseEthRequest {
    pub recipients: BTreeMap<Address, FractionOrAmount>,
    pub caller: Address,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisperseEthResponse(pub DisperseCollectResponse);

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisperseErc20Request {
    pub recipients: BTreeMap<Address, FractionOrAmount>,
    pub token: Address,
    pub spender: Address,
    pub caller: Address,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisperseErc20Response(pub DisperseCollectResponse);

#[derive(Debug, Clone, Deserialize)]
pub struct TransferRequest {
    pub recipient: Address,
    pub value: FractionOrAmount,
    pub token: Option<Address>,
    pub caller: Address,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApproveRequest {
    pub spender: Address,
    pub amount: FractionOrAmount,
    pub token: Address,
    pub caller: Address,
}

#[derive(Serialize)]
pub struct ErrorResponse<'a> {
    pub error: Cow<'a, str>,
}

#[cfg(test)]
mod tests {
    use alloy::primitives::U256;

    use super::FractionalAmount;

    #[test]
    fn should_calculate_correct_amount() {
        let f = FractionalAmount {
            fraction: U256::from(110),
            units: U256::from(1000),
        };

        assert_eq!(U256::from(11), f.to_absolute(U256::from(100)).unwrap());
    }
}
