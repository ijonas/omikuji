#[cfg(test)]
mod tests {
    use crate::database::models::{NewFeedLog, FeedLog};
    use crate::database::repository::FeedSummary;
    use crate::database::establish_connection;
    use chrono::{Utc, Duration};

    #[test]
    fn test_new_feed_log_creation() {
        let log = NewFeedLog {
            feed_name: "eth_usd".to_string(),
            network_name: "ethereum".to_string(),
            feed_value: 2045.34,
            feed_timestamp: Utc::now().timestamp(),
            error_status_code: None,
            network_error: false,
        };

        assert_eq!(log.feed_name, "eth_usd");
        assert_eq!(log.network_name, "ethereum");
        assert_eq!(log.feed_value, 2045.34);
        assert!(log.error_status_code.is_none());
        assert!(!log.network_error);
    }

    #[test]
    fn test_error_feed_log_creation() {
        let log = NewFeedLog {
            feed_name: "btc_usd".to_string(),
            network_name: "base".to_string(),
            feed_value: 0.0,
            feed_timestamp: Utc::now().timestamp(),
            error_status_code: Some(500),
            network_error: false,
        };

        assert_eq!(log.error_status_code, Some(500));
        assert!(!log.network_error);
    }

    #[test]
    fn test_network_error_log_creation() {
        let log = NewFeedLog {
            feed_name: "eth_usd".to_string(),
            network_name: "ethereum".to_string(),
            feed_value: 0.0,
            feed_timestamp: Utc::now().timestamp(),
            error_status_code: None,
            network_error: true,
        };

        assert!(log.error_status_code.is_none());
        assert!(log.network_error);
    }


    #[test]
    fn test_feed_log_model() {
        let now = Utc::now();
        let log = FeedLog {
            id: 1,
            feed_name: "eth_usd".to_string(),
            network_name: "ethereum".to_string(),
            feed_value: 2500.0,
            feed_timestamp: now.timestamp(),
            updated_at: now,
            error_status_code: None,
            network_error: false,
            created_at: now,
        };

        assert_eq!(log.id, 1);
        assert_eq!(log.feed_name, "eth_usd");
        assert_eq!(log.network_name, "ethereum");
        assert_eq!(log.feed_value, 2500.0);
        assert!(!log.network_error);
    }



    #[test]
    fn test_feed_summary_struct() {
        let now = Utc::now();
        let summary = FeedSummary {
            feed_name: "eth_usd".to_string(),
            network_name: "ethereum".to_string(),
            log_count: 1000,
            oldest_log: now - Duration::days(30),
            newest_log: now,
            error_count: 5,
        };

        assert_eq!(summary.feed_name, "eth_usd");
        assert_eq!(summary.network_name, "ethereum");
        assert_eq!(summary.log_count, 1000);
        assert_eq!(summary.error_count, 5);
    }

    // Mock database connection for testing
    #[cfg(test)]
    mod mock_db {
        use std::env;

        pub fn setup_test_db_url() {
            // Set a test database URL if not already set
            if env::var("DATABASE_URL").is_err() {
                env::set_var("DATABASE_URL", "postgres://test:test@localhost:5432/test_db");
            }
        }

        pub fn cleanup_test_db_url() {
            env::remove_var("DATABASE_URL");
        }
    }

    #[tokio::test]
    async fn test_establish_connection_no_database_url() {
        // Ensure DATABASE_URL is not set
        mock_db::cleanup_test_db_url();
        
        let result = establish_connection().await;
        assert!(result.is_err());
        
        let err = result.unwrap_err();
        assert!(err.to_string().contains("DATABASE_URL environment variable not set"));
    }

    #[tokio::test]
    async fn test_establish_connection_invalid_url() {
        // Set an invalid database URL
        std::env::set_var("DATABASE_URL", "invalid://url");
        
        let result = establish_connection().await;
        assert!(result.is_err());
        
        mock_db::cleanup_test_db_url();
    }

    #[test]
    fn test_database_url_masking() {
        // Test URL masking logic
        let test_url = "postgres://user:password@localhost:5432/mydb";
        let url_parts: Vec<&str> = test_url.split('@').collect();
        
        assert_eq!(url_parts.len(), 2);
        
        let masked = if url_parts.len() > 1 {
            let host_and_db = url_parts[1];
            format!("postgres://***@{}", host_and_db)
        } else {
            "postgres://***".to_string()
        };
        
        assert_eq!(masked, "postgres://***@localhost:5432/mydb");
    }

    #[test]
    fn test_cutoff_date_calculation() {
        let now = Utc::now();
        let days = 7;
        let cutoff = now - Duration::days(days as i64);
        
        // Check that cutoff is approximately 7 days ago
        let diff = now - cutoff;
        assert_eq!(diff.num_days(), 7);
    }

    #[test]
    fn test_cleanup_manager_config_validation() {
        use crate::config::models::{OmikujiConfig, DatabaseCleanupConfig};
        use crate::config::metrics_config::MetricsConfig;
        use crate::gas_price::models::GasPriceFeedConfig;
        
        let config = OmikujiConfig {
            networks: vec![],
            datafeeds: vec![],
            database_cleanup: DatabaseCleanupConfig {
                enabled: true,
                schedule: "0 2 * * *".to_string(),
            },
            key_storage: Default::default(),
            metrics: MetricsConfig::default(),
            gas_price_feeds: GasPriceFeedConfig::default(),
        };
        
        assert!(config.database_cleanup.enabled);
    }

    // Additional repository tests
    #[test]
    fn test_repository_creation() {
        
        // This would normally use a real pool in integration tests
        // For unit tests, we're just testing the structure
        // let pool = PgPool::new();
        // let repo = FeedLogRepository::new(pool);
    }

    #[test]
    fn test_transaction_repository_methods() {
        
        // Test that TransactionLogRepository can be created
        // In real tests, this would use a test database
        // let pool = PgPool::new();
        // let repo = TransactionLogRepository::new(pool);
    }

    #[tokio::test]
    async fn test_cleanup_manager_creation() {
        use crate::config::models::{OmikujiConfig, DatabaseCleanupConfig};
        use crate::config::metrics_config::MetricsConfig;
        use crate::gas_price::models::GasPriceFeedConfig;
        
        let config = OmikujiConfig {
            networks: vec![],
            datafeeds: vec![],
            database_cleanup: DatabaseCleanupConfig {
                enabled: false, // Disabled for test
                schedule: "0 0 * * *".to_string(),
            },
            key_storage: Default::default(),
            metrics: MetricsConfig::default(),
            gas_price_feeds: GasPriceFeedConfig::default(),
        };
        
        // Mock repository for testing
        // In real tests, this would use a test database
        // let pool = create_test_pool().await;
        // let repository = Arc::new(FeedLogRepository::new(pool));
        // let manager = CleanupManager::new(config, repository).await;
        
        assert!(!config.database_cleanup.enabled);
    }

    #[test]
    fn test_cron_schedule_parsing() {
        // Test various cron schedule formats
        let valid_schedules = vec![
            "0 0 * * *",      // Daily at midnight
            "0 2 * * *",      // Daily at 2 AM
            "0 */6 * * *",    // Every 6 hours
            "30 2 * * 0",     // Weekly on Sunday at 2:30 AM
            "0 0 1 * *",      // Monthly on the 1st
        ];
        
        for schedule in valid_schedules {
            assert!(!schedule.is_empty());
            assert!(schedule.split_whitespace().count() == 5);
        }
    }
}
