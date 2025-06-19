#[cfg(test)]
mod tests {
    use crate::database::models::NewFeedLog;
    use chrono::Utc;

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
}

// Integration tests would go in tests/ directory and require a test database
// Example integration test structure:
//
// #[tokio::test]
// async fn test_save_and_retrieve_feed_log() {
//     let pool = create_test_pool().await;
//     let repo = FeedLogRepository::new(pool);
//     
//     let log = NewFeedLog { ... };
//     let saved = repo.save(log).await.unwrap();
//     
//     assert!(saved.id > 0);
//     
//     let retrieved = repo.get_latest("eth_usd", "ethereum").await.unwrap();
//     assert!(retrieved.is_some());
// }