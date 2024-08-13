use std::collections::BTreeMap;

use alloy::{
    contract,
    primitives::{Address, U256},
    providers::{Provider, WalletProvider},
    transports::{RpcError, TransportErrorKind},
};
use futures::future::try_join_all;
use thiserror::Error;
use tokio::try_join;

use alloy::contract::Error as ContractError;

use crate::{
    contracts::{DisperseCollectContract, Erc20Contract},
    dto::{
        CollectErc20Request, CollectErc20Response, DisperseCollectResponse, DisperseErc20Request,
        DisperseErc20Response, DisperseEthRequest, DisperseEthResponse, FractionOrAmount,
        FractionalAmount,
    },
    state::DefaultProvider,
};

#[derive(Debug, Error)]
pub enum DcError {
    #[error(
        "insufficient funds for address {address}, required: {required}, available: {available}, check balance or allowance"
    )]
    InsufficientFunds {
        required: U256,
        available: U256,
        address: Address,
    },
    #[error("fraction {0} results in invalid amount")]
    InvalidFractionalAmount(FractionalAmount),
    #[error("erc20 not found at address: {0}")]
    TokenNotFound(Address),
    #[error("error communicating with node: {0}")]
    Transport(#[from] TransportErrorKind),
    #[error("unexpected error: {0}")]
    Unexpected(#[source] anyhow::Error),
}

impl DcError {
    pub fn unexpected(e: impl Into<anyhow::Error>) -> Self {
        Self::Unexpected(e.into())
    }

    pub fn from_erc20_err(e: contract::Error, token_address: Address) -> Self {
        match e {
            ContractError::UnknownFunction(_) | ContractError::UnknownSelector(_) => {
                Self::TokenNotFound(token_address)
            }
            ContractError::TransportError(e) => e.into(),
            e => Self::unexpected(e),
        }
    }
}

impl From<RpcError<TransportErrorKind>> for DcError {
    fn from(value: RpcError<TransportErrorKind>) -> Self {
        match value {
            RpcError::Transport(t) => Self::Transport(t),
            e => Self::unexpected(e),
        }
    }
}

pub async fn disperse_eth(
    provider: &DefaultProvider,
    contract: &DisperseCollectContract,
    request: DisperseEthRequest,
) -> Result<DisperseEthResponse, DcError> {
    let available_balance = provider
        .get_balance(provider.signer_addresses().next().unwrap())
        .await?;

    let (addresses, amounts) =
        get_disperse_args(available_balance, request.recipients.into_iter())?;

    let tx = contract
        .disperseEth(addresses.clone(), amounts.clone())
        .send()
        .await
        .map_err(DcError::unexpected)?
        .get_receipt()
        .await?;

    Ok(DisperseEthResponse(DisperseCollectResponse {
        tx_hash: tx.transaction_hash,
        transfers: BTreeMap::from_iter(addresses.into_iter().zip(amounts)),
    }))
}

pub async fn disperse_erc20(
    provider: &DefaultProvider,
    contract: &DisperseCollectContract,
    request: DisperseErc20Request,
) -> Result<DisperseErc20Response, DcError> {
    let token = Erc20Contract::new(request.token, provider.clone());

    let (balance, allowance) = try_join!(
        async {
            token
                .allowance(request.spender, *contract.address())
                .call()
                .await
        },
        async { token.balanceOf(request.spender).call().await }
    )
    .map(|(a, b)| (a._0, b._0))
    .map_err(|e: alloy::contract::Error| DcError::from_erc20_err(e, request.token))?;

    let available_balance = balance.min(allowance);

    let (addresses, amounts) = get_disperse_args(available_balance, request.recipients.into_iter())
        .map_err(|mut e| {
            if let DcError::InsufficientFunds { address, .. } = &mut e {
                *address = request.spender;
            }
            e
        })?;

    let tx = contract
        .disperseERC20(
            request.spender,
            request.token,
            addresses.clone(),
            amounts.clone(),
        )
        .send()
        .await
        .map_err(DcError::unexpected)?
        .get_receipt()
        .await?;

    Ok(DisperseErc20Response(DisperseCollectResponse {
        tx_hash: tx.transaction_hash,
        transfers: BTreeMap::from_iter(addresses.into_iter().zip(amounts)),
    }))
}

pub async fn collect_erc20(
    provider: &DefaultProvider,
    contract: &DisperseCollectContract,
    request: CollectErc20Request,
) -> Result<CollectErc20Response, DcError> {
    let token = Erc20Contract::new(request.token, provider.clone());

    let balances = try_join_all(request.spenders.keys().cloned().map(|owner| {
        let token = token.clone();
        async move {
            try_join!(
                // nested async blocks because part before .call() is borrowed
                async { token.allowance(owner, *contract.address()).call().await },
                async { token.balanceOf(owner).call().await }
            )
        }
    }))
    .await
    .map_err(|e| DcError::from_erc20_err(e, request.token))?
    .into_iter()
    .map(|(a, b)| (a._0, b._0));

    let mut addresses = Vec::with_capacity(request.spenders.len());
    let mut amounts = Vec::with_capacity(request.spenders.len());

    for ((allowance, balance), (address, amount)) in balances.zip(request.spenders.into_iter()) {
        let actual_amount = match amount {
            FractionOrAmount::Amount { amount } => amount,
            FractionOrAmount::Fraction(f) => f
                .to_absolute(balance)
                .ok_or(DcError::InvalidFractionalAmount(f))?,
        };

        let available = allowance.min(balance);

        if actual_amount > available {
            return Err(DcError::InsufficientFunds {
                required: actual_amount,
                available,
                address,
            });
        }

        addresses.push(address);
        amounts.push(actual_amount);
    }

    let tx = contract
        .collectERC20(
            request.token,
            request.recipient,
            addresses.clone(),
            amounts.clone(),
        )
        .send()
        .await
        .map_err(DcError::unexpected)?
        .get_receipt()
        .await?;

    Ok(CollectErc20Response(DisperseCollectResponse {
        tx_hash: tx.transaction_hash,
        transfers: BTreeMap::from_iter(addresses.into_iter().zip(amounts)),
    }))
}

fn get_disperse_args(
    total_balance: U256,
    recipients: impl Iterator<Item = (Address, FractionOrAmount)>,
) -> Result<(Vec<Address>, Vec<U256>), DcError> {
    let (l, u) = recipients.size_hint();
    let iter_len = u.unwrap_or(l);

    let mut addresses = Vec::with_capacity(iter_len);
    let mut amounts = Vec::with_capacity(iter_len);
    let mut sum = U256::ZERO;

    for (address, amount) in recipients {
        let actual_amount = match amount {
            FractionOrAmount::Amount { amount } => amount,
            FractionOrAmount::Fraction(f) => f
                .to_absolute(total_balance)
                .ok_or(DcError::InvalidFractionalAmount(f))?,
        };
        sum += actual_amount;

        addresses.push(address);
        amounts.push(actual_amount);
    }

    if sum > total_balance {
        return Err(DcError::InsufficientFunds {
            required: sum,
            available: total_balance,
            address: Default::default(),
        });
    }

    Ok((addresses, amounts))
}
