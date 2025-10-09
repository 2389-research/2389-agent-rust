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
//! # Example
//!
//! ```no_run
//! use agent2389::routing::gatekeeper_router::GatekeeperRouter;
//! use agent2389::routing::router::Router;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let router = GatekeeperRouter::new(
//!     "http://localhost:8080/route".to_string(),
//!     5000,  // 5 second timeout
//!     3      // 3 retry attempts
//! );
//!
//! // Router automatically handles retries and timeouts
//! // let decision = router.decide_next_step(&task, &output, &registry).await?;
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
    /// URL of the external routing service
    url: String,
    /// Request timeout in milliseconds
    timeout: Duration,
    /// Number of retry attempts for transient failures
    retry_attempts: usize,
    /// HTTP client for making requests
    client: reqwest::Client,
}

impl GatekeeperRouter {
    /// Create a new GatekeeperRouter
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
    /// let router = GatekeeperRouter::new(
    ///     "http://localhost:8080/route".to_string(),
    ///     5000,  // 5 second timeout
    ///     3      // 3 retry attempts
    /// );
    /// ```
    pub fn new(url: String, timeout_ms: u64, retry_attempts: usize) -> Self {
        Self {
            url,
            timeout: Duration::from_millis(timeout_ms),
            retry_attempts,
            client: reqwest::Client::new(),
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

        for attempt in 0..=self.retry_attempts {
            debug!(
                attempt = attempt + 1,
                max_attempts = self.retry_attempts + 1,
                "Calling gatekeeper routing service"
            );

            match self
                .client
                .post(&self.url)
                .json(request)
                .timeout(self.timeout)
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
                    } else if status.is_server_error() && attempt < self.retry_attempts {
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
                        message: format!("Gatekeeper routing timeout after {:?}", self.timeout),
                    });
                }
                Err(e) if attempt < self.retry_attempts => {
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
                self.retry_attempts,
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

        // Create router pointing to mock server
        let router = GatekeeperRouter::new(format!("{}/route", mock_server.uri()), 5000, 3);

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
}
