//! RFC-compliant MQTT client implementation for 2389 Agent Protocol
//!
//! This module provides a focused, decomposed MQTT client implementation that
//! separates pure functions from I/O operations for better testability and maintainability.
//!
//! # Architecture
//!
//! The module is split into four focused sub-modules:
//!
//! - [`connection`] - Pure connection state management and configuration
//! - [`message_handler`] - Pure message routing and processing logic
//! - [`health_monitor`] - Pure health monitoring and reconnection logic
//! - [`client`] - Impure I/O operations and coordination
//!
//! # Usage
//!
//! ```rust,no_run
//! use agent2389::transport::mqtt::MqttClient;
//! use agent2389::config::MqttSection;
//!
//! # tokio_test::block_on(async {
//! let config = MqttSection {
//!     broker_url: "mqtt://localhost:1883".to_string(),
//!     username_env: None,
//!     password_env: None,
//!     heartbeat_interval_secs: 900,
//! };
//!
//! let mut client = MqttClient::new("my-agent", config).await?;
//! client.connect().await?;
//! client.subscribe_to_tasks().await?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # });
//! ```

pub mod client;
pub mod connection;
pub mod health_monitor;
pub mod message_handler;

// Re-export public types for convenience
pub use client::MqttClient;
pub use connection::{ConnectionState, MqttError, ReconnectConfig, TopicBuilder};
pub use health_monitor::{
    ConnectionEvent, ConnectionQuality, HealthMetrics, HealthMonitor, ReconnectionDecision,
};
pub use message_handler::{EventRoute, MessageHandler};

// Re-export for backwards compatibility
pub use client::MqttClient as Client;
pub use connection::MqttError as Error;
