# Reqwest Documentation

## Overview

Reqwest is an ergonomic, batteries-included HTTP client for Rust. It provides both async and blocking APIs for making HTTP requests with features like JSON support, form data, multipart uploads, and more.

## Adding Reqwest as a Dependency

Add reqwest to your `Cargo.toml`:

```toml
[dependencies]
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["full"] }
```

## Basic Example - Async HTTP GET with JSON

```rust
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resp = reqwest::get("https://httpbin.org/ip")
        .await?
        .json::<HashMap<String, String>>()
        .await?;
    println!("{resp:#?}");
    Ok(())
}
```

## Authentication

### Bearer Token Authentication

```rust
RequestBuilder::bearer_auth(token)
```

Example:
```rust
let response = client
    .get("https://api.example.com/protected")
    .bearer_auth("your-token-here")
    .send()
    .await?;
```

## Client Configuration

### TLS Certificate Verification

To disable certificate verification (use with caution):

```rust
ClientBuilder::danger_accept_invalid_certs(bool)
```

Example:
```rust
let client = reqwest::Client::builder()
    .danger_accept_invalid_certs(true)
    .build()?;
```

### Builder Pattern Changes

Note that `ClientBuilder` now uses a by-value builder pattern:

```rust
// Current pattern (by-value)
let mut builder = ClientBuilder::new();
if some_val {
    builder = builder.gzip(false);
}
let client = builder.build()?;
```

## Headers

### Setting Headers

```rust
// Using string headers
client
    .get("https://hyper.rs")
    .header("user-agent", "hallo")
    .send()?;
```

### Using Hyper 0.11 Headers (with feature flag)

For backwards compatibility:

```rust
client
    .get("https://hyper.rs")
    .header_011(reqwest::hyper_011::header::UserAgent::new("hallo"))
    .send()?
```

## Multipart Forms

### Setting MIME Type for Multipart Parts

```rust
let part = multipart::Part::file(path)?
    .mime_str("text/plain")?
```

## WASM Support

Reqwest supports WebAssembly targets. To run WASM examples:

```shell
# Install dependencies
npm install

# Start development server
npm run serve
```

## Common Use Cases

### GET Request with Query Parameters

```rust
let params = [("key", "value"), ("foo", "bar")];
let response = client
    .get("https://httpbin.org/get")
    .query(&params)
    .send()
    .await?;
```

### POST Request with JSON Body

```rust
use serde_json::json;

let response = client
    .post("https://httpbin.org/post")
    .json(&json!({
        "name": "John Doe",
        "age": 30
    }))
    .send()
    .await?;
```

### Custom Headers

```rust
use reqwest::header;

let mut headers = header::HeaderMap::new();
headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("application/json"));
headers.insert("X-Custom-Header", header::HeaderValue::from_static("custom-value"));

let client = reqwest::Client::builder()
    .default_headers(headers)
    .build()?;
```

### Timeout Configuration

```rust
use std::time::Duration;

let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(10))
    .build()?;
```

### Form Data

```rust
let params = [("username", "alice"), ("password", "secret")];
let response = client
    .post("https://httpbin.org/post")
    .form(&params)
    .send()
    .await?;
```

### Error Handling

```rust
match reqwest::get("https://httpbin.org/status/404").await {
    Ok(response) => {
        if response.status().is_success() {
            println!("Success!");
        } else {
            println!("HTTP Error: {}", response.status());
        }
    }
    Err(e) => {
        if e.is_timeout() {
            println!("Request timed out");
        } else if e.is_connect() {
            println!("Connection error");
        } else {
            println!("Other error: {}", e);
        }
    }
}
```