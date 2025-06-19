use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::env;
use anyhow::{Context, Result};
use tracing::info;

pub type DatabasePool = Pool<Postgres>;

/// Establishes a connection pool to the PostgreSQL database
pub async fn establish_connection() -> Result<DatabasePool> {
    // Try to get database URL from environment
    let database_url = env::var("DATABASE_URL")
        .context("DATABASE_URL environment variable not set")?;
    
    info!("Connecting to PostgreSQL database");
    
    // Create connection pool with sensible defaults
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .min_connections(2)
        .connect(&database_url)
        .await
        .context("Failed to create PostgreSQL connection pool")?;
    
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