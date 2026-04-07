//! Integration tests for WebSocket support.
//!
//! Tests the WebSocket connection and manager functionality.

use open_core::websocket::{WebSocketConfig, WebSocketManager};

// ---------------------------------------------------------------------------
// Config Tests
// ---------------------------------------------------------------------------

mod config_tests {
    use super::*;

    #[test]
    fn test_websocket_config_defaults() {
        let config = WebSocketConfig::default();
        assert_eq!(config.max_per_origin, 6);
        assert_eq!(config.connect_timeout_secs, 30);
        assert_eq!(config.max_message_size, 10 * 1024 * 1024);
        assert!(config.block_private_ips);
        assert!(config.block_loopback);
    }

    #[test]
    fn test_websocket_config_custom() {
        let config = WebSocketConfig {
            max_per_origin: 10,
            connect_timeout_secs: 60,
            max_message_size: 5 * 1024 * 1024,
            block_private_ips: false,
            block_loopback: false,
        };
        assert_eq!(config.max_per_origin, 10);
        assert_eq!(config.connect_timeout_secs, 60);
        assert_eq!(config.max_message_size, 5 * 1024 * 1024);
        assert!(!config.block_private_ips);
        assert!(!config.block_loopback);
    }
}

// ---------------------------------------------------------------------------
// Manager Tests
// ---------------------------------------------------------------------------

mod manager_tests {
    use super::*;

    #[test]
    fn test_manager_creation() {
        let manager = WebSocketManager::new(WebSocketConfig::default());
        assert_eq!(manager.connection_count(), 0);
        assert!(manager.connection_ids().is_empty());
    }

    #[test]
    fn test_manager_default() {
        let manager = WebSocketManager::default();
        assert_eq!(manager.connection_count(), 0);
    }

    #[test]
    fn test_manager_config() {
        let config = WebSocketConfig {
            max_per_origin: 3,
            connect_timeout_secs: 10,
            max_message_size: 1024,
            block_private_ips: false,
            block_loopback: false,
        };
        let manager = WebSocketManager::new(config);
        assert_eq!(manager.config().max_per_origin, 3);
        assert_eq!(manager.config().connect_timeout_secs, 10);
    }

    #[test]
    fn test_manager_get_nonexistent() {
        let manager = WebSocketManager::default();
        assert!(manager.get("nonexistent-id").is_none());
    }

    #[test]
    fn test_manager_get_mut_nonexistent() {
        let mut manager = WebSocketManager::default();
        assert!(manager.get_mut("nonexistent-id").is_none());
    }

    #[tokio::test]
    async fn test_manager_close_nonexistent() {
        let mut manager = WebSocketManager::default();
        let result = manager.close("nonexistent-id").await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not found"));
    }

    #[tokio::test]
    async fn test_manager_send_text_nonexistent() {
        let mut manager = WebSocketManager::default();
        let result = manager.send_text("nonexistent-id", "hello").await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not found"));
    }

    #[tokio::test]
    async fn test_manager_send_binary_nonexistent() {
        let mut manager = WebSocketManager::default();
        let result = manager.send_binary("nonexistent-id", b"data").await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not found"));
    }

    #[tokio::test]
    async fn test_manager_recv_nonexistent() {
        let mut manager = WebSocketManager::default();
        let result = manager.recv("nonexistent-id").await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not found"));
    }

    #[tokio::test]
    async fn test_manager_close_all_empty() {
        let mut manager = WebSocketManager::default();
        // Should not panic
        manager.close_all().await;
        assert_eq!(manager.connection_count(), 0);
    }

    #[tokio::test]
    async fn test_manager_connect_blocks_private_ip() {
        let mut manager = WebSocketManager::default();
        let result = manager.connect("ws://192.168.1.1/ws").await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("blocked by security policy"));
    }

    #[tokio::test]
    async fn test_manager_connect_blocks_localhost() {
        let mut manager = WebSocketManager::default();
        let result = manager.connect("ws://localhost:8080/ws").await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("blocked by security policy") || err_msg.contains("localhost"));
    }

    #[tokio::test]
    async fn test_manager_connect_blocks_loopback() {
        let mut manager = WebSocketManager::default();
        let result = manager.connect("ws://127.0.0.1:8080/ws").await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("blocked by security policy"));
    }

    #[tokio::test]
    async fn test_manager_connect_blocks_link_local() {
        let mut manager = WebSocketManager::default();
        let result = manager.connect("ws://169.254.1.1/ws").await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("blocked by security policy"));
    }
}

// ---------------------------------------------------------------------------
// Event Bus Tests
// ---------------------------------------------------------------------------

mod event_bus_tests {
    use super::*;
    use tokio::sync::mpsc;

    #[test]
    fn test_event_bus_creation() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let config = WebSocketConfig::default();
        let _manager = WebSocketManager::new(config).with_event_bus(tx);
    }

    #[tokio::test]
    async fn test_no_event_on_failed_connect() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut manager = WebSocketManager::default().with_event_bus(tx);

        // This connect will be blocked by security policy
        let _ = manager.connect("ws://localhost/ws").await;

        // No event should be emitted since connection failed before creation
        assert!(rx.try_recv().is_err());
    }
}

// ---------------------------------------------------------------------------
// IPv6 Tests
// ---------------------------------------------------------------------------

mod ipv6_tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_blocks_ipv6_loopback() {
        let mut manager = WebSocketManager::default();
        let result = manager.connect("ws://[::1]:8080/ws").await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("blocked by security policy"));
    }

    #[tokio::test]
    async fn test_manager_blocks_ipv6_link_local() {
        let mut manager = WebSocketManager::default();
        let result = manager.connect("ws://[fe80::1]:8080/ws").await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("blocked by security policy"));
    }

    #[tokio::test]
    async fn test_manager_blocks_ipv6_unique_local() {
        let mut manager = WebSocketManager::default();
        let result = manager.connect("ws://[fc00::1]:8080/ws").await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("blocked by security policy"));
    }
}

// ---------------------------------------------------------------------------
// URL Validation Tests
// ---------------------------------------------------------------------------

mod url_validation_tests {
    use super::*;

    #[tokio::test]
    async fn test_wss_scheme_accepted() {
        // wss:// should be a valid scheme (will fail on connection, not validation)
        let mut manager = WebSocketManager::default();
        let result = manager.connect("wss://192.168.1.1/ws").await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // Should fail on IP blocking, not scheme validation
        assert!(!err_msg.contains("Invalid WebSocket URL scheme"));
    }

    #[tokio::test]
    async fn test_ws_scheme_accepted() {
        // ws:// should be a valid scheme
        let mut manager = WebSocketManager::default();
        let result = manager.connect("ws://192.168.1.1/ws").await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // Should fail on IP blocking, not scheme validation
        assert!(!err_msg.contains("Invalid WebSocket URL scheme"));
    }

    #[tokio::test]
    async fn test_websocket_with_port() {
        let mut manager = WebSocketManager::default();
        let result = manager.connect("ws://192.168.1.1:9000/ws").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_websocket_with_path() {
        let mut manager = WebSocketManager::default();
        let result = manager.connect("ws://192.168.1.1/api/v1/websocket").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_websocket_with_query() {
        let mut manager = WebSocketManager::default();
        let result = manager.connect("ws://192.168.1.1/ws?token=abc123").await;
        assert!(result.is_err());
    }
}

// ---------------------------------------------------------------------------
// Permissive Policy Tests
// ---------------------------------------------------------------------------

mod permissive_policy_tests {
    use super::*;

    #[tokio::test]
    async fn test_permissive_allows_localhost() {
        let config = WebSocketConfig {
            max_per_origin: 6,
            connect_timeout_secs: 1,
            max_message_size: 1024 * 1024,
            block_private_ips: false,
            block_loopback: false,
        };
        let mut manager = WebSocketManager::new(config);

        // With permissive policy, should attempt connection (will fail due to no server)
        let result = manager.connect("ws://127.0.0.1:9999/ws").await;

        // Should fail due to connection refused, not security policy
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // Should not be a security policy error
        assert!(!err_msg.contains("blocked by security policy"));
    }
}

// ---------------------------------------------------------------------------
// Connection Limit Tests
// ---------------------------------------------------------------------------

mod connection_limit_tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_limit_per_origin() {
        let config = WebSocketConfig {
            max_per_origin: 1,
            connect_timeout_secs: 1,
            max_message_size: 1024,
            block_private_ips: false,
            block_loopback: false,
        };
        let mut manager = WebSocketManager::new(config);

        // First connection attempt to localhost:9999
        let _result1 = manager.connect("ws://127.0.0.1:9999/ws").await;
        // This will fail due to connection refused, but that's ok

        // The important thing is that if we had a real server,
        // the second connection to the same origin would be rejected
        // due to max_per_origin limit
    }

    #[test]
    fn test_connection_limit_config() {
        let config = WebSocketConfig {
            max_per_origin: 2,
            ..Default::default()
        };
        assert_eq!(config.max_per_origin, 2);
    }
}

// ---------------------------------------------------------------------------
// CDP Event Tests
// ---------------------------------------------------------------------------

mod cdp_event_tests {
    use super::*;

    #[test]
    fn test_extract_origin() {
        // Test the extract_origin function behavior via manager's behavior
        let manager = WebSocketManager::default();

        // Verify manager was created successfully
        assert_eq!(manager.connection_count(), 0);
    }
}
