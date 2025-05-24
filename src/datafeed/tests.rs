#[cfg(test)]
mod tests {
    use serde_json::json;
    
    mod json_extractor_tests {
        use super::*;
        use crate::datafeed::json_extractor::JsonExtractor;
        
        #[test]
        fn test_extract_float_from_nested_json() {
            let json = json!({
                "RAW": {
                    "ETH": {
                        "USD": {
                            "PRICE": 2045.34,
                            "LASTUPDATE": 1748068861
                        }
                    }
                }
            });
            
            let result = JsonExtractor::extract_float(&json, "RAW.ETH.USD.PRICE");
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 2045.34);
        }
        
        #[test]
        fn test_extract_float_from_string() {
            let json = json!({
                "data": {
                    "price": "1234.56"
                }
            });
            
            let result = JsonExtractor::extract_float(&json, "data.price");
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 1234.56);
        }
        
        #[test]
        fn test_extract_float_missing_path() {
            let json = json!({
                "RAW": {
                    "ETH": {}
                }
            });
            
            let result = JsonExtractor::extract_float(&json, "RAW.ETH.USD.PRICE");
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Failed to extract path component 'USD'"));
        }
        
        #[test]
        fn test_extract_float_wrong_type() {
            let json = json!({
                "value": ["array", "not", "number"]
            });
            
            let result = JsonExtractor::extract_float(&json, "value");
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("is not a number or string"));
        }
        
        #[test]
        fn test_extract_timestamp_from_path() {
            let json = json!({
                "RAW": {
                    "ETH": {
                        "USD": {
                            "LASTUPDATE": 1748068861
                        }
                    }
                }
            });
            
            let result = JsonExtractor::extract_timestamp(&json, Some("RAW.ETH.USD.LASTUPDATE"));
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 1748068861);
        }
        
        #[test]
        fn test_extract_timestamp_current_time() {
            let json = json!({});
            
            let result = JsonExtractor::extract_timestamp(&json, None);
            assert!(result.is_ok());
            
            // Check that timestamp is recent (within last minute)
            let timestamp = result.unwrap();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            assert!(timestamp <= now);
            assert!(timestamp > now - 60); // Within last minute
        }
        
        #[test]
        fn test_extract_feed_data() {
            let json = json!({
                "RAW": {
                    "BTC": {
                        "USD": {
                            "PRICE": 108245.90,
                            "LASTUPDATE": 1748071295
                        }
                    }
                }
            });
            
            let result = JsonExtractor::extract_feed_data(
                &json,
                "RAW.BTC.USD.PRICE",
                Some("RAW.BTC.USD.LASTUPDATE")
            );
            
            assert!(result.is_ok());
            let (value, timestamp) = result.unwrap();
            assert_eq!(value, 108245.90);
            assert_eq!(timestamp, 1748071295);
        }
        
        #[test]
        fn test_single_level_path() {
            let json = json!({
                "price": 42.0
            });
            
            let result = JsonExtractor::extract_float(&json, "price");
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 42.0);
        }
        
        #[test]
        fn test_empty_path() {
            let json = json!(123.45);
            
            let result = JsonExtractor::extract_float(&json, "");
            assert!(result.is_err()); // Empty path should fail
        }
    }
    
    mod fetcher_tests {
        use crate::datafeed::fetcher::Fetcher;
        use mockito;
        
        #[tokio::test]
        async fn test_fetch_json_success() {
            let mut server = mockito::Server::new_async().await;
            let mock = server.mock("GET", "/api/data")
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(r#"{"result": "success", "value": 123.45}"#)
                .expect(1)
                .create_async()
                .await;
            
            let fetcher = Fetcher::new();
            let url = format!("{}/api/data", server.url());
            let result = fetcher.fetch_json(&url).await;
            
            assert!(result.is_ok());
            let json = result.unwrap();
            assert_eq!(json["result"], "success");
            assert_eq!(json["value"], 123.45);
            
            mock.assert_async().await;
        }
        
        #[tokio::test]
        async fn test_fetch_json_http_error() {
            let mut server = mockito::Server::new_async().await;
            let mock = server.mock("GET", "/api/error")
                .with_status(500)
                .with_body("Internal Server Error")
                .expect(1)
                .create_async()
                .await;
            
            let fetcher = Fetcher::new();
            let url = format!("{}/api/error", server.url());
            let result = fetcher.fetch_json(&url).await;
            
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("HTTP request failed with status: 500"));
            
            mock.assert_async().await;
        }
        
        #[tokio::test]
        async fn test_fetch_json_invalid_json() {
            let mut server = mockito::Server::new_async().await;
            let mock = server.mock("GET", "/api/invalid")
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body("not valid json")
                .expect(1)
                .create_async()
                .await;
            
            let fetcher = Fetcher::new();
            let url = format!("{}/api/invalid", server.url());
            let result = fetcher.fetch_json(&url).await;
            
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Failed to parse response as JSON"));
            
            mock.assert_async().await;
        }
        
        #[tokio::test]
        async fn test_fetch_json_connection_error() {
            let fetcher = Fetcher::new();
            // Use an invalid URL that will fail to connect
            let result = fetcher.fetch_json("http://localhost:99999/nonexistent").await;
            
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Failed to fetch from URL"));
        }
    }
    
    mod monitor_tests {
        use crate::datafeed::monitor::FeedMonitor;
        use crate::datafeed::fetcher::Fetcher;
        use crate::config::models::Datafeed;
        use std::sync::Arc;
        
        fn create_test_datafeed() -> Datafeed {
            Datafeed {
                name: "test_feed".to_string(),
                networks: "test".to_string(),
                check_frequency: 1, // 1 second for testing
                contract_address: "0x1234567890123456789012345678901234567890".to_string(),
                contract_type: "fluxmon".to_string(),
                read_contract_config: false,
                decimals: Some(8),
                min_value: Some(0),
                max_value: Some(1000000),
                minimum_update_frequency: 60,
                deviation_threshold_pct: 0.5,
                feed_url: "http://example.com/api".to_string(),
                feed_json_path: "data.price".to_string(),
                feed_json_path_timestamp: Some("data.timestamp".to_string()),
            }
        }
        
        #[test]
        fn test_monitor_creation() {
            let datafeed = create_test_datafeed();
            let fetcher = Arc::new(Fetcher::new());
            let monitor = FeedMonitor::new(datafeed.clone(), fetcher);
            
            // Just verify it can be created
            assert_eq!(monitor.datafeed.name, "test_feed");
        }
    }
    
    mod integration_tests {
        use super::*;
        use crate::datafeed::json_extractor::JsonExtractor;
        
        #[test]
        fn test_cryptocompare_api_format() {
            // Test with actual CryptoCompare API response format
            let json = json!({
                "RAW": {
                    "ETH": {
                        "USD": {
                            "TYPE": "5",
                            "MARKET": "CCCAGG",
                            "FROMSYMBOL": "ETH",
                            "TOSYMBOL": "USD",
                            "FLAGS": "2049",
                            "PRICE": 2557.96,
                            "LASTUPDATE": 1748071295,
                            "MEDIAN": 2558.11,
                            "LASTVOLUME": 0.01204,
                            "LASTVOLUMETO": 30.8076472,
                            "LASTTRADEID": "253426159",
                            "VOLUMEDAY": 151981.45,
                            "VOLUMEDAYTO": 389123456.78,
                            "VOLUME24HOUR": 234567.89
                        }
                    }
                }
            });
            
            let result = JsonExtractor::extract_feed_data(
                &json,
                "RAW.ETH.USD.PRICE",
                Some("RAW.ETH.USD.LASTUPDATE")
            );
            
            assert!(result.is_ok());
            let (price, timestamp) = result.unwrap();
            assert_eq!(price, 2557.96);
            assert_eq!(timestamp, 1748071295);
        }
    }
}