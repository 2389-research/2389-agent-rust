//! MQTT Integration Test Helpers
//!
//! Provides helper utilities for integration tests with MQTT broker.
//! Assumes MQTT broker is ALWAYS running at localhost:1883 (in CI/CD and dev).

use agent2389::config::MqttSection;

/// MQTT broker URL - always available at localhost:1883
pub const MQTT_BROKER_URL: &str = "mqtt://localhost:1883";
#[allow(dead_code)]
pub const MQTT_BROKER_PORT: u16 = 1883;

/// Create MQTT config pointing to localhost broker
pub fn mqtt_config() -> MqttSection {
    MqttSection {
        broker_url: MQTT_BROKER_URL.to_string(),
        username_env: None,
        password_env: None,
        heartbeat_interval_secs: 900,
    }
}

/// Create MQTT config with custom heartbeat interval
pub fn mqtt_config_with_heartbeat(heartbeat_secs: u64) -> MqttSection {
    MqttSection {
        broker_url: MQTT_BROKER_URL.to_string(),
        username_env: None,
        password_env: None,
        heartbeat_interval_secs: heartbeat_secs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mqtt_config_uses_localhost() {
        let config = mqtt_config();
        assert_eq!(config.broker_url, "mqtt://localhost:1883");
        assert_eq!(config.heartbeat_interval_secs, 900);
    }

    #[test]
    fn test_mqtt_config_custom_heartbeat() {
        let config = mqtt_config_with_heartbeat(300);
        assert_eq!(config.broker_url, "mqtt://localhost:1883");
        assert_eq!(config.heartbeat_interval_secs, 300);
    }
}
