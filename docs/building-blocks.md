# Building Blocks

This document outlines the core architectural patterns and building blocks used in Omikuji. Adhering to these patterns will help maintain code quality, consistency, and readability as new features are added.

## 1. Database Module (`src/database`)

**Pattern:** Generic `Repository` Trait

- **Description:** To avoid boilerplate and ensure a consistent API for database interactions, we use a generic `Repository` trait. This trait defines common database operations like `save`, `get`, `delete`, etc.
- **Usage:** When adding new database tables, create a corresponding repository struct (e.g., `MyNewObjectRepository`) and implement the `Repository` trait for it. This centralizes database logic and makes it easier to test and maintain.
- **Example:**
  ```rust
  pub struct TransactionLogRepository {
      pool: Arc<PgPool>,
  }
  
  impl TransactionLogRepository {
      pub async fn save(&self, log: &TransactionLog) -> Result<()> { ... }
      pub async fn get_by_hash(&self, hash: &str) -> Result<Option<TransactionLog>> { ... }
  }
  ```

## 2. Contract Interaction (`src/contracts`)

**Pattern:** `ContractInteraction` and `ContractReader` Utilities

- **Description:** Interacting with smart contracts often involves repetitive boilerplate for building transactions, making calls, handling results, and recording metrics. The `ContractInteraction` and `ContractReader` utilities abstract this logic.
- **Usage:** 
  - For read-only operations, use `ContractReader` which handles metrics tracking automatically
  - For transactions, use `ContractInteraction` which provides gas configuration, retry logic, and standardized error handling
- **Examples:**
  ```rust
  // Read operation with metrics
  let reader = ContractReader::new(provider, address, network_name)
      .with_feed_name("eth_usd");
  let answer = reader.call(call_data, "latestAnswer", decode_fn).await?;
  
  // Transaction with full handling
  let interaction = ContractInteraction::new(provider, address, network_config)
      .with_feed_name("eth_usd");
  let receipt = interaction.submit_transaction_with_handling(
      call_data, context, gas_limit, tx_repo, gas_manager
  ).await?;
  ```
- **See:** `FluxAggregatorV2` in `src/contracts/flux_aggregator_v2.rs` for a complete example

## 3. Metrics (`src/metrics`)

**Pattern:** Consolidated Metrics Recording with `MetricsContext` and Specialized Recorders

- **Description:** Metrics recording should be consistent across all modules to avoid duplication and ensure proper tracking. The metrics system uses context objects and specialized recorders to handle common patterns like timing operations, recording retries, and tracking transaction metrics.
- **Key Components:**
  - `MetricsContext`: Carries feed_name, network, and optional method information
  - `TimedOperationRecorder`: Automatically tracks operation duration and success/failure
  - `TransactionMetricsRecorder`: Handles all transaction-related metrics (gas, retries, reverts)
  - `RetryMetricsRecorder`: Tracks retry attempts and reasons consistently
  - `FeedMetricsRecorder`: Manages feed value and deviation metrics
- **Usage:** 
  ```rust
  // Basic context creation
  let context = MetricsContext::new("eth_usd", "ethereum");
  
  // Timed operations
  let recorder = TimedOperationRecorder::contract_read(context.clone(), "latestAnswer");
  let result = perform_operation().await;
  recorder.record_result(&result, None);
  
  // Transaction metrics
  let tx_recorder = TransactionMetricsRecorder::new(context, "eip1559");
  tx_recorder.record_success(&receipt, gas_limit, Some(submission_time));
  
  // Retry tracking
  let mut retry_recorder = RetryMetricsRecorder::new(context, max_attempts);
  let attempt = retry_recorder.start_attempt();
  if should_retry {
      retry_recorder.record_retry("network_congestion");
  }
  ```
- **Benefits:** Ensures consistent metrics labeling, reduces boilerplate, and provides structured retry/transaction tracking
- **See:** `FluxAggregatorContractV2` for comprehensive example usage

**Pattern:** `MetricsFactory`

- **Description:** To simplify the creation of Prometheus metrics and ensure consistency, we use a `MetricsFactory`. This factory provides a simple interface for creating common metric types (counters, gauges, histograms).
- **Usage:** When adding new metrics, use the `MetricsFactory` to create them. This avoids the need to use `lazy_static` and the `register_*` macros directly, reducing boilerplate and ensuring that all metrics are registered correctly.
- **Example:**
  ```rust
  lazy_static! {
      static ref UPDATE_ATTEMPTS: IntCounterVec = register_int_counter_vec!(
          "omikuji_update_attempts_total",
          "Total number of update attempts",
          &["feed", "network", "status"]
      ).unwrap();
  }
  ```

## 4. Configuration (`src/config`)

**Pattern:** `#[serde(default)]` with `Default` Trait

- **Description:** For configuration structs, we use `#[serde(default)]` on the struct itself and implement the `Default` trait. This provides a clean and idiomatic way to handle default values for configuration options.
- **Usage:** When adding new configuration structs, implement the `Default` trait to provide sensible defaults for all fields. Then, add `#[serde(default)]` to the struct definition. This makes the configuration more robust and easier to manage.
- **Example:**
  ```rust
  #[derive(Debug, Clone, Deserialize)]
  #[serde(default)]
  pub struct GasConfig {
      pub gas_limit: Option<u64>,
      pub max_gas_price_gwei: Option<u64>,
      pub priority_fee_gwei: Option<u64>,
  }
  
  impl Default for GasConfig {
      fn default() -> Self {
          Self {
              gas_limit: None,
              max_gas_price_gwei: Some(50),
              priority_fee_gwei: Some(2),
          }
      }
  }
  ```

## 5. Error Handling

**Pattern:** Module-specific errors with `thiserror`

- **Description:** We use the `thiserror` crate to create custom, structured error types for each module. A top-level error enum wraps these module-specific errors, providing a clear and consistent error handling strategy across the application.
- **Usage:** When adding new functionality that can fail, define a new variant in the module's error enum. Use `#[from]` to automatically convert from underlying error types. This makes error handling more explicit and debugging easier.
- **Context Pattern:** Use `.with_context(|| format!("..."))` from the `anyhow` crate to add context to errors at the point where they occur.

## 6. Transaction Handling (`src/utils/transaction_handler.rs`)

**Pattern:** Unified Transaction Processing

- **Description:** All blockchain transactions follow a similar pattern: submission, receipt waiting, metrics recording, and cost calculation. The `TransactionHandler` provides a builder-pattern API to handle these common operations.
- **Usage:** After receiving a transaction receipt, create a `TransactionHandler` with the appropriate context and use the builder methods to configure optional dependencies.
- **Example:**
  ```rust
  TransactionHandler::new(receipt, context, network)
      .with_gas_price_manager(gas_price_manager.as_ref())
      .with_tx_log_repo(tx_log_repo.as_ref())
      .with_gas_limit(gas_limit)
      .with_transaction_type(tx_type)
      .process()
      .await?;
  ```

## 7. Logging Utilities (`src/utils/tx_logger.rs`)

**Pattern:** Standardized Transaction Logging

- **Description:** Consistent logging messages for transaction-related events help with debugging and monitoring. The `TransactionLogger` provides static methods for common logging scenarios.
- **Usage:** Use `TransactionLogger` methods instead of direct `info!`, `error!`, etc. calls for transaction-related events.
- **Example:**
  ```rust
  TransactionLogger::log_submission("datafeed", &feed_name, &network, Some(&value));
  TransactionLogger::log_confirmation(tx_hash, gas_used);
  TransactionLogger::log_usd_cost(total_cost, gas_used, gas_price, token_price);
  ```

## 8. Gas Utilities (`src/gas/utils.rs`)

**Pattern:** Standardized Gas Unit Conversions

- **Description:** Gas calculations often require conversions between wei, gwei, and ether. The gas utilities module provides consistent, tested conversion functions.
- **Usage:** Always use the utility functions for gas conversions to avoid precision errors and ensure consistency.
- **Examples:**
  ```rust
  use crate::gas::utils;
  
  // Convert gwei to wei
  let gas_price_wei = utils::gwei_to_wei(50.0);
  
  // Calculate gas cost
  let total_cost = utils::calculate_gas_cost(gas_used, gas_price_wei);
  
  // Format for display
  let cost_str = utils::format_wei(total_cost); // "0.005 ETH"
  
  // Calculate fee bumps for retries
  let bumped_fee = utils::calculate_fee_bump(base_fee, attempt, 10.0);
  ```

## 9. Transaction Building (`src/gas/transaction_builder.rs`)

**Pattern:** Type-Safe Transaction Construction

- **Description:** Building transactions with proper gas configuration is complex and error-prone. The `GasAwareTransactionBuilder` provides a fluent API for constructing transactions.
- **Usage:** Use the builder for all transaction construction to ensure proper gas settings based on network configuration.
- **Example:**
  ```rust
  let tx = GasAwareTransactionBuilder::new(provider, to, data, network_config)
      .with_value(value)
      .with_gas_limit(300_000)
      .build()
      .await?;
  ```
- **Best Practice:** The builder handles transaction type (legacy vs EIP-1559) automatically based on network configuration.

## 10. Provider Management (`src/network`)

**Pattern:** Centralized Provider Factory with Read/Write Separation

- **Description:** Network providers are expensive to create and should be reused. The `NetworkManager` maintains a cache of read-only providers and provides methods for creating signer providers on demand.
- **Usage:** 
  - For read operations: Use `NetworkManager::get_provider()` to get cached providers
  - For write operations: Create signer providers on demand using the pattern shown in `ContractUpdater::create_signer_provider()`
- **Example:**
  ```rust
  // Read provider (cached)
  let provider = network_manager.get_provider(network_name)?;
  
  // Signer provider (created on demand)
  async fn create_signer_provider(
      network_manager: &NetworkManager,
      network_name: &str,
  ) -> Result<impl Provider<Http<Client>, Ethereum> + Clone> {
      let private_key = network_manager.get_private_key(network_name)?;
      let rpc_url = network_manager.get_rpc_url(network_name)?;
      
      let signer = private_key.parse::<PrivateKeySigner>()?;
      let wallet = EthereumWallet::from(signer);
      
      let url = Url::parse(rpc_url)?;
      let provider = ProviderBuilder::new()
          .with_recommended_fillers()
          .wallet(wallet)
          .on_http(url);
      
      Ok(provider)
  }
  ```
- **Best Practices:**
  - Cache read-only providers for performance
  - Create signer providers on demand to avoid key exposure
  - Consider implementing provider pooling for high-frequency write operations
  - Store references to providers in long-lived structures to avoid repeated lookups

## 11. Gas Estimation (`src/gas`)

**Pattern:** Strategy-based Gas Configuration

- **Description:** Different networks and transaction types require different gas strategies. The gas estimation module provides flexible configuration while maintaining safe defaults.
- **Usage:** Use the `GasEstimator` to calculate appropriate gas limits and prices based on network configuration and current conditions.
- **Configuration:** Gas settings cascade from global → network → datafeed/task level, allowing fine-grained control.
- **Integration:** Works seamlessly with `GasAwareTransactionBuilder` for complete transaction preparation.

## 12. Async Task Management

**Pattern:** Tokio Task Spawning with Graceful Shutdown

- **Description:** Long-running tasks (datafeeds, scheduled tasks) are spawned as independent Tokio tasks with proper error handling and shutdown signals.
- **Usage:** Use `tokio::spawn` with a select! macro to handle both the main task and shutdown signals. Always log task lifecycle events.
- **Example:**
  ```rust
  tokio::spawn(async move {
      tokio::select! {
          result = task_future => {
              if let Err(e) = result {
                  error!("Task failed: {}", e);
              }
          }
          _ = shutdown_signal => {
              info!("Task shutting down");
          }
      }
  });
  ```

## 13. Testing Patterns

**Pattern:** Test Utilities and Mocks

- **Description:** Common test scenarios should have reusable utilities. Mock implementations should implement the same traits as production code.
- **Usage:** Create test utility modules for common operations like creating test datafeeds, mock providers, and assertion helpers.
- **Location:** Test utilities go in `src/test_utils/` for cross-module use or in module-specific `tests/` subdirectories.

## 14. Builder Pattern for Complex Objects

**Pattern:** Fluent API Builders

- **Description:** Complex configuration objects benefit from builder patterns that provide a fluent API and validation at build time.
- **Usage:** Implement builders for objects with many optional fields or complex validation requirements.
- **Example:**
  ```rust
  let datafeed = DatafeedBuilder::new("eth_usd")
      .with_network("mainnet")
      .with_check_frequency(60)
      .with_deviation_threshold(0.5)
      .build()?;
  ```

## 15. ABI Encoding/Decoding (`src/contracts/abi_utils.rs`)

**Pattern:** Centralized ABI Utilities

- **Description:** ABI encoding and decoding is repetitive and error-prone. The ABI utilities module provides type-safe abstractions for common operations.
- **Usage:** Use the utility functions and builders instead of manual ABI encoding/decoding.
- **Examples:**
  ```rust
  // Parse function signatures
  let (name, params) = parse_function_signature("transfer(address,uint256)")?;
  
  // Encode parameters dynamically
  let encoded = encode_parameters(&["0x123...", "1000"], &["address", "uint256"])?;
  
  // Build function calls fluently
  let call_data = ContractCallBuilder::new("transfer(address,uint256)")
      .with_param("0x123...")
      .with_param("1000")
      .build()?;
  
  // Use common call helpers
  let data = common_calls::balance_of(address)?;
  ```
- **Best Practices:**
  - Use `parse_function_signature()` for dynamic function parsing
  - Use `ContractCallBuilder` for constructing calls with proper encoding
  - Leverage `common_calls` module for standard ERC20/contract operations

## 16. Metrics-Aware Contract Calls (`src/contracts/generic_caller.rs`)

**Pattern:** Automatic Metrics and Error Context

- **Description:** Every contract call should record metrics and provide error context. The `MetricsAwareContractCaller` handles this automatically.
- **Usage:** Use the generic caller instead of direct provider calls for consistent metrics and error handling.
- **Example:**
  ```rust
  // Create a metrics-aware caller
  let caller = create_contract_reader(provider, contract_address, "mainnet")
      .with_feed_name("eth_usd");
  
  // Make calls with automatic metrics
  let result = caller.call(
      call_data,
      "latestAnswer",
      |bytes| decode_int256(bytes)
  ).await?;
  
  // Or use the builder pattern
  let result = ContractCallBuilder::new(provider, address, network, "balanceOf")
      .with_feed_name("eth_usd")
      .with_data(encoded_call)
      .execute(decode_fn)
      .await?;
  ```
- **Benefits:**
  - Automatic success/failure metrics recording
  - Consistent error context and logging
  - Reduced boilerplate in contract interaction code

## 17. Error Handling and Context (`src/error_context.rs`, `src/error_handlers.rs`)

**Pattern:** Consolidated Error Context and Domain-Specific Handlers

- **Description:** Error messages and context patterns are centralized to ensure consistency and reduce duplication. Domain-specific error types provide better error categorization.
- **Usage:** Use the error context helpers and extension traits instead of manual error formatting.
- **Examples:**
  ```rust
  // Instead of:
  .with_context(|| format!("Failed to create provider for network {}", network.name))?
  
  // Use:
  .map_err(|e| NetworkOperationError::provider_creation(&network.name, e))?
  
  // Or with extension trait:
  .context_network("get_chain_id", network_name)?
  
  // For database operations:
  .context_db("insert", "feed_log")?
  
  // For validation:
  validation::require_not_empty(&name, "network name")?;
  validation::require_positive(gas_price, "gas price")?;
  ```
- **Key Components:**
  - `error_context` module provides consistent message templates
  - `ErrorContextExt` trait adds context methods to Result types
  - Domain-specific error types (NetworkOperationError, DatabaseOperationError, etc.)
  - Validation helpers for common checks
  - Retry helper with context-aware error handling
- **Benefits:**
  - Consistent error messages across the codebase
  - Reduced boilerplate in error handling
  - Better error categorization and debugging
  - Centralized location for error message updates

## Best Practices Summary

1. **DRY (Don't Repeat Yourself):** Extract common patterns into reusable utilities
2. **Fail Fast:** Validate configuration and inputs early
3. **Explicit over Implicit:** Make dependencies and side effects clear
4. **Testability:** Design with testing in mind - use dependency injection
5. **Observability:** Always include appropriate logging and metrics
6. **Error Context:** Add context to errors at the point of occurrence
7. **Type Safety:** Leverage Rust's type system to prevent errors at compile time
