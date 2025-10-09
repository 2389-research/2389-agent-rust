//! Gatekeeper Router Implementation
//!
//! This module implements the Router trait using an external HTTP service to make
//! routing decisions. This allows users to implement custom routing logic in their
//! own services while maintaining compatibility with the 2389 Agent Protocol.
//!
//! # Architecture
//!
//! The GatekeeperRouter delegates routing decisions to an external HTTP endpoint:
//! - Sends workflow context to external service via POST request
//! - Receives routing decision (complete vs forward)
//! - Implements retry logic with exponential backoff
//! - Enforces configurable timeouts
//! - Maps HTTP errors to AgentError types
//! - Flexible configuration (host, port, scheme, path)
//!
//! # Gatekeeper API Integration
//!
//! This router is designed to work with the official Gatekeeper API service.
//! The Gatekeeper API provides several endpoints:
//!
//! - `/health` - Health check endpoint
//! - `/ready` - Readiness check (model loaded)
//! - `/should_agents_respond` - Agent routing decisions (default)
//! - `/agents_to_add_to_chat` - Agent addition recommendations (v1)
//! - `/agents_to_add_to_chat_v1_5` - Agent addition with gpt-3.5-turbo
//! - `/agents_to_add_to_chat_v2` - Optimized agent addition (cached embeddings)
//!
//! # Request Format
//!
//! ```json
//! {
//!   "original_query": "User's original request",
//!   "workflow_history": [...],
//!   "current_output": {...},
//!   "available_agents": [...],
//!   "iteration_count": 2
//! }
//! ```
//!
//! # Response Format
//!
//! ```json
//! {
//!   "workflow_complete": false,
//!   "next_agent": "agent-id",
//!   "next_instruction": "What to do next",
//!   "reasoning": "Why this decision was made"
//! }
//! ```
//!
//! # Example - Using Builder Pattern
//!
//! ```no_run
//! use agent2389::routing::gatekeeper_router::{GatekeeperRouter, GatekeeperConfig};
//! use agent2389::routing::router::Router;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Configure for production Gatekeeper service with HTTPS
//! let config = GatekeeperConfig::new()
//!     .with_host("gatekeeper.example.com")
//!     .with_port(443)
//!     .with_scheme("https")
//!     .with_path("/should_agents_respond")
//!     .with_timeout_ms(5000)
//!     .with_retry_attempts(3);
//!
//! let router = GatekeeperRouter::new(config);
//!
//! // Router automatically handles retries and timeouts
//! // let decision = router.decide_next_step(&task, &output, &registry).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Example - Local Development
//!
//! ```no_run
//! use agent2389::routing::gatekeeper_router::{GatekeeperRouter, GatekeeperConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Local Gatekeeper service on default port
//! let config = GatekeeperConfig::new()
//!     .with_host("localhost")
//!     .with_port(8000)
//!     .with_path("/should_agents_respond");
//!
//! let router = GatekeeperRouter::new(config);
//! # Ok(())
//! # }
//! ```

use crate::agent::discovery::AgentRegistry;
use crate::error::AgentError;
use crate::protocol::messages::{TaskEnvelopeV2, WorkflowStep};
use crate::routing::router::{Router, RoutingDecision};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, info, warn};

/// HTTP-based router that delegates routing decisions to an external service
///
/// The GatekeeperRouter allows users to implement custom routing logic in their
/// own services. This is useful for:
/// - Complex routing rules that don't fit in LLM prompts
/// - Integration with existing routing systems
/// - Custom decision logic based on business rules
/// - Rate limiting or cost-aware routing
pub struct GatekeeperRouter {
    /// Base configuration for the Gatekeeper service
    config: GatekeeperConfig,
    /// HTTP client for making requests
    client: reqwest::Client,
}

/// Configuration for the Gatekeeper HTTP service
///
/// Provides flexible configuration for connecting to Gatekeeper API services
/// with support for different schemes (http/https), custom ports, and API paths.
#[derive(Debug, Clone)]
pub struct GatekeeperConfig {
    /// Hostname or IP address (e.g., "localhost", "gatekeeper.example.com")
    pub host: String,
    /// Port number (e.g., 8080)
    pub port: u16,
    /// URL scheme - "http" or "https"
    pub scheme: String,
    /// API endpoint path (e.g., "/should_agents_respond", "/agents_to_add_to_chat")
    pub path: String,
    /// Request timeout in milliseconds
    pub timeout_ms: u64,
    /// Number of retry attempts for transient failures (5xx errors)
    pub retry_attempts: usize,
}

impl Default for GatekeeperConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 8000,
            scheme: "http".to_string(),
            path: "/should_agents_respond".to_string(),
            timeout_ms: 5000,
            retry_attempts: 3,
        }
    }
}

impl GatekeeperConfig {
    /// Create a new GatekeeperConfig with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the hostname
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Set the port
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set the URL scheme (http or https)
    pub fn with_scheme(mut self, scheme: impl Into<String>) -> Self {
        self.scheme = scheme.into();
        self
    }

    /// Set the API endpoint path
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    /// Set the request timeout in milliseconds
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Set the number of retry attempts
    pub fn with_retry_attempts(mut self, retry_attempts: usize) -> Self {
        self.retry_attempts = retry_attempts;
        self
    }

    /// Build the full URL from configuration
    pub fn build_url(&self) -> String {
        format!("{}://{}:{}{}", self.scheme, self.host, self.port, self.path)
    }

    /// Build timeout Duration
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }
}

impl GatekeeperRouter {
    /// Create a new GatekeeperRouter with custom configuration
    ///
    /// # Arguments
    ///
    /// * `config` - GatekeeperConfig with host, port, scheme, path, timeout, and retry settings
    ///
    /// # Example
    ///
    /// ```
    /// use agent2389::routing::gatekeeper_router::{GatekeeperRouter, GatekeeperConfig};
    ///
    /// // Using builder pattern for configuration
    /// let config = GatekeeperConfig::new()
    ///     .with_host("gatekeeper.example.com")
    ///     .with_port(8080)
    ///     .with_scheme("https")
    ///     .with_path("/should_agents_respond")
    ///     .with_timeout_ms(5000)
    ///     .with_retry_attempts(3);
    ///
    /// let router = GatekeeperRouter::new(config);
    /// ```
    pub fn new(config: GatekeeperConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Create a new GatekeeperRouter from a full URL (legacy convenience method)
    ///
    /// # Arguments
    ///
    /// * `url` - Full URL of the routing endpoint (e.g., "http://localhost:8080/route")
    /// * `timeout_ms` - Request timeout in milliseconds
    /// * `retry_attempts` - Number of retry attempts for transient failures (5xx errors)
    ///
    /// # Example
    ///
    /// ```
    /// use agent2389::routing::gatekeeper_router::GatekeeperRouter;
    ///
    /// let router = GatekeeperRouter::from_url(
    ///     "http://localhost:8080/route".to_string(),
    ///     5000,  // 5 second timeout
    ///     3      // 3 retry attempts
    /// );
    /// ```
    pub fn from_url(url: String, timeout_ms: u64, retry_attempts: usize) -> Self {
        // For backward compatibility, store the full URL in the config
        // The build_url method will not be used in this case
        let config = GatekeeperConfig {
            host: url,
            port: 0,
            scheme: String::new(),
            path: String::new(),
            timeout_ms,
            retry_attempts,
        };

        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Get the URL to use for API calls
    fn url(&self) -> String {
        // If scheme is empty, assume host contains the full URL (legacy mode)
        if self.config.scheme.is_empty() {
            self.config.host.clone()
        } else {
            self.config.build_url()
        }
    }
}

/// Request sent to external routing service
#[derive(Debug, Clone, Serialize)]
struct GatekeeperRequest {
    /// The original user query that started the workflow
    original_query: String,
    /// History of all workflow steps completed so far
    workflow_history: Vec<WorkflowStep>,
    /// The current agent's output
    current_output: Value,
    /// List of available agents with their capabilities
    available_agents: Vec<AgentSummary>,
    /// Current iteration count in the workflow
    iteration_count: usize,
}

/// Simplified agent information for routing decisions
#[derive(Debug, Clone, Serialize)]
struct AgentSummary {
    /// Agent identifier
    agent_id: String,
    /// Agent capabilities (if any)
    capabilities: Vec<String>,
    /// Current load (0.0 = idle, 1.0 = fully loaded)
    load: f32,
}

/// Response from external routing service
#[derive(Debug, Clone, Deserialize)]
struct GatekeeperResponse {
    /// Whether the workflow is complete
    workflow_complete: bool,
    /// Next agent to forward to (if not complete)
    next_agent: Option<String>,
    /// Instruction for the next agent (if not complete)
    next_instruction: Option<String>,
    /// Reasoning for the routing decision (optional)
    reasoning: Option<String>,
}

#[async_trait::async_trait]
impl Router for GatekeeperRouter {
    async fn decide_next_step(
        &self,
        original_task: &TaskEnvelopeV2,
        work_output: &Value,
        registry: &AgentRegistry,
    ) -> Result<RoutingDecision, AgentError> {
        info!("GatekeeperRouter making routing decision");

        // Build request payload
        let request = self.build_request(original_task, work_output, registry);

        // Call external service with retry logic
        let response = self.call_external_api(&request).await?;

        // Convert response to RoutingDecision
        self.parse_response(&response, work_output)
    }
}

impl GatekeeperRouter {
    /// Build request payload for external routing service
    fn build_request(
        &self,
        task: &TaskEnvelopeV2,
        work_output: &Value,
        registry: &AgentRegistry,
    ) -> GatekeeperRequest {
        let context = task.context.as_ref();

        let original_query = context
            .map(|c| c.original_query.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        let workflow_history = context
            .map(|c| c.steps_completed.clone())
            .unwrap_or_default();

        let iteration_count = context.map(|c| c.iteration_count).unwrap_or(0);

        // Get available agents
        let agent_ids = registry.get_all_agent_ids();
        let available_agents: Vec<AgentSummary> = agent_ids
            .iter()
            .filter_map(|id| registry.get_agent(id))
            .filter(|agent| agent.is_healthy() && !agent.is_expired())
            .map(|agent| AgentSummary {
                agent_id: agent.agent_id.clone(),
                capabilities: agent.capabilities.clone().unwrap_or_default(),
                load: agent.load as f32,
            })
            .collect();

        GatekeeperRequest {
            original_query,
            workflow_history,
            current_output: work_output.clone(),
            available_agents,
            iteration_count,
        }
    }

    /// Call external routing API with retry logic
    async fn call_external_api(
        &self,
        request: &GatekeeperRequest,
    ) -> Result<GatekeeperResponse, AgentError> {
        let mut last_error = None;
        let url = self.url();
        let timeout = self.config.timeout();
        let retry_attempts = self.config.retry_attempts;

        for attempt in 0..=retry_attempts {
            debug!(
                attempt = attempt + 1,
                max_attempts = retry_attempts + 1,
                url = %url,
                "Calling gatekeeper routing service"
            );

            match self
                .client
                .post(&url)
                .json(request)
                .timeout(timeout)
                .send()
                .await
            {
                Ok(response) => {
                    let status = response.status();

                    if status.is_success() {
                        // Success - parse response
                        let body =
                            response
                                .text()
                                .await
                                .map_err(|e| AgentError::InternalError {
                                    message: format!("Failed to read response body: {e}"),
                                })?;

                        let parsed: GatekeeperResponse =
                            serde_json::from_str(&body).map_err(|e| AgentError::InvalidInput {
                                message: format!("Invalid JSON response from gatekeeper: {e}"),
                            })?;

                        info!(
                            workflow_complete = parsed.workflow_complete,
                            "Received routing decision from gatekeeper"
                        );

                        return Ok(parsed);
                    } else if status.is_server_error() && attempt < retry_attempts {
                        // Server error - retry
                        warn!(
                            status = %status,
                            attempt = attempt + 1,
                            "Gatekeeper returned server error, retrying..."
                        );

                        // Exponential backoff
                        let backoff_ms = 100 * 2_u64.pow(attempt as u32);
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;

                        last_error = Some(format!("Server error: {status}"));
                        continue;
                    } else {
                        // Client error or final retry - return error
                        return Err(AgentError::InternalError {
                            message: format!("Gatekeeper routing failed with status: {status}"),
                        });
                    }
                }
                Err(e) if e.is_timeout() => {
                    return Err(AgentError::InternalError {
                        message: format!("Gatekeeper routing timeout after {timeout:?}"),
                    });
                }
                Err(e) if attempt < retry_attempts => {
                    // Network error - retry
                    warn!(
                        error = %e,
                        attempt = attempt + 1,
                        "Gatekeeper network error, retrying..."
                    );

                    let backoff_ms = 100 * 2_u64.pow(attempt as u32);
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;

                    last_error = Some(format!("Network error: {e}"));
                    continue;
                }
                Err(e) => {
                    // Final retry failed
                    return Err(AgentError::InternalError {
                        message: format!("Gatekeeper routing failed: {e}"),
                    });
                }
            }
        }

        // All retries exhausted
        Err(AgentError::InternalError {
            message: format!(
                "Gatekeeper routing failed after {} retries: {}",
                retry_attempts,
                last_error.unwrap_or_else(|| "Unknown error".to_string())
            ),
        })
    }

    /// Parse gatekeeper response into RoutingDecision
    fn parse_response(
        &self,
        response: &GatekeeperResponse,
        work_output: &Value,
    ) -> Result<RoutingDecision, AgentError> {
        if let Some(reasoning) = &response.reasoning {
            debug!(reasoning = %reasoning, "Gatekeeper reasoning");
        }

        if response.workflow_complete {
            Ok(RoutingDecision::Complete {
                final_output: work_output.clone(),
            })
        } else {
            let next_agent =
                response
                    .next_agent
                    .as_ref()
                    .ok_or_else(|| AgentError::InvalidInput {
                        message: "Gatekeeper response missing next_agent".to_string(),
                    })?;

            let next_instruction =
                response
                    .next_instruction
                    .as_ref()
                    .ok_or_else(|| AgentError::InvalidInput {
                        message: "Gatekeeper response missing next_instruction".to_string(),
                    })?;

            Ok(RoutingDecision::Forward {
                next_agent: next_agent.clone(),
                next_instruction: next_instruction.clone(),
                forwarded_data: work_output.clone(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::discovery::AgentInfo;
    use crate::protocol::messages::WorkflowContext;
    use serde_json::json;
    use uuid::Uuid;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_gatekeeper_successful_forward() {
        // Setup: Start mock HTTP server
        let mock_server = MockServer::start().await;

        // Configure mock to return a forward decision
        Mock::given(method("POST"))
            .and(path("/route"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "workflow_complete": false,
                "next_agent": "editor-agent",
                "next_instruction": "Polish the document",
                "reasoning": "Document needs editing"
            })))
            .mount(&mock_server)
            .await;

        // Create router pointing to mock server using from_url for convenience
        let router = GatekeeperRouter::from_url(format!("{}/route", mock_server.uri()), 5000, 3);

        // Create test task
        let task = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "test-conv".to_string(),
            topic: "/test".to_string(),
            instruction: Some("Test instruction".to_string()),
            input: json!({}),
            next: None,
            version: "2.0".to_string(),
            context: Some(WorkflowContext {
                original_query: "Write a blog post".to_string(),
                steps_completed: vec![],
                iteration_count: 1,
            }),
            routing_trace: None,
        };

        let work_output = json!({"draft": "This is my blog post..."});
        let registry = AgentRegistry::new();

        // Register an agent so it appears in available_agents
        let mut agent = AgentInfo::new("editor-agent".to_string(), "ok".to_string(), 0.5);
        agent.capabilities = Some(vec!["editing".to_string()]);
        registry.register_agent(agent);

        // Execute: Call decide_next_step
        let decision = router
            .decide_next_step(&task, &work_output, &registry)
            .await;

        // Assert: Should return Forward decision
        assert!(decision.is_ok());
        let decision = decision.unwrap();
        assert!(decision.is_forward());
        assert_eq!(decision.next_agent(), Some("editor-agent"));
    }

    #[tokio::test]
    async fn test_gatekeeper_successful_complete() {
        // Setup: Start mock HTTP server
        let mock_server = MockServer::start().await;

        // Configure mock to return a complete decision
        Mock::given(method("POST"))
            .and(path("/route"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "workflow_complete": true,
                "reasoning": "Task is complete"
            })))
            .mount(&mock_server)
            .await;

        let router = GatekeeperRouter::from_url(format!("{}/route", mock_server.uri()), 5000, 3);

        let task = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "test-conv".to_string(),
            topic: "/test".to_string(),
            instruction: Some("Test instruction".to_string()),
            input: json!({}),
            next: None,
            version: "2.0".to_string(),
            context: Some(WorkflowContext {
                original_query: "Complete task".to_string(),
                steps_completed: vec![],
                iteration_count: 1,
            }),
            routing_trace: None,
        };

        let work_output = json!({"result": "Task completed successfully"});
        let registry = AgentRegistry::new();

        let decision = router
            .decide_next_step(&task, &work_output, &registry)
            .await;

        // Assert: Should return Complete decision
        assert!(decision.is_ok());
        let decision = decision.unwrap();
        assert!(decision.is_complete());
        assert!(!decision.is_forward());
    }

    #[tokio::test]
    async fn test_gatekeeper_retry_on_500() {
        // Setup: Start mock HTTP server
        let mock_server = MockServer::start().await;

        // Use wiremock's up_to_n_times feature to simulate: 500 then 200
        // First request returns 500
        Mock::given(method("POST"))
            .and(path("/route"))
            .respond_with(ResponseTemplate::new(500))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        // Second request returns 200
        Mock::given(method("POST"))
            .and(path("/route"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "workflow_complete": true,
                "reasoning": "Success on retry"
            })))
            .mount(&mock_server)
            .await;

        let router = GatekeeperRouter::from_url(format!("{}/route", mock_server.uri()), 5000, 3);

        let task = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "test-conv".to_string(),
            topic: "/test".to_string(),
            instruction: None,
            input: json!({}),
            next: None,
            version: "2.0".to_string(),
            context: Some(WorkflowContext {
                original_query: "Test".to_string(),
                steps_completed: vec![],
                iteration_count: 0,
            }),
            routing_trace: None,
        };

        let work_output = json!({});
        let registry = AgentRegistry::new();

        // Should succeed on retry
        let decision = router
            .decide_next_step(&task, &work_output, &registry)
            .await;

        assert!(decision.is_ok());
    }

    #[tokio::test]
    async fn test_gatekeeper_timeout() {
        // Setup: Start mock HTTP server
        let mock_server = MockServer::start().await;

        // Mock server delays response beyond timeout
        Mock::given(method("POST"))
            .and(path("/route"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"workflow_complete": true}))
                    .set_delay(Duration::from_secs(10)), // 10 second delay
            )
            .mount(&mock_server)
            .await;

        // Router has 100ms timeout
        let router = GatekeeperRouter::from_url(format!("{}/route", mock_server.uri()), 100, 0);

        let task = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "test-conv".to_string(),
            topic: "/test".to_string(),
            instruction: None,
            input: json!({}),
            next: None,
            version: "2.0".to_string(),
            context: None,
            routing_trace: None,
        };

        let work_output = json!({});
        let registry = AgentRegistry::new();

        let decision = router
            .decide_next_step(&task, &work_output, &registry)
            .await;

        // Should return timeout error
        assert!(decision.is_err());
        let err = decision.unwrap_err();
        assert!(err.to_string().contains("timeout"));
    }

    #[tokio::test]
    async fn test_gatekeeper_404_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/route"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let router = GatekeeperRouter::from_url(format!("{}/route", mock_server.uri()), 5000, 3);

        let task = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "test-conv".to_string(),
            topic: "/test".to_string(),
            instruction: None,
            input: json!({}),
            next: None,
            version: "2.0".to_string(),
            context: None,
            routing_trace: None,
        };

        let work_output = json!({});
        let registry = AgentRegistry::new();

        let decision = router
            .decide_next_step(&task, &work_output, &registry)
            .await;

        // Should return error for 404
        assert!(decision.is_err());
    }

    #[tokio::test]
    async fn test_gatekeeper_invalid_json() {
        let mock_server = MockServer::start().await;

        // Return 200 but with invalid JSON
        Mock::given(method("POST"))
            .and(path("/route"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not valid json"))
            .mount(&mock_server)
            .await;

        let router = GatekeeperRouter::from_url(format!("{}/route", mock_server.uri()), 5000, 3);

        let task = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "test-conv".to_string(),
            topic: "/test".to_string(),
            instruction: None,
            input: json!({}),
            next: None,
            version: "2.0".to_string(),
            context: None,
            routing_trace: None,
        };

        let work_output = json!({});
        let registry = AgentRegistry::new();

        let decision = router
            .decide_next_step(&task, &work_output, &registry)
            .await;

        // Should return JSON parse error
        assert!(decision.is_err());
        let err = decision.unwrap_err();
        assert!(err.to_string().contains("Invalid JSON"));
    }

    #[tokio::test]
    async fn test_gatekeeper_network_error() {
        // Use a URL that will fail to connect
        let router = GatekeeperRouter::from_url("http://localhost:1".to_string(), 1000, 2);

        let task = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "test-conv".to_string(),
            topic: "/test".to_string(),
            instruction: None,
            input: json!({}),
            next: None,
            version: "2.0".to_string(),
            context: None,
            routing_trace: None,
        };

        let work_output = json!({});
        let registry = AgentRegistry::new();

        let decision = router
            .decide_next_step(&task, &work_output, &registry)
            .await;

        // Should return network error
        assert!(decision.is_err());
    }

    #[tokio::test]
    async fn test_gatekeeper_config_builder() {
        // Setup: Start mock HTTP server
        let mock_server = MockServer::start().await;

        // Parse mock server URL to get host and port
        let mock_url = url::Url::parse(&mock_server.uri()).unwrap();
        let host = mock_url.host_str().unwrap().to_string();
        let port = mock_url.port().unwrap();

        // Configure mock to return a complete decision
        Mock::given(method("POST"))
            .and(path("/agents_to_add_to_chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "workflow_complete": true,
                "reasoning": "Using config builder pattern"
            })))
            .mount(&mock_server)
            .await;

        // Create router using builder pattern
        let config = GatekeeperConfig::new()
            .with_host(host)
            .with_port(port)
            .with_scheme("http")
            .with_path("/agents_to_add_to_chat")
            .with_timeout_ms(5000)
            .with_retry_attempts(2);

        let router = GatekeeperRouter::new(config);

        let task = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "test-conv".to_string(),
            topic: "/test".to_string(),
            instruction: None,
            input: json!({}),
            next: None,
            version: "2.0".to_string(),
            context: None,
            routing_trace: None,
        };

        let work_output = json!({"result": "Test using config builder"});
        let registry = AgentRegistry::new();

        let decision = router
            .decide_next_step(&task, &work_output, &registry)
            .await;

        // Should successfully complete
        assert!(decision.is_ok());
        let decision = decision.unwrap();
        assert!(decision.is_complete());
    }
}
