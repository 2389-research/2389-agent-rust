//! Health check HTTP server for container orchestration
//!
//! Provides HTTP endpoints for monitoring agent status, supporting both
//! human operators and container orchestration platforms.

use crate::observability::metrics::metrics;
use serde::Serialize;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use warp::Filter;

/// HTTP health check server
pub struct HealthServer {
    agent_id: String,
    port: u16,
    mqtt_connected: Arc<AtomicBool>,
    last_task_processed: Arc<AtomicU64>,
    additional_checks: Arc<RwLock<HashMap<String, HealthCheck>>>,
}

impl HealthServer {
    /// Create new health server
    pub fn new(agent_id: String, port: u16) -> Self {
        Self {
            agent_id,
            port,
            mqtt_connected: Arc::new(AtomicBool::new(false)),
            last_task_processed: Arc::new(AtomicU64::new(0)),
            additional_checks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Update MQTT connection status
    pub async fn set_mqtt_connected(&self, connected: bool) {
        self.mqtt_connected.store(connected, Ordering::Relaxed);
    }

    /// Update last task processed timestamp
    pub async fn set_last_task_processed(&self, timestamp: u64) {
        self.last_task_processed.store(timestamp, Ordering::Relaxed);
    }

    /// Add custom health check
    pub async fn add_health_check(&self, name: String, check: HealthCheck) {
        let mut checks = self.additional_checks.write().await;
        checks.insert(name, check);
    }

    /// Remove health check
    pub async fn remove_health_check(&self, name: &str) {
        let mut checks = self.additional_checks.write().await;
        checks.remove(name);
    }

    /// Start the HTTP health server
    pub async fn start(self: Arc<Self>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let health_server = self.clone();
        let metrics_server = self.clone();
        let ready_server = self.clone();
        let live_server = self.clone();
        let root_server = self.clone();

        // GET /health - comprehensive health status
        let health_route = warp::path("health").and(warp::get()).and_then(move || {
            let server = health_server.clone();
            async move {
                match server.get_health_status().await {
                    Ok(status) => {
                        let status_code = if status.status == "healthy" { 200 } else { 503 };
                        Ok::<_, Infallible>(warp::reply::with_status(
                            warp::reply::json(&status),
                            warp::http::StatusCode::from_u16(status_code).unwrap(),
                        ))
                    }
                    Err(e) => {
                        let error_response = ErrorResponse {
                            error: format!("Health check failed: {e}"),
                            timestamp: current_timestamp(),
                        };
                        Ok::<_, Infallible>(warp::reply::with_status(
                            warp::reply::json(&error_response),
                            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                        ))
                    }
                }
            }
        });

        // GET /metrics - complete metrics export
        let metrics_route = warp::path("metrics").and(warp::get()).and_then(move || {
            let _server = metrics_server.clone();
            async move {
                let metrics_snapshot = metrics().get_metrics();
                Ok::<_, Infallible>(warp::reply::json(&metrics_snapshot))
            }
        });

        // GET /ready - Kubernetes readiness probe
        let ready_route = warp::path("ready").and(warp::get()).and_then(move || {
            let server = ready_server.clone();
            async move {
                let ready = server.mqtt_connected.load(Ordering::Relaxed);
                let response = ReadinessResponse {
                    ready,
                    timestamp: current_timestamp(),
                };
                let status_code = if ready { 200 } else { 503 };
                Ok::<_, Infallible>(warp::reply::with_status(
                    warp::reply::json(&response),
                    warp::http::StatusCode::from_u16(status_code).unwrap(),
                ))
            }
        });

        // GET /live - Kubernetes liveness probe
        let live_route = warp::path("live").and(warp::get()).and_then(move || {
            let _server = live_server.clone();
            async move {
                let response = LivenessResponse {
                    alive: true,
                    timestamp: current_timestamp(),
                };
                Ok::<_, Infallible>(warp::reply::json(&response))
            }
        });

        // GET / - API documentation
        let root_route = warp::path::end().and(warp::get()).and_then(move || {
            let _server = root_server.clone();
            async move {
                let mut endpoints = HashMap::new();
                endpoints.insert(
                    "/health".to_string(),
                    "Overall health status with detailed checks".to_string(),
                );
                endpoints.insert(
                    "/metrics".to_string(),
                    "Comprehensive metrics and statistics".to_string(),
                );
                endpoints.insert(
                    "/ready".to_string(),
                    "Readiness probe for Kubernetes".to_string(),
                );
                endpoints.insert(
                    "/live".to_string(),
                    "Liveness probe for Kubernetes".to_string(),
                );

                let response = ApiDocumentationResponse { endpoints };
                Ok::<_, Infallible>(warp::reply::json(&response))
            }
        });

        let routes = health_route
            .or(metrics_route)
            .or(ready_route)
            .or(live_route)
            .or(root_route)
            .with(warp::cors().allow_any_origin());

        tracing::info!("Starting health server on port {}", self.port);

        warp::serve(routes).run(([0, 0, 0, 0], self.port)).await;

        Ok(())
    }

    async fn get_health_status(
        &self,
    ) -> Result<HealthStatus, Box<dyn std::error::Error + Send + Sync>> {
        let now = current_timestamp();

        // Perform individual health checks
        let mut checks = HashMap::new();

        // MQTT health check
        let mqtt_check = self.check_mqtt_health().await;
        checks.insert("mqtt".to_string(), mqtt_check);

        // Task processing health check
        let task_check = self.check_task_processing_health().await;
        checks.insert("task_processing".to_string(), task_check);

        // Add any additional health checks
        let additional = self.additional_checks.read().await;
        for (name, check) in additional.iter() {
            checks.insert(name.clone(), check.clone());
        }

        // Determine overall health status
        let overall_healthy = checks.values().all(|check| check.status == "healthy");
        let overall_status = if overall_healthy {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        };

        let uptime_seconds = now - metrics().get_metrics().lifecycle.uptime_seconds;

        Ok(HealthStatus {
            status: overall_status,
            timestamp: now,
            agent_id: self.agent_id.clone(),
            uptime_seconds,
            checks,
        })
    }

    async fn check_mqtt_health(&self) -> HealthCheck {
        let connected = self.mqtt_connected.load(Ordering::Relaxed);
        let now = current_timestamp();

        if connected {
            HealthCheck {
                status: "healthy".to_string(),
                message: Some("MQTT connection established".to_string()),
                last_check: now,
            }
        } else {
            HealthCheck {
                status: "unhealthy".to_string(),
                message: Some("MQTT connection failed or disconnected".to_string()),
                last_check: now,
            }
        }
    }

    async fn check_task_processing_health(&self) -> HealthCheck {
        const TASK_STALENESS_THRESHOLD_SECONDS: u64 = 300; // 5 minutes

        let now = current_timestamp();
        let last_task = self.last_task_processed.load(Ordering::Relaxed);

        if last_task == 0 {
            // No tasks processed yet - this is healthy for a new agent
            HealthCheck {
                status: "healthy".to_string(),
                message: Some("No tasks processed yet - agent ready".to_string()),
                last_check: now,
            }
        } else if now - last_task > TASK_STALENESS_THRESHOLD_SECONDS {
            // Stale task processing
            let stale_duration = now - last_task;
            HealthCheck {
                status: "stale".to_string(),
                message: Some(format!("No task activity for {stale_duration} seconds")),
                last_check: now,
            }
        } else {
            // Recent task activity
            HealthCheck {
                status: "healthy".to_string(),
                message: Some("Recent task activity".to_string()),
                last_check: now,
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthCheck {
    pub status: String,
    pub message: Option<String>,
    pub last_check: u64,
}

#[derive(Debug, Serialize)]
struct HealthStatus {
    status: String,
    timestamp: u64,
    agent_id: String,
    uptime_seconds: u64,
    checks: HashMap<String, HealthCheck>,
}

#[derive(Debug, Serialize)]
struct ReadinessResponse {
    ready: bool,
    timestamp: u64,
}

#[derive(Debug, Serialize)]
struct LivenessResponse {
    alive: bool,
    timestamp: u64,
}

#[derive(Debug, Serialize)]
struct ApiDocumentationResponse {
    endpoints: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
    timestamp: u64,
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    // use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_health_server_creation() {
        let health_server = HealthServer::new("test-agent".to_string(), 8080);
        assert_eq!(health_server.agent_id, "test-agent");
        assert_eq!(health_server.port, 8080);
    }

    #[tokio::test]
    async fn test_mqtt_connection_status() {
        let health_server = HealthServer::new("test-agent".to_string(), 8080);

        // Initially not connected
        assert!(!health_server.mqtt_connected.load(Ordering::Relaxed));

        // Set connected
        health_server.set_mqtt_connected(true).await;
        assert!(health_server.mqtt_connected.load(Ordering::Relaxed));

        // Set disconnected
        health_server.set_mqtt_connected(false).await;
        assert!(!health_server.mqtt_connected.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_task_processing_timestamp() {
        let health_server = HealthServer::new("test-agent".to_string(), 8080);

        let timestamp = current_timestamp();
        health_server.set_last_task_processed(timestamp).await;

        assert_eq!(
            health_server.last_task_processed.load(Ordering::Relaxed),
            timestamp
        );
    }

    #[tokio::test]
    async fn test_health_check_logic() {
        let health_server = HealthServer::new("test-agent".to_string(), 8080);

        // Test MQTT health check when disconnected
        let mqtt_check = health_server.check_mqtt_health().await;
        assert_eq!(mqtt_check.status, "unhealthy");

        // Test when connected
        health_server.set_mqtt_connected(true).await;
        let mqtt_check = health_server.check_mqtt_health().await;
        assert_eq!(mqtt_check.status, "healthy");

        // Test task processing health check with no tasks
        let task_check = health_server.check_task_processing_health().await;
        assert_eq!(task_check.status, "healthy");

        // Test with recent task
        let now = current_timestamp();
        health_server.set_last_task_processed(now).await;
        let task_check = health_server.check_task_processing_health().await;
        assert_eq!(task_check.status, "healthy");

        // Test with stale task (simulate old timestamp)
        health_server.set_last_task_processed(now - 600).await; // 10 minutes ago
        let task_check = health_server.check_task_processing_health().await;
        assert_eq!(task_check.status, "stale");
    }

    #[tokio::test]
    async fn test_additional_health_checks() {
        let health_server = HealthServer::new("test-agent".to_string(), 8080);

        let custom_check = HealthCheck {
            status: "healthy".to_string(),
            message: Some("Custom check passed".to_string()),
            last_check: current_timestamp(),
        };

        health_server
            .add_health_check("custom".to_string(), custom_check)
            .await;

        {
            let checks = health_server.additional_checks.read().await;
            assert!(checks.contains_key("custom"));
        } // Drop the read lock

        health_server.remove_health_check("custom").await;

        {
            let checks = health_server.additional_checks.read().await;
            assert!(!checks.contains_key("custom"));
        } // Drop the read lock
    }

    #[tokio::test]
    async fn test_overall_health_status() {
        let health_server = Arc::new(HealthServer::new("test-agent".to_string(), 8080));

        // Set up healthy state
        health_server.set_mqtt_connected(true).await;
        health_server
            .set_last_task_processed(current_timestamp())
            .await;

        let health_status = health_server.get_health_status().await.unwrap();
        assert_eq!(health_status.status, "healthy");
        assert_eq!(health_status.agent_id, "test-agent");
        assert!(health_status.checks.contains_key("mqtt"));
        assert!(health_status.checks.contains_key("task_processing"));

        // Set up degraded state
        health_server.set_mqtt_connected(false).await;

        let health_status = health_server.get_health_status().await.unwrap();
        assert_eq!(health_status.status, "degraded");
    }
}
