# DisperseCollect API
Overview
Application manages a set of private keys and allows dispersing/collecting eth/ERC20 by calling a smart contract using one of supported private keys.

## Running the Application

### Setup forked environment
Navigate to `disperse-collect-contracts` repo and perform steps described in README.md.
### Modify env
```plaintext
RPC_URL=http://localhost:8545
PORT=8080
TX_SIGNER=<your_private_key>
CONTRACT_ADDRESS=<deployed_contract_address>
RUST_LOG=info
```
### Run the app
```bash
cargo run --release
```

### What is good about the app
- Access list generation
- Idiomatic error handling
- State-of-the-art ecosystem usage

### What can be improved:
- Api-level authentication
- Load multiple private keys from env
- Write a helper contract to reduce the amount RPC calls for batch balance and allowance requests

## API Endpoints
### Disperse ETH
`POST /api/disperse-eth`

Disperse ETH to multiple recipients.
#### Request
```json
{
  "recipients": {
    "0xRecipientAddress1": { "amount": "1000000000000000000" }, // 1 ETH
    "0xRecipientAddress2": { "fraction": "11", "units": "1000" } // 0.011% of total balance
  },
  "caller": "0xYourAddress"
}
```
#### Request


```json
{
  "tx": {
    "txHash": "0xTransactionHash"
  },
  "transfers": {
    "0xRecipientAddress1": "1000000000000000000",
    "0xRecipientAddress2": "500000000000000000"
  }
}
```
### Disperse ERC20
`POST /api/disperse-erc20`

Disperse ERC20 tokens to multiple recipients.

#### Request

```json
{
  "recipients": {
    "0xRecipientAddress1": { "amount": "1000" }, // 1000 tokens
    "0xRecipientAddress2": { "fraction": "5", "units": "10" } // 50% of total tokens
  },
  "token": "0xTokenAddress",
  "spender": "0xSpenderAddress", // address that allowed the contract to spend its tokens
  "caller": "0xYourAddress" // address that signs the tx
}
```
#### Response

```json
{
  "tx": {
    "txHash": "0xTransactionHash"
  },
  "transfers": {
    "0xRecipientAddress1": "1000",
    "0xRecipientAddress2": "500"
  }
}
```
### Collect ERC20
`POST /api/collect-erc20`

Collect ERC20 tokens from multiple spenders to a single recipient.

##### Request 
```json
{
  "recipient": "0xRecipientAddress", // address that receives the tokens
  "token": "0xTokenAddress",
  "spenders": { // addresses that allow the contract to spend tokens
    "0xSpenderAddress1": { "amount": "500" },
    "0xSpenderAddress2": { "fraction": "3", "units": "10" } // 30% of spender's balance
  },
  "caller": "0xYourAddress" // address that calls the contract
}
```
#### Response

```json
{
  "tx": {
    "txHash": "0xTransactionHash"
  },
  "transfers": {
    "0xSpenderAddress1": "500",
    "0xSpenderAddress2": "300"
  }
}
```
### Transfer Tokens
`POST /api/transfer`

Transfer tokens or ETH to a recipient.
#### Request

```json
{
  "recipient": "0xRecipientAddress",
  "value": { "amount": "1000" }, // For ERC20 tokens or ETH
  "token": "0xTokenAddress", // Optional, omit for ETH transfers
  "caller": "0xYourAddress"
}
```
#### Response

```json
{
  "txHash": "0xTransactionHash"
}
```
### Approve ERC20 Spender
`POST /api/approve`

Approve a spender for a specific amount of ERC20 tokens.

#### Request

```json

{
  "spender": "0xSpenderAddress",
  "amount": { "amount": "1000" }, // Amount to approve
  "token": "0xTokenAddress",
  "caller": "0xYourAddress"
}
```

#### Response

```json
{
  "txHash": "0xTransactionHash"
}
```
