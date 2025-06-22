# Tokio Documentation

## Overview

Tokio is a runtime for writing reliable asynchronous applications with Rust. It provides I/O, networking, scheduling, timers, and more.

## Adding Tokio as a Dependency

To use Tokio in your project, add it to your `Cargo.toml`:

```toml
[dependencies]
tokio = { version = "1.45.1", features = ["full"] }
```

For LTS releases with only patch updates:
```toml
tokio = { version = "~1.38", features = [...] }
```

## Basic TCP Echo Server Example

Here's a simple TCP echo server that demonstrates Tokio's async networking capabilities:

```rust
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;

    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            let mut buf = [0; 1024];

            // In a loop, read data from the socket and write the data back.
            loop {
                let n = match socket.read(&mut buf).await {
                    // socket closed
                    Ok(0) => return,
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("failed to read from socket; err = {:?}", e);
                        return;
                    }
                };

                // Write the data back
                if let Err(e) = socket.write_all(&buf[0..n]).await {
                    eprintln!("failed to write to socket; err = {:?}", e);
                    return;
                }
            }
        });
    }
}
```

## Common Development Commands

### Building and Testing

```bash
# Build with all features
cargo build --all-features

# Check code without building
cargo check --all-features

# Run all tests
cargo test --all-features

# Run specific feature tests
cargo test --features rt
cargo test --features full

# Run examples
cargo run --example $name
```

### Code Quality

```bash
# Check formatting (Mac/Linux)
rustfmt --check --edition 2021 $(git ls-files '*.rs')

# Check formatting (PowerShell)
Get-ChildItem . -Filter "*.rs" -Recurse | foreach { rustfmt --check --edition 2021 $_.FullName }

# Run clippy with specific Rust version
cargo +1.77 clippy --all --tests --all-features

# Spellcheck
cargo install --locked cargo-spellcheck
cargo spellcheck check
```

### Documentation

```bash
# Generate docs.rs-equivalent documentation
RUSTDOCFLAGS="--cfg docsrs --cfg tokio_unstable" RUSTFLAGS="--cfg docsrs --cfg tokio_unstable" cargo +nightly doc --all-features [--open]

# Or using cargo-docs-rs
cargo install --locked cargo-docs-rs
cargo +nightly docs-rs [--open]
```

### Advanced Testing

```bash
# Run Miri for undefined behavior detection
MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-strict-provenance -Zmiri-retag-fields" \
    cargo +nightly miri test --features full --lib --tests

# Run Loom concurrency tests
cd tokio
LOOM_MAX_PREEMPTIONS=1 LOOM_MAX_BRANCHES=10000 RUSTFLAGS="--cfg loom -C debug_assertions" \
    cargo test --lib --release --features full -- --test-threads=1 --nocapture

# Fuzz testing
cargo install --locked cargo-fuzz
cargo fuzz list
cargo fuzz run fuzz_linked_list

# Run specific tests with unstable features
RUSTFLAGS="--cfg tokio_unstable" cargo test -p tokio --all-features --test rt_metrics
```

### Benchmarking

```bash
cd benches

# Run all benchmarks
cargo bench

# Run specific benchmark file
cargo bench --bench fs

# Run specific benchmark
cargo bench async_read_buf
```

## Internal Architecture

### Registration API

The internal Registration API manages I/O resource readiness:

```rust
struct Registration { ... }

struct ReadyEvent {
    tick: u32,
    ready: mio::Ready,
}

impl Registration {
    pub fn new<T>(io: &T, interest: mio::Ready) -> io::Result<Registration>
        where T: mio::Evented;

    async fn readiness(&self, interest: mio::Ready) -> io::Result<ReadyEvent>;
    async fn clear_readiness(&self, ready_event: ReadyEvent);
}
```

### Async Read Implementation Pattern

```rust
async fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
    loop {
        // Await readiness
        let event = self.readiness(interest).await?;

        match self.mio_socket.read(buf) {
            Ok(v) => return Ok(v),
            Err(ref e) if e.kind() == WouldBlock => {
                self.clear_readiness(event);
            }
            Err(e) => return Err(e),
        }
    }
}
```

### TcpStream by_ref Pattern

For concurrent access to TcpStream:

```rust
let rd = my_stream.by_ref();
let wr = my_stream.by_ref();

select! {
    // use `rd` and `wr` in separate branches.
}

let arc_stream = Arc::new(my_tcp_stream);
let n = arc_stream.by_ref().read(buf).await?;
```

## Release Process

```bash
# Dry run to verify publish readiness
bin/publish --dry-run <CRATE NAME> <CRATE VERSION>

# Actual release
bin/publish <NAME OF CRATE> <VERSION>
```

## Git Commit Message Format

```
module: explain the commit in one line

Body of commit message is a few lines of text, explaining things
in more detail, possibly giving some background about the issue
being fixed, etc.

The body of the commit message can be several paragraphs, and
please do proper word-wrap and keep columns shorter than about
72 characters or so. That way, `git log` will show things
nicely even when it is indented.

Fixes: #1337
Refs: #453, #154
```

## Target Specifications

For CI testing without AtomicU64:

```bash
rustc +nightly -Z unstable-options --print target-spec-json --target i686-unknown-linux-gnu | grep -v 'is-builtin' | sed 's/"max-atomic-width": 64/"max-atomic-width": 32/' > target-specs/i686-unknown-linux-gnu.json
```