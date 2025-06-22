use anyhow::{Context, Result};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::env;
use tracing::{debug, info};

pub type DatabasePool = Pool<Postgres>;

/// Establishes a connection pool to the PostgreSQL database
pub async fn establish_connection() -> Result<DatabasePool> {
    // Try to get database URL from environment
    let database_url =
        env::var("DATABASE_URL").context("DATABASE_URL environment variable not set")?;

    // Parse the database URL to extract connection details
    let url_parts: Vec<&str> = database_url.split('@').collect();
    let db_info = if url_parts.len() > 1 {
        let host_and_db = url_parts[1];
        // Mask sensitive parts of the URL for logging
        format!("postgres://***@{}", host_and_db)
    } else {
        "postgres://***".to_string()
    };

    debug!("Attempting to connect to database: {}", db_info);
    info!("Connecting to PostgreSQL database");

    // Create connection pool with sensible defaults
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .min_connections(2)
        .connect(&database_url)
        .await
        .context("Failed to create PostgreSQL connection pool")?;

    // Test the connection and get some info
    let (version,): (String,) = sqlx::query_as("SELECT version()")
        .fetch_one(&pool)
        .await
        .context("Failed to verify database connection")?;

    debug!("Connected to database. PostgreSQL version: {}", version);
    debug!("Connection pool created with max_connections=10, min_connections=2");
    info!("Successfully connected to PostgreSQL database");

    Ok(pool)
}

/// Run pending migrations
pub async fn run_migrations(pool: &DatabasePool) -> Result<()> {
    info!("Running database migrations");

    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("Failed to run database migrations")?;

    info!("Database migrations completed successfully");

    Ok(())
}
