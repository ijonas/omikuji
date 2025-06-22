# Serde Documentation

## Overview

Serde is a framework for serializing and deserializing Rust data structures efficiently and generically. It provides a powerful way to convert Rust structs and enums to and from various data formats like JSON, YAML, TOML, and many others.

## Adding Serde as a Dependency

Add Serde to your `Cargo.toml`:

```toml
[dependencies]

# The core APIs, including the Serialize and Deserialize traits. Always
# required when using Serde. The "derive" feature is only required when
# using #[derive(Serialize, Deserialize)] to make Serde work with structs
# and enums defined in your crate.
serde = { version = "1.0", features = ["derive"] }

# Each data format lives in its own crate; the sample code below uses JSON
# but you may be using a different one.
serde_json = "1.0"
```

## Basic Example - JSON Serialization/Deserialization

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct Point {
    x: i32,
    y: i32,
}

fn main() {
    let point = Point { x: 1, y: 2 };

    // Convert the Point to a JSON string.
    let serialized = serde_json::to_string(&point).unwrap();

    // Prints serialized = {"x":1,"y":2}
    println!("serialized = {}", serialized);

    // Convert the JSON string back to a Point.
    let deserialized: Point = serde_json::from_str(&serialized).unwrap();

    // Prints deserialized = Point { x: 1, y: 2 }
    println!("deserialized = {:?}", deserialized);
}
```

## Core Concepts

### Derive Macros

The most common way to use Serde is with the derive macros:

- `#[derive(Serialize)]` - Automatically implements serialization for your type
- `#[derive(Deserialize)]` - Automatically implements deserialization for your type
- You can derive both traits at once: `#[derive(Serialize, Deserialize)]`

### Common Attributes

#### Field Attributes

```rust
#[derive(Serialize, Deserialize)]
struct User {
    #[serde(rename = "userName")]
    username: String,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    
    #[serde(default)]
    active: bool,
    
    #[serde(skip)]
    internal_id: u64,
}
```

#### Container Attributes

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    max_connections: u32,
    timeout_seconds: u64,
    enable_logging: bool,
}
```

## Working with Different Formats

### JSON (using serde_json)

```rust
// Serialize to JSON
let json = serde_json::to_string(&data)?;
let json_pretty = serde_json::to_string_pretty(&data)?;

// Deserialize from JSON
let data: MyStruct = serde_json::from_str(&json)?;

// Work with JSON values
let value = serde_json::json!({
    "name": "John Doe",
    "age": 30,
    "emails": ["john@example.com"]
});
```

### Other Formats

Add the appropriate crate for your format:

- YAML: `serde_yaml`
- TOML: `toml`
- MessagePack: `rmp-serde`
- CBOR: `serde_cbor`
- CSV: `csv` (with serde support)

## Advanced Usage

### Custom Serialization

```rust
use serde::{Serialize, Serializer};

#[derive(Serialize)]
struct MyStruct {
    #[serde(serialize_with = "serialize_as_string")]
    number: u64,
}

fn serialize_as_string<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}
```

### Enums

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum Message {
    Request { id: u64, method: String },
    Response { id: u64, result: String },
}

// This will serialize to:
// {"type": "Request", "id": 1, "method": "getData"}
```

### Generic Types

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Wrapper<T> {
    value: T,
    metadata: String,
}

// Works with any T that implements Serialize/Deserialize
let wrapped = Wrapper {
    value: 42,
    metadata: "test".to_string(),
};
```

### Flattening

```rust
#[derive(Serialize, Deserialize)]
struct Person {
    name: String,
    #[serde(flatten)]
    address: Address,
}

#[derive(Serialize, Deserialize)]
struct Address {
    street: String,
    city: String,
}

// Serializes to flat structure:
// {"name": "John", "street": "123 Main St", "city": "Boston"}
```

## Error Handling

```rust
use serde_json::Error;

fn process_json(json_str: &str) -> Result<(), Error> {
    let data: MyStruct = serde_json::from_str(json_str)?;
    // Process data...
    Ok(())
}
```

## Testing

Run tests in the Serde repository:

```sh
# Run documentation example tests
cargo test --features derive

# Run full test suite (requires nightly)
cargo +nightly test --features unstable
```

## Performance Tips

1. Use `&str` instead of `String` when possible in deserialization
2. Consider using `serde_json::from_reader` for large files
3. Use `#[serde(borrow)]` for zero-copy deserialization when appropriate
4. Profile your serialization/deserialization if performance is critical

## Common Patterns

### Configuration Files

```rust
use serde::Deserialize;
use std::fs;

#[derive(Deserialize)]
struct Config {
    database_url: String,
    port: u16,
    debug: bool,
}

fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string("config.toml")?;
    let config: Config = toml::from_str(&contents)?;
    Ok(config)
}
```

### API Responses

```rust
#[derive(Serialize, Deserialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }
    
    fn error(msg: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg),
        }
    }
}
```