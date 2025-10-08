//! Pure health monitoring and reconnection logic for MQTT client
//!
//! This module contains pure functions for health monitoring,
//! reconnection decision making, and connection state tracking.

use super::connection::{ConnectionState, ReconnectConfig};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

/// Pure health monitoring and reconnection decision logic
pub struct HealthMonitor;

impl HealthMonitor {
    /// Determine if reconnection should be attempted (pure function)
    /// Supports unlimited retries when max_attempts is None
    pub fn should_attempt_reconnection(
        current_attempts: u32,
        config: &ReconnectConfig,
        shutdown_requested: bool,
    ) -> ReconnectionDecision {
        if shutdown_requested {
            return ReconnectionDecision::AbortShutdownRequested;
        }

        // Check max attempts only if configured (Some)
        if let Some(max_attempts) = config.max_attempts {
            if current_attempts >= max_attempts {
                return ReconnectionDecision::AbortMaxAttemptsExceeded;
            }
        }
        // If max_attempts is None, retry forever

        let backoff_delay = config.calculate_backoff_delay(current_attempts + 1);
        ReconnectionDecision::Proceed {
            attempt: current_attempts + 1,
            delay_ms: backoff_delay,
        }
    }

    /// Calculate connection timeout based on reconnection configuration (pure function)
    /// For unlimited retries, uses a reasonable initial timeout
    pub fn calculate_connection_timeout(config: &ReconnectConfig) -> Duration {
        match config.calculate_max_total_time() {
            Some(max_total_time) => Duration::from_millis(max_total_time + 30000), // Extra 30s buffer
            None => Duration::from_secs(60), // 60 second initial timeout for unlimited retries
        }
    }

    /// Determine next state after connection event (pure function)
    pub fn determine_next_state(
        _current_state: &ConnectionState,
        event: ConnectionEvent,
    ) -> ConnectionState {
        match event {
            ConnectionEvent::ConnAckReceived => {
                info!("MQTT client connected successfully");
                ConnectionState::Connected
            }
            ConnectionEvent::DisconnectedByBroker => {
                info!("MQTT broker disconnected agent");
                ConnectionState::Disconnected("Broker disconnected".to_string())
            }
            ConnectionEvent::NetworkError(error) => {
                error!("MQTT event loop error: {}", error);
                ConnectionState::Disconnected(error)
            }
            ConnectionEvent::ReconnectionStarted(attempt) => {
                info!("Starting reconnection attempt {}", attempt);
                ConnectionState::Reconnecting(attempt)
            }
            ConnectionEvent::PermanentFailure(reason) => {
                error!("Permanent connection failure: {}", reason);
                ConnectionState::PermanentlyDisconnected(reason)
            }
        }
    }

    /// Check if connection state allows publishing (pure function)
    pub fn can_publish(state: &ConnectionState) -> bool {
        matches!(state, ConnectionState::Connected)
    }

    /// Check if connection state allows subscribing (pure function)
    pub fn can_subscribe(state: &ConnectionState) -> bool {
        matches!(state, ConnectionState::Connected)
    }

    /// Calculate health metrics for connection (pure function)
    pub fn calculate_health_metrics(
        connect_time: Option<Instant>,
        last_message_time: Option<Instant>,
        reconnect_count: u32,
    ) -> HealthMetrics {
        let now = Instant::now();

        let uptime = connect_time.map(|t| now.duration_since(t));
        let time_since_last_message = last_message_time.map(|t| now.duration_since(t));

        HealthMetrics {
            uptime,
            time_since_last_message,
            reconnect_count,
            is_healthy: Self::determine_health_status(uptime, time_since_last_message),
        }
    }

    /// Determine overall health status (pure function)
    fn determine_health_status(
        uptime: Option<Duration>,
        time_since_last_message: Option<Duration>,
    ) -> bool {
        // Connection is healthy if:
        // 1. We have uptime (connected)
        // 2. Either no messages yet, or last message was recent (< 5 minutes)
        match (uptime, time_since_last_message) {
            (Some(_), None) => true, // Connected, no messages yet
            (Some(_), Some(last_msg)) => last_msg < Duration::from_secs(300), // < 5 minutes
            _ => false,              // Not connected
        }
    }

    /// Log connection state transition (pure logging function)
    pub fn log_state_transition(from: &ConnectionState, to: &ConnectionState) {
        match (from, to) {
            (ConnectionState::Connecting, ConnectionState::Connected) => {
                info!("MQTT connection established successfully");
            }
            (ConnectionState::Connected, ConnectionState::Disconnected(reason)) => {
                warn!("MQTT connection lost: {}", reason);
            }
            (ConnectionState::Disconnected(_), ConnectionState::Reconnecting(attempt)) => {
                info!("Starting reconnection attempt {}", attempt);
            }
            (ConnectionState::Reconnecting(_), ConnectionState::Connected) => {
                info!("Reconnection successful");
            }
            (_, ConnectionState::PermanentlyDisconnected(reason)) => {
                error!("MQTT connection permanently failed: {}", reason);
            }
            _ => {
                info!("MQTT connection state: {:?} -> {:?}", from, to);
            }
        }
    }

    /// Validate connection configuration (pure function)
    pub fn validate_connection_config(config: &ReconnectConfig) -> Result<(), String> {
        if let Some(max_attempts) = config.max_attempts {
            if max_attempts == 0 {
                return Err("max_attempts must be greater than 0 or None for unlimited".to_string());
            }
        }

        if config.sustained_delay == 0 {
            return Err("sustained_delay must be greater than 0".to_string());
        }

        if config.backoff_pattern.is_empty() && config.sustained_delay == 0 {
            return Err("must have either backoff_pattern or sustained_delay > 0".to_string());
        }

        Ok(())
    }
}

/// Decision result for reconnection attempts
#[derive(Debug, PartialEq)]
pub enum ReconnectionDecision {
    /// Proceed with reconnection attempt
    Proceed { attempt: u32, delay_ms: u64 },
    /// Abort reconnection - shutdown requested
    AbortShutdownRequested,
    /// Abort reconnection - max attempts exceeded
    AbortMaxAttemptsExceeded,
}

/// Connection events that trigger state transitions
#[derive(Debug, Clone)]
pub enum ConnectionEvent {
    /// ConnAck received from broker
    ConnAckReceived,
    /// Broker initiated disconnect
    DisconnectedByBroker,
    /// Network or protocol error
    NetworkError(String),
    /// Reconnection attempt started
    ReconnectionStarted(u32),
    /// Permanent failure - no more retries
    PermanentFailure(String),
}

/// Health metrics for connection monitoring
#[derive(Debug, Clone)]
pub struct HealthMetrics {
    /// Time since connection established
    pub uptime: Option<Duration>,
    /// Time since last message received
    pub time_since_last_message: Option<Duration>,
    /// Number of reconnection attempts
    pub reconnect_count: u32,
    /// Overall health status
    pub is_healthy: bool,
}

/// Connection quality assessment
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionQuality {
    /// Excellent - stable connection, recent activity
    Excellent,
    /// Good - stable connection, moderate activity
    Good,
    /// Fair - some reconnections, but working
    Fair,
    /// Poor - frequent reconnections
    Poor,
    /// Critical - connection failing frequently
    Critical,
}

impl HealthMonitor {
    /// Assess connection quality based on metrics (pure function)
    pub fn assess_connection_quality(metrics: &HealthMetrics) -> ConnectionQuality {
        if !metrics.is_healthy {
            return ConnectionQuality::Critical;
        }

        match (metrics.reconnect_count, metrics.uptime) {
            // No reconnections, good uptime
            (0, Some(uptime)) if uptime > Duration::from_secs(3600) => ConnectionQuality::Excellent,
            (0, Some(_)) => ConnectionQuality::Good,

            // Few reconnections
            (1..=2, Some(uptime)) if uptime > Duration::from_secs(1800) => ConnectionQuality::Good,
            (1..=2, Some(_)) => ConnectionQuality::Fair,

            // Many reconnections
            (3..=5, _) => ConnectionQuality::Fair,
            (6..=10, _) => ConnectionQuality::Poor,

            // Too many reconnections or no uptime
            _ => ConnectionQuality::Critical,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_attempt_reconnection() {
        let config = ReconnectConfig::default();

        // Should proceed on first attempt with custom pattern
        let decision = HealthMonitor::should_attempt_reconnection(0, &config, false);
        assert_eq!(
            decision,
            ReconnectionDecision::Proceed {
                attempt: 1,
                delay_ms: 25 // First delay in pattern
            }
        );

        // Should abort if shutdown requested
        let decision = HealthMonitor::should_attempt_reconnection(0, &config, true);
        assert_eq!(decision, ReconnectionDecision::AbortShutdownRequested);

        // Should proceed with custom backoff pattern
        let decision = HealthMonitor::should_attempt_reconnection(2, &config, false);
        assert_eq!(
            decision,
            ReconnectionDecision::Proceed {
                attempt: 3,
                delay_ms: 100 // Third delay in pattern
            }
        );

        // Should sustain at 250ms after pattern exhausted
        let decision = HealthMonitor::should_attempt_reconnection(5, &config, false);
        assert_eq!(
            decision,
            ReconnectionDecision::Proceed {
                attempt: 6,
                delay_ms: 250 // Sustained delay
            }
        );

        // Test with limited attempts
        let limited_config = ReconnectConfig {
            max_attempts: Some(5),
            backoff_pattern: vec![25, 50, 100, 250],
            sustained_delay: 250,
        };

        // Should abort if max attempts exceeded
        let decision = HealthMonitor::should_attempt_reconnection(5, &limited_config, false);
        assert_eq!(decision, ReconnectionDecision::AbortMaxAttemptsExceeded);
    }

    #[test]
    fn test_calculate_connection_timeout() {
        // Test unlimited retries - should use default 60s timeout
        let unlimited_config = ReconnectConfig::default();
        let timeout = HealthMonitor::calculate_connection_timeout(&unlimited_config);
        assert_eq!(timeout, Duration::from_secs(60));

        // Test limited retries - should use calculated timeout
        let limited_config = ReconnectConfig {
            max_attempts: Some(4),
            backoff_pattern: vec![25, 50, 100, 250],
            sustained_delay: 250,
        };
        let timeout = HealthMonitor::calculate_connection_timeout(&limited_config);
        let expected_total = 25 + 50 + 100 + 250; // Sum of pattern
        let expected = Duration::from_millis(expected_total + 30000);
        assert_eq!(timeout, expected);
    }

    #[test]
    fn test_determine_next_state() {
        let initial_state = ConnectionState::Connecting;

        // Test ConnAck transition
        let next_state =
            HealthMonitor::determine_next_state(&initial_state, ConnectionEvent::ConnAckReceived);
        assert_eq!(next_state, ConnectionState::Connected);

        // Test disconnect transition
        let connected_state = ConnectionState::Connected;
        let next_state = HealthMonitor::determine_next_state(
            &connected_state,
            ConnectionEvent::DisconnectedByBroker,
        );
        assert_eq!(
            next_state,
            ConnectionState::Disconnected("Broker disconnected".to_string())
        );

        // Test network error transition
        let next_state = HealthMonitor::determine_next_state(
            &connected_state,
            ConnectionEvent::NetworkError("timeout".to_string()),
        );
        assert_eq!(
            next_state,
            ConnectionState::Disconnected("timeout".to_string())
        );

        // Test reconnection transition
        let disconnected_state = ConnectionState::Disconnected("test".to_string());
        let next_state = HealthMonitor::determine_next_state(
            &disconnected_state,
            ConnectionEvent::ReconnectionStarted(1),
        );
        assert_eq!(next_state, ConnectionState::Reconnecting(1));

        // Test permanent failure transition
        let next_state = HealthMonitor::determine_next_state(
            &disconnected_state,
            ConnectionEvent::PermanentFailure("max attempts".to_string()),
        );
        assert_eq!(
            next_state,
            ConnectionState::PermanentlyDisconnected("max attempts".to_string())
        );
    }

    #[test]
    fn test_can_publish() {
        assert!(HealthMonitor::can_publish(&ConnectionState::Connected));
        assert!(!HealthMonitor::can_publish(&ConnectionState::Connecting));
        assert!(!HealthMonitor::can_publish(&ConnectionState::Disconnected(
            "test".to_string()
        )));
        assert!(!HealthMonitor::can_publish(&ConnectionState::Reconnecting(
            1
        )));
        assert!(!HealthMonitor::can_publish(
            &ConnectionState::PermanentlyDisconnected("test".to_string())
        ));
    }

    #[test]
    fn test_can_subscribe() {
        assert!(HealthMonitor::can_subscribe(&ConnectionState::Connected));
        assert!(!HealthMonitor::can_subscribe(&ConnectionState::Connecting));
        assert!(!HealthMonitor::can_subscribe(
            &ConnectionState::Disconnected("test".to_string())
        ));
        assert!(!HealthMonitor::can_subscribe(
            &ConnectionState::Reconnecting(1)
        ));
        assert!(!HealthMonitor::can_subscribe(
            &ConnectionState::PermanentlyDisconnected("test".to_string())
        ));
    }

    #[test]
    fn test_calculate_health_metrics() {
        let now = Instant::now();
        let connect_time = Some(now - Duration::from_secs(3600)); // 1 hour ago
        let last_message_time = Some(now - Duration::from_secs(60)); // 1 minute ago

        let metrics = HealthMonitor::calculate_health_metrics(connect_time, last_message_time, 2);

        assert!(metrics.uptime.is_some());
        assert!(metrics.time_since_last_message.is_some());
        assert_eq!(metrics.reconnect_count, 2);
        assert!(metrics.is_healthy); // Recent message should be healthy
    }

    #[test]
    fn test_determine_health_status() {
        // Connected with recent message - healthy
        let uptime = Some(Duration::from_secs(3600));
        let recent_message = Some(Duration::from_secs(60));
        assert!(HealthMonitor::determine_health_status(
            uptime,
            recent_message
        ));

        // Connected with no messages yet - healthy
        assert!(HealthMonitor::determine_health_status(uptime, None));

        // Connected with old message - unhealthy
        let old_message = Some(Duration::from_secs(400)); // > 5 minutes
        assert!(!HealthMonitor::determine_health_status(uptime, old_message));

        // Not connected - unhealthy
        assert!(!HealthMonitor::determine_health_status(None, None));
    }

    #[test]
    fn test_validate_connection_config() {
        // Valid config - unlimited retries
        let valid_config = ReconnectConfig::default();
        assert!(HealthMonitor::validate_connection_config(&valid_config).is_ok());

        // Valid config - limited retries
        let limited_config = ReconnectConfig {
            max_attempts: Some(10),
            backoff_pattern: vec![25, 50, 100, 250],
            sustained_delay: 250,
        };
        assert!(HealthMonitor::validate_connection_config(&limited_config).is_ok());

        // Invalid: max_attempts = 0
        let invalid_config = ReconnectConfig {
            max_attempts: Some(0),
            ..Default::default()
        };
        assert!(HealthMonitor::validate_connection_config(&invalid_config).is_err());

        // Invalid: sustained_delay = 0
        let invalid_config = ReconnectConfig {
            sustained_delay: 0,
            ..Default::default()
        };
        assert!(HealthMonitor::validate_connection_config(&invalid_config).is_err());

        // Invalid: empty pattern and zero sustained delay
        let invalid_config = ReconnectConfig {
            max_attempts: None,
            backoff_pattern: vec![],
            sustained_delay: 0,
        };
        assert!(HealthMonitor::validate_connection_config(&invalid_config).is_err());
    }

    #[test]
    fn test_assess_connection_quality() {
        let _now = Instant::now();

        // Excellent: No reconnections, good uptime
        let excellent_metrics = HealthMetrics {
            uptime: Some(Duration::from_secs(7200)), // 2 hours
            time_since_last_message: Some(Duration::from_secs(30)),
            reconnect_count: 0,
            is_healthy: true,
        };
        assert_eq!(
            HealthMonitor::assess_connection_quality(&excellent_metrics),
            ConnectionQuality::Excellent
        );

        // Good: No reconnections, moderate uptime
        let good_metrics = HealthMetrics {
            uptime: Some(Duration::from_secs(1800)), // 30 minutes
            time_since_last_message: Some(Duration::from_secs(30)),
            reconnect_count: 0,
            is_healthy: true,
        };
        assert_eq!(
            HealthMonitor::assess_connection_quality(&good_metrics),
            ConnectionQuality::Good
        );

        // Fair: Few reconnections
        let fair_metrics = HealthMetrics {
            uptime: Some(Duration::from_secs(1800)),
            time_since_last_message: Some(Duration::from_secs(30)),
            reconnect_count: 2,
            is_healthy: true,
        };
        assert_eq!(
            HealthMonitor::assess_connection_quality(&fair_metrics),
            ConnectionQuality::Fair
        );

        // Poor: Many reconnections
        let poor_metrics = HealthMetrics {
            uptime: Some(Duration::from_secs(1800)),
            time_since_last_message: Some(Duration::from_secs(30)),
            reconnect_count: 8,
            is_healthy: true,
        };
        assert_eq!(
            HealthMonitor::assess_connection_quality(&poor_metrics),
            ConnectionQuality::Poor
        );

        // Critical: Not healthy
        let critical_metrics = HealthMetrics {
            uptime: None,
            time_since_last_message: None,
            reconnect_count: 0,
            is_healthy: false,
        };
        assert_eq!(
            HealthMonitor::assess_connection_quality(&critical_metrics),
            ConnectionQuality::Critical
        );
    }
}
