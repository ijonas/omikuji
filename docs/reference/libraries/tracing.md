# Tracing Documentation

## Overview

Tracing is a framework for instrumenting Rust programs to collect structured, event-based diagnostic information. It provides a powerful way to understand program behavior through spans (periods of time) and events (points in time).

## Adding Tracing as a Dependency

Add tracing to your `Cargo.toml`:

```toml
[dependencies]
tracing = "0.1"
tracing-subscriber = "0.3"
```

For async instrumentation:
```toml
[dependencies]
tracing-attributes = "0.1.11"
```

## Basic Usage

### Simple Span and Event

```rust
use tracing::{info, span, Level};

fn main() {
    let span = span!(Level::INFO, "my_span");
    let _enter = span.enter();

    info!("This is an info message inside my_span");
}
```

### Setting Up a Global Subscriber

```rust
use tracing::info;
use tracing_subscriber;

fn main() {
    // Install global subscriber configured based on RUST_LOG env var
    tracing_subscriber::fmt::init();

    let number_of_yaks = 3;
    info!(number_of_yaks, "preparing to shave yaks");

    let number_shaved = yak_shave::shave_all(number_of_yaks);
    info!(
        all_yaks_shaved = number_shaved == number_of_yaks,
        "yak shaving completed."
    );
}
```

### Custom Subscriber Configuration

```rust
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    info!("This will be logged to stdout");
}
```

## Instrumenting Functions

### Using #[instrument] Attribute

The simplest way to add tracing to functions:

```rust
use tracing::instrument;

#[instrument]
pub fn my_function(my_arg: usize) {
    // This event will be recorded inside a span named `my_function` with the
    // field `my_arg`.
    tracing::info!("inside my_function!");
}
```

### Manual Instrumentation

```rust
use std::{error::Error, io};
use tracing::{debug, error, info, span, warn, Level};

#[tracing::instrument]
pub fn shave(yak: usize) -> Result<(), Box<dyn Error + 'static>> {
    debug!(excitement = "yay!", "hello! I'm gonna shave a yak.");
    
    if yak == 3 {
        warn!("could not locate yak!");
        return Err(io::Error::new(io::ErrorKind::Other, "shaving yak failed!").into());
    } else {
        debug!("yak shaved successfully");
    }
    Ok(())
}

pub fn shave_all(yaks: usize) -> usize {
    let span = span!(Level::TRACE, "shaving_yaks", yaks);
    let _enter = span.enter();

    info!("shaving yaks");

    let mut yaks_shaved = 0;
    for yak in 1..=yaks {
        let res = shave(yak);
        debug!(yak, shaved = res.is_ok());

        if let Err(ref error) = res {
            error!(yak, error = error.as_ref(), "failed to shave yak!");
        } else {
            yaks_shaved += 1;
        }
        debug!(yaks_shaved);
    }

    yaks_shaved
}
```

## Async Instrumentation

### Correct Way - Using #[instrument]

```rust
use tracing::{info, instrument};
use tokio::{io::AsyncWriteExt, net::TcpStream};
use std::io;

#[instrument]
async fn write(stream: &mut TcpStream) -> io::Result<usize> {
    let result = stream.write(b"hello world\n").await;
    info!("wrote to stream; success={:?}", result.is_ok());
    result
}
```

### Correct Way - Using Future::instrument

```rust
use tracing::Instrument;

let my_future = async {
    // async work here
};

my_future
    .instrument(tracing::info_span!("my_future"))
    .await
```

### Incorrect Pattern - Avoid This

```rust
// DON'T DO THIS - incorrect for async code
async {
    let _s = span.enter();
    // ...
}
```

## Working with Spans

### Creating and Entering Spans

```rust
use tracing::{span, Level};

// Construct a new span
let mut span = span!(Level::INFO, "my span");

// Enter the span for a specific scope
span.in_scope(|| {
    // Any trace events in this closure will occur within the span
});
// Dropping the span will close it
```

## Events

### Creating Events

```rust
use tracing::{event, Level};

event!(Level::INFO, "something has happened!");

// Using convenience macros
tracing::info!("informational message");
tracing::debug!("debug message");
tracing::warn!("warning message");
tracing::error!("error message");
```

## Local Scope Subscribers

```rust
use tracing::{info, Level};
use tracing_subscriber;

fn main() {
    let collector = tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::collect::with_default(collector, || {
        info!("This will be logged to stdout");
    });
    
    info!("This will _not_ be logged to stdout");
}
```

## File Appenders

### Rolling File Appender

```rust
fn main() {
    let file_appender = tracing_appender::rolling::hourly("/some/directory", "prefix.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .init();
}
```

### Non-Blocking Console Output

```rust
fn main() {
    let (non_blocking, _guard) = tracing_appender::non_blocking(std::io::stdout());
    
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .init();
}
```

## Error Handling with SpanTrace

### Setup ErrorSubscriber

```rust
use tracing_error::ErrorSubscriber;
use tracing_subscriber::prelude::*;

fn main() {
    let subscriber = tracing_subscriber::Registry::default()
        .with(ErrorSubscriber::default());

    tracing::subscriber::set_global_default(subscriber);
}
```

### Custom Error with SpanTrace

```rust
use std::{fmt, error::Error};
use tracing_error::SpanTrace;

#[derive(Debug)]
pub struct MyError {
    context: SpanTrace,
    // other fields...
}

impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // format error message
        self.context.fmt(f)?;
        Ok(())
    }
}

impl Error for MyError {}

impl MyError {
    pub fn new() -> Self {
        Self {
            context: SpanTrace::capture(),
        }
    }
}
```

### Using in_current_span

```rust
use tracing_error::prelude::*;

std::fs::read_to_string("myfile.txt").in_current_span()?;
```

## Flame Graphs

### Setup FlameLayer

```rust
use tracing_flame::FlameLayer;
use tracing_subscriber::{prelude::*, fmt};

fn setup_global_subscriber() -> impl Drop {
    let fmt_layer = fmt::Layer::default();
    let (flame_layer, _guard) = FlameLayer::with_file("./tracing.folded").unwrap();

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(flame_layer)
        .init();
    
    _guard
}
```

### Generate Flame Graph

```shell
# Install inferno
cargo install inferno

# Generate flamegraph
cat tracing.folded | inferno-flamegraph > tracing-flamegraph.svg

# Generate flamechart
cat tracing.folded | inferno-flamegraph --flamechart > tracing-flamechart.svg
```

## Testing with tracing-mock

```rust
use tracing::collect::with_default;
use tracing_mock::{collector, expect};

#[tracing::instrument]
fn yak_shaving(number_of_yaks: u32) {
    tracing::info!(number_of_yaks, "preparing to shave yaks");
}

let yak_count: u32 = 3;
let span = expect::span().named("yak_shaving");

let (collector, handle) = collector::mock()
    .new_span(
        span.clone()
            .with_fields(expect::field("number_of_yaks").with_value(&yak_count).only()),
    )
    .enter(span.clone())
    .event(
        expect::event().with_fields(
            expect::field("number_of_yaks")
                .with_value(&yak_count)
                .and(expect::msg("preparing to shave yaks"))
                .only(),
        ),
    )
    .exit(span.clone())
    .only()
    .run_with_handle();

with_default(collector, || {
    yak_shaving(yak_count);
});

handle.assert_finished();
```

## Serialization with tracing-serde

```toml
[dependencies]
tracing = "0.1"
tracing-serde = "0.1"
```

```rust
use tracing_serde::AsSerde;

pub struct JsonSubscriber {
    next_id: AtomicUsize,
}

impl Subscriber for JsonSubscriber {
    fn new_span(&self, attrs: &Attributes) -> Id {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let id = Id::from_u64(id as u64);
        let json = json!({
            "new_span": {
                "attributes": attrs.as_serde(),
                "id": id.as_serde(),
            }
        });
        println!("{}", json);
        id
    }
}
```

## no_std Support

For `no_std` environments (requires `liballoc`):

```toml
[dependencies]
tracing-core = { version = "0.1.17", default-features = false }
tracing-serde = { version = "0.2", default-features = false }
```

## Best Practices

1. **Use #[instrument] for async functions** - It's the most ergonomic way
2. **Keep spans focused** - Create spans for logical units of work
3. **Use structured fields** - Pass data as fields rather than formatting into messages
4. **Set appropriate levels** - Use TRACE for very detailed info, DEBUG for debugging, INFO for general info, WARN for warnings, ERROR for errors
5. **Don't hold span guards across await points** - Use `.instrument()` instead
6. **Use non-blocking appenders** - For file I/O to avoid blocking your application
7. **Keep the _guard alive** - When using non-blocking appenders, keep the guard to ensure logs are flushed

## Environment Variables

- `RUST_LOG` - Controls log level when using `tracing_subscriber::fmt::init()`
  - Examples: `RUST_LOG=debug`, `RUST_LOG=myapp=debug,other_crate=warn`

## Building Documentation

```bash
RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --no-deps
```