# Alloy-rs Documentation

This documentation provides comprehensive information about the alloy-rs library, which is used extensively in the Omikuji project for Ethereum blockchain interactions.

## Table of Contents

1. [Installation](#installation)
2. [Contract Interaction](#contract-interaction)
3. [Provider Setup](#provider-setup)
4. [Transaction Signing](#transaction-signing)
5. [RPC Client Usage](#rpc-client-usage)
6. [Network Abstraction](#network-abstraction)
7. [API Reference](#api-reference)

## Installation

### Via Cargo CLI

The easiest way to get started with Alloy:

```shell
cargo add alloy --features full
```

### Via Cargo.toml

Alternatively, add to your `Cargo.toml`:

```toml
[dependencies]
alloy = { version = "1.0.1", features = ["full"] }
```

## Contract Interaction

### Using CallBuilder with sol! Macro

This example demonstrates how to use `alloy-contract`'s `CallBuilder` with the `sol!` macro to interact with on-chain contracts:

```rust
use alloy_contract::SolCallBuilder;
use alloy_network::Ethereum;
use alloy_primitives::{Address, U256};
use alloy_provider::ProviderBuilder;
use alloy_sol_types::sol;

sol! {
    #[sol(rpc)] // <-- Important! Generates the necessary `MyContract` struct and function methods.
    #[sol(bytecode = "0x1234")] // <-- Generates the `BYTECODE` static and the `deploy` method.
    contract MyContract {
        constructor(address) {} // The `deploy` method will also include any constructor arguments.

        #[derive(Debug)]
        function doStuff(uint a, bool b) public payable returns(address c, bytes32 d);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build a provider
    let provider = ProviderBuilder::new().connect("http://localhost:8545").await?;

    // Deploy contract if bytecode is provided
    let constructor_arg = Address::ZERO;
    let contract = MyContract::deploy(&provider, constructor_arg).await?;

    // Or create instance of already deployed contract
    let address = Address::ZERO;
    let contract = MyContract::new(address, &provider);

    // Build a call to the `doStuff` function
    let a = U256::from(123);
    let b = true;
    let call_builder = contract.doStuff(a, b).value(U256::from(50e18 as u64));

    // Send the call (not broadcasted as transaction)
    let call_return = call_builder.call().await?;
    println!("{call_return:?}"); // doStuffReturn { c: 0x..., d: 0x... }

    // Use `send` to broadcast as a transaction
    let pending_tx = call_builder.send().await?;
    
    Ok(())
}
```

## Provider Setup

### Basic HTTP Provider Usage

```rust
use alloy_provider::{ProviderBuilder, RootProvider, Provider};
use alloy_network::Ethereum;
use alloy_primitives::address;
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a basic HTTP provider
    let provider = RootProvider::<Ethereum>::new_http("https://reth-ethereum.ithaca.xyz/rpc".parse()?);

    // Get the latest block number
    let block_number = provider.get_block_number().await?;
    println!("Latest block number: {block_number}");

    // Get balance of an address
    let address = address!("0x71C7656EC7ab88b098defB751B7401B5f6d8976F");
    let balance = provider.get_balance(address).await?;
    println!("Balance: {balance}");

    // Use the builder pattern to create a provider with recommended fillers
    let provider = ProviderBuilder::new().connect_http("https://reth-ethereum.ithaca.xyz/rpc".parse()?);

    Ok(())
}
```

## Transaction Signing

### Sign Ethereum Transaction

```rust
use alloy_consensus::TxLegacy;
use alloy_primitives::{U256, address, bytes};
use alloy_signer::{Signer, SignerSync};
use alloy_signer_local::PrivateKeySigner;
use alloy_network::TxSignerSync;

fn sign_transaction() -> Result<(), Box<dyn std::error::Error>> {
    // Instantiate a signer
    let signer = "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7"
        .parse::<PrivateKeySigner>()?;

    // Create a transaction
    let mut tx = TxLegacy {
        to: address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045").into(),
        value: U256::from(1_000_000_000),
        gas_limit: 2_000_000,
        nonce: 0,
        gas_price: 21_000_000_000,
        input: bytes!(),
        chain_id: Some(1),
    };

    // Sign it
    let signature = signer.sign_transaction_sync(&mut tx)?;
    
    Ok(())
}
```

### Sign Ethereum Prefixed Message (EIP-712)

```rust
use alloy_signer::{Signer, SignerSync};
use alloy_signer_local::PrivateKeySigner;

fn sign_message() -> Result<(), Box<dyn std::error::Error>> {
    // Instantiate a signer
    let signer = PrivateKeySigner::random();

    // Sign a message
    let message = "Some data";
    let signature = signer.sign_message_sync(message.as_bytes())?;

    // Recover the signer from the message
    let recovered = signature.recover_address_from_msg(message)?;
    assert_eq!(recovered, signer.address());
    
    Ok(())
}
```

## RPC Client Usage

### Single RPC Request

```rust
use alloy_rpc_client::{ReqwestClient, ClientBuilder};

async fn single_request(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Instantiate a new client
    let client: ReqwestClient = ClientBuilder::default().http(url);

    // Prepare a request
    let request = client.request_noparams("eth_blockNumber");

    // Poll the request to completion
    let block_number = request.await?;
    
    Ok(())
}
```

### Batch RPC Requests

```rust
use alloy_rpc_client::{ReqwestClient, ClientBuilder};
use alloy_primitives::Address;

async fn batch_request(url: &str, address: Address) -> Result<(), Box<dyn std::error::Error>> {
    // Instantiate a new client
    let client: ReqwestClient = ClientBuilder::default().http(url);

    // Prepare a batch request
    let batch = client.new_batch();

    // Add calls to the batch
    let block_number_fut = batch.add_call("eth_blockNumber", ()).unwrap();
    let balance_fut = batch.add_call("eth_getBalance", address).unwrap();

    // Send the batch
    batch.send().await?;

    // Get the results
    let block_number = block_number_fut.await?;
    let balance = balance_fut.await?;
    
    Ok(())
}
```

## Network Abstraction

### Implement Network Trait for Custom Blockchain

```rust
use alloy_network::Network;

// Foo must be a ZST (Zero-Sized Type)
struct Foo;

impl Network for Foo {
    type Transaction = FooTransaction;
    type Block = FooBlock;
    type Header = FooHeader;
    type Receipt = FooReceipt;
    // etc.
}
```

### Add Custom RPC Methods

```rust
use alloy_provider::Provider;
use alloy_rpc_types::RpcResult;
use alloy_transport::TransportError;
use async_trait::async_trait;

#[async_trait]
trait FooProviderExt: Provider<Foo> {
    async fn custom_foo_method(&self) -> RpcResult<Something, TransportError>;
    async fn another_custom_method(&self) -> RpcResult<Something, TransportError>;
}
```

## API Reference

### Core Crates

- **`alloy`**: Meta-crate for the entire project, including `alloy-core`
- **`alloy-consensus`**: Ethereum consensus interface
- **`alloy-contract`**: Interact with on-chain contracts
- **`alloy-eips`**: Ethereum Improvement Proposal (EIP) implementations
- **`alloy-genesis`**: Ethereum genesis file definitions
- **`alloy-json-rpc`**: Core data types for JSON-RPC 2.0 clients
- **`alloy-network`**: Network abstraction for RPC types
- **`alloy-node-bindings`**: Ethereum execution-layer client bindings
- **`alloy-provider`**: Interface with an Ethereum blockchain
- **`alloy-pubsub`**: Ethereum JSON-RPC publish-subscribe service

### RPC Types Crates

- **`alloy-rpc-types`**: Meta-crate for all Ethereum JSON-RPC types
- **`alloy-rpc-types-admin`**: Types for the `admin` namespace
- **`alloy-rpc-types-anvil`**: Types for Anvil development node
- **`alloy-rpc-types-beacon`**: Types for Ethereum Beacon Node API
- **`alloy-rpc-types-debug`**: Types for the `debug` namespace
- **`alloy-rpc-types-engine`**: Types for the `engine` namespace
- **`alloy-rpc-types-eth`**: Types for the `eth` namespace
- **`alloy-rpc-types-trace`**: Types for the `trace` namespace
- **`alloy-rpc-types-txpool`**: Types for the `txpool` namespace

### Signer Crates

- **`alloy-signer`**: Ethereum signer abstraction
- **`alloy-signer-aws`**: AWS KMS signer implementation
- **`alloy-signer-gcp`**: GCP KMS signer implementation
- **`alloy-signer-ledger`**: Ledger signer implementation
- **`alloy-signer-local`**: Local signer implementations (private key, keystore, mnemonic)
- **`alloy-signer-trezor`**: Trezor signer implementation

### Transport Crates

- **`alloy-transport`**: Low-level Ethereum JSON-RPC transport abstraction
- **`alloy-transport-http`**: HTTP transport implementation
- **`alloy-transport-ipc`**: IPC transport implementation
- **`alloy-transport-ws`**: WebSocket transport implementation

## Development Commands

Common cargo commands for Alloy development:

```sh
cargo check --all-features
cargo +nightly fmt --all
cargo build --all-features
cargo test --all-features
cargo test --no-default-features
cargo +nightly clippy --all-features
```

## Provider Methods

Key provider trait methods:

- `uninstall_filter()`
- `get_block_transaction_count_by_number()`
- `get_block_transaction_count_by_hash()`
- `get_filter_logs()`
- `get_block_by_number()`
- `eth_call()` (defaults to Pending block)
- `eth_estimateGas()` (defaults to Pending block)

## PubSub Components

### Core Types

- **`PubSubConnect`**: Trait for instantiating a PubSub service
- **`ConnectionHandle`**: Handle to a running backend
- **`ConnectionInterface`**: Backend's interface to communicate with service
- **`PubSubFrontend`**: Handle to issue requests and manage subscriptions
- **`RawSubscription`**: Handle to a subscription (tokio broadcast channel)
- **`Subscription`**: Typed wrapper around `RawSubscription`
- **`SubscriptionItem`**: Deserialized notification item

### Request Lifecycles

1. **Regular Request**: User → Frontend → Service → Backend → RPC Server → Response path
2. **Subscription Request**: Similar to regular but returns `U256` server_id, creates local subscription
3. **Subscription Notification**: RPC Server → Backend → Service → Subscription channel

## Release Process

For maintainers releasing new versions:

1. Install tools:
   ```sh
   cargo install cargo-release
   cargo install cargo-semver-checks
   cargo install --git https://github.com/DaniPopes/git-cliff.git --branch fix-include-paths git-cliff
   ```

2. Check and release:
   ```sh
   cargo +stable semver-checks
   cargo release <version>
   PUBLISH_GRACE_SLEEP=10 cargo release --execute <version>
   git push --tags
   ```

## Additional Resources

- [Official Alloy Repository](https://github.com/alloy-rs/alloy)
- [Alloy Core](https://github.com/alloy-rs/core)
- [Examples Repository](https://github.com/alloy-rs/examples)

This documentation is fetched from the official alloy-rs sources and provides a comprehensive guide for using alloy in the Omikuji project.