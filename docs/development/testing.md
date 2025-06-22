# Testing Guide

This guide covers testing strategies, tools, and best practices for Omikuji development.

## Test Philosophy

- **Test Pyramid**: Many unit tests, fewer integration tests, minimal E2E tests
- **Fast Feedback**: Tests should run quickly
- **Deterministic**: Tests should not be flaky
- **Isolated**: Tests should not depend on external services when possible

## Running Tests

### Basic Commands

```bash
# Run all tests
cargo test

# Run tests in a specific module
cargo test datafeed

# Run a specific test
cargo test test_calculate_deviation

# Run tests with output
cargo test -- --nocapture

# Run tests in parallel (default)
cargo test

# Run tests sequentially
cargo test -- --test-threads=1

# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test '*'

# Run with coverage (requires cargo-tarpaulin)
cargo tarpaulin --out Html
```

### Continuous Testing

Use cargo-watch for automatic test runs:

```bash
# Install cargo-watch
cargo install cargo-watch

# Watch and run tests on changes
cargo watch -x test

# Watch and run specific tests
cargo watch -x "test datafeed"
```

## Test Organization

### Unit Tests

Located in the same file as the code being tested:

```rust
// src/datafeed/json_extractor.rs

pub fn extract_value(json: &Value, path: &str) -> Result<f64> {
    // Implementation
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extract_simple_value() {
        let json = json!({"price": 100.5});
        let result = extract_value(&json, "price").unwrap();
        assert_eq!(result, 100.5);
    }
}
```

### Integration Tests

Located in `tests/` directory:

```rust
// tests/integration_test.rs

use omikuji::config::load_config;

#[test]
fn test_config_loading() {
    let config = load_config("tests/fixtures/test_config.yaml").unwrap();
    assert_eq!(config.networks.len(), 2);
}
```

### Test Fixtures

Store test data in `tests/fixtures/`:

```
tests/
├── fixtures/
│   ├── test_config.yaml
│   ├── sample_response.json
│   └── invalid_config.yaml
└── integration_test.rs
```

## Testing Patterns

### Testing Async Code

```rust
#[tokio::test]
async fn test_fetch_price() {
    let client = Client::new();
    let result = fetch_price(&client, "https://api.example.com").await;
    assert!(result.is_ok());
}
```

### Testing with Mocks

#### HTTP Mocking with mockito

```rust
#[tokio::test]
async fn test_api_request() {
    let _m = mockito::mock("GET", "/price")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"USD": 1234.56}"#)
        .create();
    
    let url = &mockito::server_url();
    let result = fetch_feed_value(&format!("{}/price", url)).await.unwrap();
    assert_eq!(result, 1234.56);
}
```

#### Custom Mocks

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    struct MockProvider {
        response: String,
    }
    
    impl Provider for MockProvider {
        async fn call(&self, _req: Request) -> Result<Response> {
            Ok(Response::new(self.response.clone()))
        }
    }
    
    #[tokio::test]
    async fn test_with_mock_provider() {
        let provider = MockProvider {
            response: "0x1234".to_string(),
        };
        // Test using mock provider
    }
}
```

### Testing Error Cases

```rust
#[test]
fn test_invalid_input() {
    let result = parse_value("invalid");
    assert!(result.is_err());
    
    match result {
        Err(e) => assert_eq!(e.to_string(), "Invalid number format"),
        Ok(_) => panic!("Expected error"),
    }
}

#[test]
#[should_panic(expected = "Division by zero")]
fn test_panic_condition() {
    calculate_percentage(100, 0);
}
```

### Testing with Temporary Files

```rust
use tempfile::TempDir;

#[test]
fn test_file_operations() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.yaml");
    
    // Write test file
    fs::write(&file_path, "test: data").unwrap();
    
    // Test file operations
    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "test: data");
    
    // Temp directory is automatically cleaned up
}
```

### Database Testing

```rust
#[sqlx::test]
async fn test_database_operations(pool: PgPool) -> sqlx::Result<()> {
    // This test gets its own database and transaction
    let repo = FeedLogRepository::new(pool);
    
    repo.insert_log("test_feed", 100.0).await?;
    let logs = repo.get_recent_logs("test_feed", 10).await?;
    
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].value, 100.0);
    
    Ok(())
}
```

## Test Data Strategies

### Builders for Complex Objects

```rust
#[cfg(test)]
mod tests {
    struct ConfigBuilder {
        networks: Vec<Network>,
        datafeeds: Vec<Datafeed>,
    }
    
    impl ConfigBuilder {
        fn new() -> Self {
            Self {
                networks: vec![],
                datafeeds: vec![],
            }
        }
        
        fn with_network(mut self, name: &str, url: &str) -> Self {
            self.networks.push(Network {
                name: name.to_string(),
                rpc_url: url.to_string(),
                ..Default::default()
            });
            self
        }
        
        fn build(self) -> Config {
            Config {
                networks: self.networks,
                datafeeds: self.datafeeds,
            }
        }
    }
    
    #[test]
    fn test_with_builder() {
        let config = ConfigBuilder::new()
            .with_network("ethereum", "http://localhost:8545")
            .build();
            
        assert_eq!(config.networks.len(), 1);
    }
}
```

### Property-Based Testing

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_deviation_calculation(
        old_value in 0.1f64..1000000.0,
        new_value in 0.1f64..1000000.0
    ) {
        let deviation = calculate_deviation(old_value, new_value);
        
        // Properties that should always hold
        assert!(deviation >= -100.0);
        assert!(!deviation.is_nan());
        assert!(!deviation.is_infinite());
    }
}
```

## Performance Testing

### Benchmarking

```rust
#[bench]
fn bench_json_extraction(b: &mut Bencher) {
    let json = json!({"data": {"nested": {"value": 123.45}}});
    let path = "data.nested.value";
    
    b.iter(|| {
        extract_value(&json, path)
    });
}
```

### Load Testing

Create a separate load test:

```rust
// tests/load_test.rs
#[tokio::test]
async fn test_concurrent_feeds() {
    let mut handles = vec![];
    
    // Spawn 100 concurrent feed monitors
    for i in 0..100 {
        let handle = tokio::spawn(async move {
            // Simulate feed monitoring
            monitor_feed(i).await
        });
        handles.push(handle);
    }
    
    // Wait for all to complete
    for handle in handles {
        handle.await.unwrap();
    }
}
```

## Test Coverage

### Measuring Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html

# With specific features
cargo tarpaulin --features "postgres" --out Html

# Exclude certain files
cargo tarpaulin --exclude-files "src/bin/*" --out Html
```

### Coverage Goals

- Unit tests: >80% coverage
- Critical paths: 100% coverage
- Error handling: All error cases tested
- Edge cases: Boundary conditions covered

## CI Integration

Tests run automatically in GitHub Actions:

```yaml
# .github/workflows/ci.yml
- name: Run tests
  run: cargo test --verbose

- name: Run clippy
  run: cargo clippy -- -D warnings

- name: Check formatting
  run: cargo fmt -- --check
```

## Testing Checklist

Before submitting a PR:

- [ ] All tests pass: `cargo test`
- [ ] No clippy warnings: `cargo clippy`
- [ ] Code is formatted: `cargo fmt`
- [ ] New features have tests
- [ ] Edge cases are tested
- [ ] Error conditions are tested
- [ ] Documentation is updated
- [ ] Integration tests pass

## Common Testing Patterns

### Testing Retries

```rust
#[tokio::test]
async fn test_retry_logic() {
    let counter = Arc::new(Mutex::new(0));
    let counter_clone = counter.clone();
    
    let _m = mockito::mock("GET", "/api")
        .with_status(500)
        .expect(2)  // Expect 2 failed attempts
        .create();
        
    let _m2 = mockito::mock("GET", "/api")
        .with_status(200)
        .with_body("success")
        .expect(1)  // Expect 1 successful attempt
        .create();
    
    // Your retry logic here
    let result = retry_with_backoff(|| fetch_data()).await;
    assert!(result.is_ok());
}
```

### Testing Time-Dependent Code

```rust
use tokio::time::{pause, advance, Duration};

#[tokio::test]
async fn test_time_based_trigger() {
    tokio::time::pause();  // Pause time
    
    let feed = start_feed_monitor();
    
    // Advance time by 1 hour
    tokio::time::advance(Duration::from_secs(3600)).await;
    
    // Check that update was triggered
    assert_eq!(feed.update_count(), 1);
}
```

## Troubleshooting Tests

### Flaky Tests

1. Check for race conditions
2. Mock external dependencies
3. Use deterministic test data
4. Avoid time-dependent assertions

### Slow Tests

1. Use test parallelization
2. Mock expensive operations
3. Use smaller test datasets
4. Profile test execution

### Test Isolation

1. Each test should set up its own data
2. Clean up resources after tests
3. Don't rely on test execution order
4. Use unique identifiers for test data

## Additional Resources

- [Rust Testing Book](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Tokio Testing](https://tokio.rs/tokio/topics/testing)
- [mockito Documentation](https://docs.rs/mockito/)
- [proptest Documentation](https://docs.rs/proptest/)