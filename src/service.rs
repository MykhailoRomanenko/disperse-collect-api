use std::collections::BTreeMap;

use alloy::{
    contract,
    network::TransactionBuilder,
    primitives::{Address, U256},
    providers::{Provider, WalletProvider},
    rpc::types::TransactionRequest,
    serde::WithOtherFields,
    transports::{RpcError, TransportErrorKind},
};
use futures::future::try_join_all;
use thiserror::Error;
use tokio::try_join;

use alloy::contract::Error as ContractError;
use tracing::instrument;

use crate::{
    contracts::{DisperseCollectContract, Erc20Contract},
    dto::{
        ApproveRequest, CollectErc20Request, CollectErc20Response, DisperseCollectResponse,
        DisperseErc20Request, DisperseErc20Response, DisperseEthRequest, DisperseEthResponse,
        FractionOrAmount, FractionalAmount, TransactionResponse, TransferRequest,
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
    #[error(transparent)]
    InvalidFractionalAmount(#[from] InvalidFractionalAmountError),
    #[error("erc20 not found at address: {0}")]
    TokenNotFound(Address),
    #[error("error communicating with node: {0}")]
    Transport(#[from] TransportErrorKind),
    #[error("unexpected error: {0}")]
    Unexpected(#[source] anyhow::Error),
    #[error("no signer found for {0}")]
    SignerNotFound(Address),
}

#[derive(Debug, thiserror::Error)]
#[error("fraction {0} results in invalid or zero amount for corresponding balance")]
pub struct InvalidFractionalAmountError(FractionalAmount);

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
    let available_balance = provider.get_balance(request.caller).await?;

    let (addresses, amounts) = construct_disperse_recipients(
        request.caller,
        available_balance,
        request.recipients.into_iter(),
    )?;

    let tx = contract
        .disperseEth(addresses.clone(), amounts.clone())
        .value(amounts.iter().sum())
        .into_transaction_request();

    let tx_response = send_transaction(provider, tx, request.caller).await?;

    Ok(DisperseEthResponse(DisperseCollectResponse {
        transfers: BTreeMap::from_iter(addresses.into_iter().zip(amounts)),
        tx: tx_response,
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

    let (addresses, amounts) = construct_disperse_recipients(
        request.spender,
        available_balance,
        request.recipients.into_iter(),
    )?;

    let tx = contract
        .disperseERC20(
            request.spender,
            request.token,
            addresses.clone(),
            amounts.clone(),
        )
        .into_transaction_request();

    let tx_response = send_transaction(provider, tx, request.caller).await?;

    Ok(DisperseErc20Response(DisperseCollectResponse {
        tx: tx_response,
        transfers: BTreeMap::from_iter(addresses.into_iter().zip(amounts)),
    }))
}

#[instrument(skip(provider, contract), target = "collect_erc20")]
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
        let actual_amount = normalize_amount(amount, balance)?;

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
        .into_transaction_request();

    let tx_response = send_transaction(provider, tx, request.caller).await?;

    Ok(CollectErc20Response(DisperseCollectResponse {
        tx: tx_response,
        transfers: BTreeMap::from_iter(addresses.into_iter().zip(amounts)),
    }))
}

pub async fn transfer(
    provider: &DefaultProvider,
    request: TransferRequest,
) -> Result<TransactionResponse, DcError> {
    match request.token {
        Some(addr) => {
            transfer_erc20(
                provider,
                request.caller,
                request.recipient,
                addr,
                request.value,
            )
            .await
        }
        None => transfer_eth(provider, request.caller, request.recipient, request.value).await,
    }
}

pub async fn transfer_eth(
    provider: &DefaultProvider,
    caller: Address,
    recipient: Address,
    amount: FractionOrAmount,
) -> Result<TransactionResponse, DcError> {
    let available_balance = provider.get_balance(caller).await?;

    let actual_amount = normalize_amount(amount, available_balance)?;

    if actual_amount > available_balance {
        return Err(DcError::InsufficientFunds {
            required: actual_amount,
            available: available_balance,
            address: caller,
        });
    }

    let tx = TransactionRequest::default()
        .value(actual_amount)
        .to(recipient);

    let tx_response = send_transaction(provider, WithOtherFields::new(tx), caller).await?;

    Ok(tx_response)
}

pub async fn transfer_erc20(
    provider: &DefaultProvider,
    caller: Address,
    recipient: Address,
    token_address: Address,
    amount: FractionOrAmount,
) -> Result<TransactionResponse, DcError> {
    let token = Erc20Contract::new(token_address, provider.clone());
    let balance = get_erc20_balance(&token, caller).await?;

    let actual_amount = normalize_amount(amount, balance)?;

    if actual_amount > balance {
        return Err(DcError::InsufficientFunds {
            required: actual_amount,
            available: balance,
            address: caller,
        });
    }

    let tx = token
        .transfer(recipient, actual_amount)
        .into_transaction_request();

    let tx_response = send_transaction(provider, tx, caller).await?;

    Ok(tx_response)
}

pub async fn approve(
    provider: &DefaultProvider,
    request: ApproveRequest,
) -> Result<TransactionResponse, DcError> {
    let token = Erc20Contract::new(request.token, provider.clone());

    let balance = get_erc20_balance(&token, request.caller).await?;
    let actual_amount = normalize_amount(request.amount, balance)?;

    let tx = token
        .approve(request.spender, actual_amount)
        .into_transaction_request();

    let tx_response = send_transaction(provider, tx, request.caller).await?;

    Ok(tx_response)
}

async fn get_erc20_balance(token: &Erc20Contract, address: Address) -> Result<U256, DcError> {
    token
        .balanceOf(address)
        .call()
        .await
        .map(|b| b._0)
        .map_err(|e| DcError::from_erc20_err(e, *token.address()))
}

fn normalize_amount(
    amount: FractionOrAmount,
    available_balance: U256,
) -> Result<U256, InvalidFractionalAmountError> {
    let actual_amount = match amount {
        FractionOrAmount::Amount { amount } => amount,
        FractionOrAmount::Fraction(f) => f
            .to_absolute(available_balance)
            .filter(|a| *a != U256::ZERO)
            .ok_or(InvalidFractionalAmountError(f))?,
    };

    Ok(actual_amount)
}

async fn send_transaction(
    provider: &DefaultProvider,
    mut tx: WithOtherFields<TransactionRequest>,
    signer: Address,
) -> Result<TransactionResponse, DcError> {
    if !provider.has_signer_for(&signer) {
        return Err(DcError::SignerNotFound(signer));
    }

    tx.set_from(signer);

    let access_list = provider.create_access_list(&tx).await?.access_list;

    tx.set_access_list(access_list);

    let receipt = provider.send_transaction(tx).await?.get_receipt().await?;

    Ok(TransactionResponse {
        tx_hash: receipt.transaction_hash,
    })
}

fn construct_disperse_recipients(
    sender: Address,
    total_balance: U256,
    recipients: impl Iterator<Item = (Address, FractionOrAmount)>,
) -> Result<(Vec<Address>, Vec<U256>), DcError> {
    let (l, u) = recipients.size_hint();
    let iter_len = u.unwrap_or(l);

    let mut addresses = Vec::with_capacity(iter_len);
    let mut amounts = Vec::with_capacity(iter_len);
    let mut sum = U256::ZERO;

    for (address, amount) in recipients {
        let actual_amount = normalize_amount(amount, total_balance)?;
        sum += actual_amount;

        addresses.push(address);
        amounts.push(actual_amount);
    }

    if sum > total_balance {
        return Err(DcError::InsufficientFunds {
            required: sum,
            available: total_balance,
            address: sender,
        });
    }

    Ok((addresses, amounts))
}
