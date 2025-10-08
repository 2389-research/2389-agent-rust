//! MQTT Integration Test Helpers
//!
//! Provides testcontainers infrastructure and helper utilities for
//! integration tests with real Mosquitto broker.

use agent2389::config::MqttSection;
use std::time::Duration;
use testcontainers::{Container, Image, clients::Cli};
use tokio::time::sleep;

/// Mosquitto MQTT broker image for testcontainers
#[derive(Debug, Default)]
pub struct MosquittoImage {
    /// MQTT v5 protocol version
    _version: u8,
}

impl MosquittoImage {
    pub fn new() -> Self {
        Self { _version: 5 }
    }
}

impl Image for MosquittoImage {
    type Args = MosquittoArgs;

    fn name(&self) -> String {
        "eclipse-mosquitto".to_string()
    }

    fn tag(&self) -> String {
        "2.0.22".to_string()
    }

    fn ready_conditions(&self) -> Vec<testcontainers::core::WaitFor> {
        vec![testcontainers::core::WaitFor::message_on_stderr(
            "mosquitto version 2.0",
        )]
    }

    fn expose_ports(&self) -> Vec<u16> {
        vec![1883]
    }
}

#[derive(Debug, Clone, Default)]
pub struct MosquittoArgs;

impl testcontainers::core::ImageArgs for MosquittoArgs {
    fn into_iterator(self) -> Box<dyn Iterator<Item = String>> {
        Box::new(
            vec![
                "mosquitto".to_string(),
                "-c".to_string(),
                "/mosquitto-no-auth.conf".to_string(),
            ]
            .into_iter(),
        )
    }
}

/// Test harness for MQTT integration tests
pub struct MqttTestHarness<'a> {
    _docker: &'a Cli,
    _container: Container<'a, MosquittoImage>,
    pub broker_url: String,
    pub broker_port: u16,
}

impl<'a> MqttTestHarness<'a> {
    /// Create new test harness with Mosquitto broker
    pub async fn new(docker: &'a Cli) -> Self {
        let image = MosquittoImage::new();
        let container = docker.run(image);
        let broker_port = container.get_host_port_ipv4(1883);
        let broker_url = format!("mqtt://localhost:{broker_port}");

        // Wait for broker to be fully ready
        sleep(Duration::from_millis(500)).await;

        Self {
            _docker: docker,
            _container: container,
            broker_url,
            broker_port,
        }
    }

    /// Create MQTT config pointing to test broker
    pub fn mqtt_config(&self) -> MqttSection {
        MqttSection {
            broker_url: self.broker_url.clone(),
            username_env: None,
            password_env: None,
            heartbeat_interval_secs: 900,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mqtt_harness_creates_broker() {
        let docker = Cli::default();
        let harness = MqttTestHarness::new(&docker).await;

        assert!(harness.broker_url.starts_with("mqtt://localhost:"));
        assert!(harness.broker_port > 0);
    }
}
