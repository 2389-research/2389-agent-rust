//! Health check system for production monitoring
//!
//! Provides health check traits and implementations for various agent components
//! including MQTT transport, LLM providers, and tool systems.

use crate::error::AgentResult;
use crate::llm::provider::LlmProvider;
use crate::transport::Transport;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, warn};

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub component: String,
    pub healthy: bool,
    pub message: Option<String>,
    pub response_time_ms: Option<u64>,
}

/// Trait for components that can be health checked
#[async_trait]
pub trait HealthCheck: Send + Sync {
    /// Perform health check on this component
    async fn health_check(&self) -> HealthCheckResult;

    /// Get the component name for reporting
    fn component_name(&self) -> &str;
}

/// MQTT transport health check implementation
pub struct MqttHealthCheck<T: Transport> {
    transport: Arc<T>,
}

impl<T: Transport> MqttHealthCheck<T> {
    pub fn new(transport: Arc<T>) -> Self {
        Self { transport }
    }
}

#[async_trait]
impl<T: Transport> HealthCheck for MqttHealthCheck<T> {
    async fn health_check(&self) -> HealthCheckResult {
        let start = std::time::Instant::now();
        let component = self.component_name().to_string();

        // Check connection status
        let is_connected = self.transport.is_connected();
        let is_permanently_disconnected = self.transport.is_permanently_disconnected();
        let connection_state = self.transport.connection_state();

        let healthy = is_connected && !is_permanently_disconnected;
        let response_time_ms = start.elapsed().as_millis() as u64;

        let message = if !healthy {
            Some(format!(
                "MQTT not connected - state: {connection_state:?}, permanently disconnected: {is_permanently_disconnected}"
            ))
        } else {
            Some("MQTT connection healthy".to_string())
        };

        debug!(
            "MQTT health check: healthy={}, connection_state={:?}, response_time={}ms",
            healthy, connection_state, response_time_ms
        );

        HealthCheckResult {
            component,
            healthy,
            message,
            response_time_ms: Some(response_time_ms),
        }
    }

    fn component_name(&self) -> &str {
        "mqtt_transport"
    }
}

/// LLM provider health check implementation
pub struct LlmProviderHealthCheck {
    llm_provider: Arc<dyn LlmProvider>,
}

impl LlmProviderHealthCheck {
    pub fn new(llm_provider: Arc<dyn LlmProvider>) -> Self {
        Self { llm_provider }
    }
}

#[async_trait]
impl HealthCheck for LlmProviderHealthCheck {
    async fn health_check(&self) -> HealthCheckResult {
        let start = std::time::Instant::now();
        let component = self.component_name().to_string();

        match self.llm_provider.health_check().await {
            Ok(()) => {
                let response_time_ms = start.elapsed().as_millis() as u64;
                debug!(
                    "LLM provider health check: healthy=true, provider={}, response_time={}ms",
                    self.llm_provider.name(),
                    response_time_ms
                );

                HealthCheckResult {
                    component,
                    healthy: true,
                    message: Some(format!("{} provider healthy", self.llm_provider.name())),
                    response_time_ms: Some(response_time_ms),
                }
            }
            Err(e) => {
                let response_time_ms = start.elapsed().as_millis() as u64;
                warn!(
                    "LLM provider health check failed: provider={}, error={}, response_time={}ms",
                    self.llm_provider.name(),
                    e,
                    response_time_ms
                );

                HealthCheckResult {
                    component,
                    healthy: false,
                    message: Some(format!(
                        "{} provider error: {}",
                        self.llm_provider.name(),
                        e
                    )),
                    response_time_ms: Some(response_time_ms),
                }
            }
        }
    }

    fn component_name(&self) -> &str {
        "llm_provider"
    }
}

/// Aggregated health check manager
pub struct HealthCheckManager {
    health_checks: Vec<Box<dyn HealthCheck>>,
}

impl HealthCheckManager {
    pub fn new() -> Self {
        Self {
            health_checks: Vec::new(),
        }
    }

    /// Add a health check to the manager
    pub fn add_health_check(&mut self, health_check: Box<dyn HealthCheck>) {
        self.health_checks.push(health_check);
    }

    /// Run all health checks and return aggregated results
    pub async fn run_health_checks(&self) -> Vec<HealthCheckResult> {
        let mut results = Vec::new();

        for health_check in &self.health_checks {
            let result = health_check.health_check().await;
            results.push(result);
        }

        results
    }

    /// Calculate overall health status from all components
    pub async fn calculate_overall_health(&self) -> AgentResult<bool> {
        let results = self.run_health_checks().await;

        if results.is_empty() {
            warn!("No health checks configured - assuming healthy");
            return Ok(true);
        }

        let healthy_count = results.iter().filter(|r| r.healthy).count();
        let total_count = results.len();

        // All components must be healthy for overall health
        let overall_healthy = healthy_count == total_count;

        debug!(
            "Overall health check: {}/{} components healthy, overall={}",
            healthy_count, total_count, overall_healthy
        );

        Ok(overall_healthy)
    }
}

impl Default for HealthCheckManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::mocks::{MockLlmProvider, MockTransport};

    #[tokio::test]
    async fn test_mqtt_health_check_healthy() {
        let transport = Arc::new(MockTransport::new());
        let health_check = MqttHealthCheck::new(transport);

        let result = health_check.health_check().await;

        assert_eq!(result.component, "mqtt_transport");
        assert!(result.healthy);
        assert!(result.message.is_some());
        assert!(result.response_time_ms.is_some());
    }

    #[tokio::test]
    async fn test_mqtt_health_check_unhealthy() {
        let transport = Arc::new(MockTransport::with_failure());
        let health_check = MqttHealthCheck::new(transport);

        let result = health_check.health_check().await;

        assert_eq!(result.component, "mqtt_transport");
        assert!(!result.healthy);
        assert!(result.message.is_some());
        assert!(result.response_time_ms.is_some());
    }

    #[tokio::test]
    async fn test_llm_provider_health_check_healthy() {
        let llm_provider = Arc::new(MockLlmProvider::single_response("test"));
        let health_check = LlmProviderHealthCheck::new(llm_provider);

        let result = health_check.health_check().await;

        assert_eq!(result.component, "llm_provider");
        assert!(result.healthy);
        assert!(result.message.is_some());
        assert!(result.response_time_ms.is_some());
    }

    #[tokio::test]
    async fn test_llm_provider_health_check_unhealthy() {
        let llm_provider = Arc::new(MockLlmProvider::with_failure());
        let health_check = LlmProviderHealthCheck::new(llm_provider);

        let result = health_check.health_check().await;

        assert_eq!(result.component, "llm_provider");
        assert!(!result.healthy);
        assert!(result.message.is_some());
        assert!(result.response_time_ms.is_some());
    }

    #[tokio::test]
    async fn test_health_check_manager() {
        let mut manager = HealthCheckManager::new();

        // Add healthy components
        let transport = Arc::new(MockTransport::new());
        let llm_provider = Arc::new(MockLlmProvider::single_response("test"));

        manager.add_health_check(Box::new(MqttHealthCheck::new(transport)));
        manager.add_health_check(Box::new(LlmProviderHealthCheck::new(llm_provider)));

        let results = manager.run_health_checks().await;
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.healthy));

        let overall_healthy = manager.calculate_overall_health().await.unwrap();
        assert!(overall_healthy);
    }

    #[tokio::test]
    async fn test_health_check_manager_with_failure() {
        let mut manager = HealthCheckManager::new();

        // Add healthy and unhealthy components
        let healthy_transport = Arc::new(MockTransport::new());
        let unhealthy_llm = Arc::new(MockLlmProvider::with_failure());

        manager.add_health_check(Box::new(MqttHealthCheck::new(healthy_transport)));
        manager.add_health_check(Box::new(LlmProviderHealthCheck::new(unhealthy_llm)));

        let results = manager.run_health_checks().await;
        assert_eq!(results.len(), 2);
        assert_eq!(results.iter().filter(|r| r.healthy).count(), 1);

        let overall_healthy = manager.calculate_overall_health().await.unwrap();
        assert!(!overall_healthy);
    }
}
