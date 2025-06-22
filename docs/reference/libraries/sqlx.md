# SQLx Documentation

## Overview

SQLx is an async, pure Rust SQL crate featuring compile-time checked queries without a DSL. It supports PostgreSQL, MySQL, MariaDB, SQLite, and MSSQL.

## Adding SQLx as a Dependency

Add SQLx to your `Cargo.toml` with your choice of runtime and TLS backend:

```toml
# Cargo.toml
[dependencies]
# PICK ONE OF THE FOLLOWING:

# tokio (no TLS)
sqlx = { version = "0.8", features = [ "runtime-tokio" ] }
# tokio + native-tls
sqlx = { version = "0.8", features = [ "runtime-tokio", "tls-native-tls" ] }
# tokio + rustls with ring
sqlx = { version = "0.8", features = [ "runtime-tokio", "tls-rustls-ring-webpki" ] }
sqlx = { version = "0.8", features = [ "runtime-tokio", "tls-rustls-ring-native-roots" ] }
# tokio + rustls with aws-lc-rs
sqlx = { version = "0.8", features = [ "runtime-tokio", "tls-rustls-aws-lc-rs" ] }

# async-std (no TLS)
sqlx = { version = "0.8", features = [ "runtime-async-std" ] }
# async-std + native-tls
sqlx = { version = "0.8", features = [ "runtime-async-std", "tls-native-tls" ] }
# async-std + rustls
sqlx = { version = "0.8", features = [ "runtime-async-std", "tls-rustls-ring-webpki" ] }
sqlx = { version = "0.8", features = [ "runtime-async-std", "tls-rustls-ring-native-roots" ] }
sqlx = { version = "0.8", features = [ "runtime-async-std", "tls-rustls-aws-lc-rs" ] }
```

### Optimization for Faster Builds

```toml
[profile.dev.package.sqlx-macros]
opt-level = 3
```

## Basic Example - PostgreSQL Connection Pool

```rust
use sqlx::postgres::PgPoolOptions;

#[async_std::main] // or #[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    // Create a connection pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://postgres:password@localhost/test").await?;

    // Make a simple query (use ? for MySQL/MariaDB, $1 for PostgreSQL)
    let row: (i64,) = sqlx::query_as("SELECT $1")
        .bind(150_i64)
        .fetch_one(&pool).await?;

    assert_eq!(row.0, 150);

    Ok(())
}
```

## Connection Management

### Connection Pools (Recommended)

```rust
// PostgreSQL
let pool = PgPool::connect("postgres://user:pass@host/database").await?;

// MySQL
let pool = MySqlPool::connect("mysql://user:pass@host/database").await?;

// SQLite
let pool = SqlitePool::connect("sqlite:todos.db").await?;
```

### Single Connections

```rust
use sqlx::Connection;

// SQLite in-memory
let conn = SqliteConnection::connect("sqlite::memory:").await?;

// With options
let conn = SqliteConnectOptions::from_str("sqlite://a.db")?
    .foreign_keys(false)
    .connect().await?;
```

### Connection Configuration

```rust
// PostgreSQL with after_connect hook
let pool = PgPoolOptions::new()
    .after_connect(|conn| Box::pin(async move {
        conn.execute("SET application_name = 'your_app';").await?;
        conn.execute("SET search_path = 'my_schema';").await?;
        Ok(())
    }))
    .connect("postgres://...").await?;

// MSSQL with builder
let conn = MssqlConnectOptions::new()
    .host("localhost")
    .database("master")
    .username("sa")
    .password("Password")
    .connect().await?;
```

## Query Execution

### Low-Level Queries

```rust
// Unprepared query
conn.execute("BEGIN").await?;

// Prepared, cached query
conn.execute(sqlx::query("DELETE FROM table")).await?;

// High-level interface
sqlx::query("DELETE FROM table").execute(&pool).await?;
```

### Fetching Results

```rust
use futures::TryStreamExt;
use sqlx::Row;

// Fetch multiple rows
let mut rows = sqlx::query("SELECT * FROM users WHERE email = ?")
    .bind(email)
    .fetch(&mut conn);

while let Some(row) = rows.try_next().await? {
    let email: &str = row.try_get("email")?;
}

// Map results
let values: Vec<i32> = sqlx::query("SELECT id FROM users")
    .map(|row: PgRow| row.get(0))
    .fetch_all(&pool).await?;
```

### Using FromRow Derive

```rust
#[derive(sqlx::FromRow)]
struct User {
    name: String,
    id: i64
}

let mut stream = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = ? OR name = ?")
    .bind(user_email)
    .bind(user_name)
    .fetch(&mut conn);
```

## Compile-Time Verification

### Using query! Macro

```rust
// Set DATABASE_URL environment variable
// export DATABASE_URL=postgres://localhost/my_database

let countries = sqlx::query!(
    "
    SELECT country, COUNT(*) as count
    FROM users
    GROUP BY country
    WHERE organization = ?
    ",
    organization
)
.fetch_all(&pool) // -> Vec<{ country: String, count: i64 }>
.await?;

// Access fields
countries[0].country
countries[0].count
```

### Using query_as! Macro

```rust
// No traits needed
struct Country { 
    country: String, 
    count: i64 
}

let countries = sqlx::query_as!(Country,
    "
    SELECT country, COUNT(*) as count
    FROM users
    GROUP BY country
    WHERE organization = ?
    ",
    organization
)
.fetch_all(&pool) // -> Vec<Country>
.await?;
```

### Offline Mode for CI/CD

```shell
# Generate query data for offline compilation
cargo sqlx prepare

# Set up git hook to auto-prepare
echo "cargo sqlx prepare > /dev/null 2>&1; git add .sqlx > /dev/null" > .git/hooks/pre-commit
```

In `build.rs`:
```rust
fn main() {
    // Enable offline mode for docs.rs
    if std::env::var_os("DOCS_RS").is_some() {
        println!("cargo:rustc-env=SQLX_OFFLINE=true");
    }
}
```

## Prepared Statements and Parameters

### Parameter Placeholders

```sql
-- MySQL
INSERT INTO Students (name) VALUES(?);
-- PostgreSQL and SQLite
INSERT INTO Students (name) VALUES($1);
```

### Array Parameters (PostgreSQL)

```rust
let foo_ids: Vec<i64> = vec![1, 2, 3];

let foos = sqlx::query!(
    "SELECT * FROM foo WHERE id = ANY($1)",
    &foo_ids[..]  // Must be a slice
)
.fetch_all(&db)
.await?;
```

### Bulk Inserts with UNNEST (PostgreSQL)

```rust
// Single column
let foo_texts: Vec<String> = vec!["a", "b", "c"];

sqlx::query!(
    "INSERT INTO foo(text_column) SELECT * FROM UNNEST($1::text[])",
    &foo_texts[..]
)
.execute(&db)
.await?;

// Multiple columns
let texts: Vec<String> = vec![/* ... */];
let bools: Vec<bool> = vec![/* ... */];
let ints: Vec<i64> = vec![/* ... */];

sqlx::query!(
    "
    INSERT INTO foo(text_column, bool_column, int_column) 
    SELECT * FROM UNNEST($1::text[], $2::bool[], $3::int8[])
    ",
    &texts[..],
    &bools[..],
    &ints[..]
)
.execute(&db)
.await?;
```

## Custom Types

### Transparent Types

```rust
#[derive(sqlx::Type)]
#[repr(transparent)]
struct Meters(i32);
```

### Enum Types

```rust
// Integer-based enum
#[derive(sqlx::Type)]
#[repr(i32)]
enum Color { Red = 1, Green = 2, Blue = 3 }

// String-based enum
#[derive(sqlx::Type)]
#[sqlx(rename = "TEXT")]
#[sqlx(rename_all = "lowercase")]
enum Color { Red, Green, Blue }  // expects 'red', 'green', 'blue'
```

### Composite Types (PostgreSQL)

```rust
#[derive(sqlx::Type)]
#[sqlx(rename = "interface_type")]
struct InterfaceType {
    name: String,
    supplier_id: i32,
    price: f64
}
```

## Transactions

```rust
// Using transaction wrapper
conn.transaction(|transaction: &mut Transaction<Database>| {
    // Your transactional operations here
});
```

## PostgreSQL Notifications

```rust
let mut listener = PgListener::new(DATABASE_URL).await?;
listener.listen("topic").await?;

loop {
    let message = listener.recv().await?;
    println!("payload = {}", message.payload);
}
```

## Getting Last Insert ID

```rust
// MySQL
let id: u64 = query!("INSERT INTO table (col) VALUES (?)", val)
    .execute(&mut conn).await?
    .last_insert_id();

// SQLite
let id: i64 = query!("INSERT INTO table (col) VALUES (?1)", val)
    .execute(&mut conn).await?
    .last_insert_rowid();
```

## SQLx CLI

### Installation

```shell
cargo install sqlx-cli
```

### Environment Setup

```shell
# Set DATABASE_URL
export DATABASE_URL="postgres://postgres:password@localhost/todos"

# Or use .env file
echo "DATABASE_URL=postgres://postgres@localhost/my_database" > .env
```

### Database Management

```shell
# Create database
sqlx db create

# Run migrations
sqlx migrate run

# Prepare for offline mode
sqlx prepare
```

## Migration Management

Create migrations in the `migrations/` directory:

```shell
migrations/
├── 20210101000000_initial_schema.sql
└── 20210102000000_add_users_table.sql
```

## Performance and Security

### SQL Injection Prevention

SQLx uses prepared statements by default, preventing SQL injection:

```rust
// Safe - uses prepared statements
let user = sqlx::query!("SELECT * FROM users WHERE id = ?", user_id)
    .fetch_one(&pool).await?;

// Never do string concatenation!
// let query = format!("SELECT * FROM users WHERE id = {}", user_id); // DANGEROUS!
```

### Connection Pool Configuration

```rust
let pool = PgPoolOptions::new()
    .max_connections(5)
    .min_connections(1)
    .connect_timeout(Duration::from_secs(3))
    .idle_timeout(Duration::from_secs(10))
    .max_lifetime(Duration::from_secs(30))
    .connect(&database_url).await?;
```

## Version Pinning

For stability with SQLite:

```toml
[dependencies]
sqlx = { version = "=0.7.0", features = ["sqlite"] }
rusqlite = "=0.29.0"
```