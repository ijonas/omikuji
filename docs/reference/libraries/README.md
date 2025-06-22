# Library Reference Documentation

This directory contains reference documentation for the key third-party libraries used by Omikuji.

## Core Libraries

### Blockchain & Ethereum
- **[alloy-rs](alloy-rs-documentation.md)** - Comprehensive Ethereum library for contract interaction, providers, and transactions
- **[Flux Interface](flux-interface-docs.md)** - Chainlink FluxAggregator contract interface documentation

### Async Runtime & Networking
- **[tokio](tokio.md)** - Asynchronous runtime powering Omikuji's concurrent operations
- **[reqwest](reqwest.md)** - HTTP client for fetching external data from APIs

### Data Processing
- **[serde](serde.md)** - Serialization framework for configuration and JSON handling
- **[sqlx](sqlx.md)** - Async PostgreSQL driver with compile-time checked queries

### Observability
- **[tracing](tracing.md)** - Application-level tracing and structured logging

### Internal Documentation
- **[Feed Value Retrieval](feed-value-retrieval.md)** - Design documentation for the feed value retrieval system

## Usage

These documents provide:
- API references and examples
- Best practices for each library
- Integration patterns used in Omikuji
- Performance considerations

## Updating Library Documentation

When updating dependencies:
1. Check for API changes that affect Omikuji
2. Update relevant documentation if interfaces change
3. Test all integrations thoroughly

## External Resources

For the latest documentation, always refer to the official sources:
- [alloy-rs GitHub](https://github.com/alloy-rs/alloy)
- [tokio.rs](https://tokio.rs/)
- [serde.rs](https://serde.rs/)
- [sqlx GitHub](https://github.com/launchbadge/sqlx)
- [tracing GitHub](https://github.com/tokio-rs/tracing)