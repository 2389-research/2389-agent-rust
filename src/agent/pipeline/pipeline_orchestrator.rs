//! Main pipeline orchestrator for agent lifecycle management
//!
//! This module contains the primary AgentPipeline struct that coordinates
//! task processing using the 9-step algorithm with clean separation of concerns.

// TaskProcessor not needed - using AgentProcessor directly
use crate::agent::discovery::AgentRegistry;
use crate::agent::processor::AgentProcessor;
use crate::processing::nine_step::ProcessingResult;
use crate::protocol::messages::{
    TaskEnvelopeV2, TaskEnvelopeWrapper, WorkflowContext, WorkflowStep,
};
use crate::routing::{Router, RoutingDecision};
use crate::transport::Transport;
use chrono::Utc;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Maximum number of workflow steps to keep in history to prevent unbounded memory growth
const MAX_WORKFLOW_HISTORY_STEPS: usize = 100;

/// Agent pipeline that orchestrates the complete agent lifecycle
/// Supports both v1.0 and v2.0 TaskEnvelope formats
///
/// With V2 routing, the pipeline can optionally use a Router to make
/// intelligent workflow decisions after agent work completes.
pub struct AgentPipeline<T: Transport> {
    processor: AgentProcessor<T>,
    task_receiver: Option<mpsc::Receiver<TaskEnvelopeWrapper>>,
    max_pipeline_depth: usize,
    /// Optional V2 router for workflow decisions
    router: Option<Arc<dyn Router>>,
    /// Agent registry for router to query available agents
    agent_registry: Arc<AgentRegistry>,
    /// Maximum iterations before forced workflow completion
    max_iterations: usize,
}

/// Synthesize a default workflow context from a task envelope
///
/// Uses the task's instruction field as the original_query if available,
/// falling back to "Unknown" if the instruction is None or empty/whitespace.
fn synthesize_context_from_task(
    task: &TaskEnvelopeV2,
) -> crate::protocol::messages::WorkflowContext {
    let original_query = task
        .instruction
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "Unknown".to_string());

    crate::protocol::messages::WorkflowContext {
        original_query,
        steps_completed: vec![],
        iteration_count: 0,
    }
}

/// Cap workflow history to a maximum number of steps using FIFO
///
/// Removes oldest steps when the vector exceeds the specified maximum,
/// keeping only the most recent steps for routing decisions.
fn cap_workflow_steps(steps: &mut Vec<WorkflowStep>, max: usize) {
    if steps.len() > max {
        let overflow = steps.len() - max;
        steps.drain(0..overflow);
    }
}

impl<T: Transport + 'static> AgentPipeline<T> {
    /// Create new agent pipeline without V2 routing
    pub fn new(
        processor: AgentProcessor<T>,
        task_receiver: mpsc::Receiver<TaskEnvelopeWrapper>,
        max_pipeline_depth: usize,
    ) -> Self {
        Self {
            processor,
            task_receiver: Some(task_receiver),
            max_pipeline_depth,
            router: None,
            agent_registry: Arc::new(AgentRegistry::new()),
            max_iterations: 10,
        }
    }

    /// Create new agent pipeline with V2 routing support
    pub fn with_router(
        processor: AgentProcessor<T>,
        task_receiver: mpsc::Receiver<TaskEnvelopeWrapper>,
        max_pipeline_depth: usize,
        router: Arc<dyn Router>,
        agent_registry: Arc<AgentRegistry>,
        max_iterations: usize,
    ) -> Self {
        Self {
            processor,
            task_receiver: Some(task_receiver),
            max_pipeline_depth,
            router: Some(router),
            agent_registry,
            max_iterations,
        }
    }

    /// Get reference to the processor
    pub fn processor(&self) -> &AgentProcessor<T> {
        &self.processor
    }

    /// Start the pipeline - set up transport connections and subscriptions
    pub async fn start(&mut self) -> Result<(), PipelineError> {
        info!("Starting agent pipeline");
        // Note: AgentProcessor doesn't need explicit start - it's ready upon creation
        Ok(())
    }

    /// Main processing loop - runs until shutdown is requested
    pub async fn run(&mut self) -> Result<(), PipelineError> {
        info!("Agent pipeline running, waiting for tasks");

        let mut task_receiver = self.task_receiver.take().ok_or_else(|| {
            PipelineError::ProcessingFailed("Task receiver not available".to_string())
        })?;

        while let Some(task) = task_receiver.recv().await {
            self.process_single_task(task).await?;
        }

        info!("Pipeline processing loop ended");
        Ok(())
    }

    /// Calculate topic depth by counting non-empty segments
    fn calculate_topic_depth(topic: &str) -> usize {
        topic.split('/').filter(|s| !s.is_empty()).count()
    }

    /// Process a single task using the 9-step algorithm
    /// Supports both v1.0 and v2.0 TaskEnvelope formats
    ///
    /// For V2 tasks with router configured, this will:
    /// 1. Process the task (agent does work)
    /// 2. Invoke router to decide next step
    /// 3. Either complete workflow or forward to next agent
    pub async fn process_single_task(
        &self,
        wrapper: TaskEnvelopeWrapper,
    ) -> Result<ProcessingResult, PipelineError> {
        // Extract topic from wrapper
        let topic = match &wrapper {
            TaskEnvelopeWrapper::V1(env) => env.topic.clone(),
            TaskEnvelopeWrapper::V2(env) => env.topic.clone(),
        };
        let is_retained = false; // Assume not retained

        // VALIDATE TOPIC DEPTH: Prevent DoS attacks via deep topic nesting
        let topic_depth = Self::calculate_topic_depth(&topic);
        if topic_depth > self.max_pipeline_depth {
            error!(
                topic = %topic,
                depth = topic_depth,
                max_depth = self.max_pipeline_depth,
                "Topic depth exceeds maximum allowed depth"
            );
            return Err(PipelineError::PipelineDepthExceeded(topic_depth));
        }

        // Process the task (agent does its work)
        let result = self
            .processor
            .process_task(wrapper.clone(), &topic, is_retained)
            .await
            .map_err(|e| {
                error!("Task processing failed: {}", e);
                PipelineError::ProcessingFailed(e.to_string())
            })?;

        // V2 ROUTING: Check if we should invoke the router
        if let Some(_router) = &self.router {
            if let TaskEnvelopeWrapper::V2(task) = wrapper {
                debug!(
                    task_id = %task.task_id,
                    has_router = true,
                    "V2 task with router - invoking routing"
                );

                // Parse the response string to JSON for router
                let work_output: Value = serde_json::from_str(&result.response).map_err(|e| {
                    error!(
                        error = %e,
                        response = %result.response,
                        "Failed to parse agent response as JSON"
                    );
                    PipelineError::ProcessingFailed(format!(
                        "Agent response is not valid JSON: {e}"
                    ))
                })?;

                // Invoke V2 routing workflow
                self.process_with_routing(task, work_output).await?;

                info!(
                    task_id = %result.task_id,
                    "V2 routing workflow completed"
                );
            } else {
                debug!(
                    task_id = %result.task_id,
                    "V1 task or no router - using standard flow"
                );
            }
        }

        Ok(result)
    }

    /// Update agent status
    pub async fn update_status(
        &self,
        status: crate::protocol::messages::AgentStatusType,
    ) -> Result<(), PipelineError> {
        let status_msg = crate::protocol::messages::AgentStatus {
            agent_id: self.processor.config().agent.id.clone(),
            status: status.clone(),
            timestamp: chrono::Utc::now(),
            capabilities: None,
            description: None,
        };

        self.processor
            .transport()
            .publish_status(&status_msg)
            .await
            .map_err(|e| {
                error!("Failed to publish status: {}", e);
                PipelineError::TransportError(e.to_string())
            })?;

        debug!("Published status: {:?}", status);
        Ok(())
    }

    // ===== V2 Routing Methods =====

    /// Process task with V2 routing support
    ///
    /// This method:
    /// 1. Processes the task with the agent (does work)
    /// 2. Invokes the router to decide next step
    /// 3. Either completes workflow or forwards to next agent
    pub async fn process_with_routing(
        &self,
        task: TaskEnvelopeV2,
        work_output: Value,
    ) -> Result<(), PipelineError> {
        // Check if we have a router configured
        let router = self
            .router
            .as_ref()
            .ok_or_else(|| PipelineError::ProcessingFailed("No router configured".to_string()))?;

        info!(
            task_id = %task.task_id,
            iteration_count = task.context.as_ref().map(|c| c.iteration_count).unwrap_or(0),
            "Invoking router for workflow decision"
        );

        // Router decides next step
        let decision = router
            .decide_next_step(&task, &work_output, &self.agent_registry)
            .await
            .map_err(|e| PipelineError::ProcessingFailed(format!("Routing failed: {e}")))?;

        match decision {
            RoutingDecision::Complete { final_output } => {
                info!(
                    task_id = %task.task_id,
                    conversation_id = %task.conversation_id,
                    "Workflow complete, publishing final result"
                );

                // Publish final result to conversation topic
                self.publish_final_result(&task.conversation_id, &final_output)
                    .await?;
            }
            RoutingDecision::Forward {
                next_agent,
                next_instruction,
                forwarded_data,
            } => {
                info!(
                    task_id = %task.task_id,
                    next_agent = %next_agent,
                    next_instruction = %next_instruction,
                    "Forwarding to next agent"
                );

                // Forward to next agent with iteration enforcement
                self.forward_to_agent(&task, next_agent, next_instruction, forwarded_data)
                    .await?;
            }
        }

        Ok(())
    }

    /// Prepare workflow context - clone existing or synthesize default
    /// Pure function extracted for testability
    fn prepare_workflow_context(original_task: &TaskEnvelopeV2) -> WorkflowContext {
        match original_task.context.clone() {
            Some(ctx) => ctx,
            None => {
                // Context should exist in typical flows
                warn!(
                    task_id = %original_task.task_id,
                    conversation_id = %original_task.conversation_id,
                    "Missing workflow context on forward; synthesizing default context"
                );
                synthesize_context_from_task(original_task)
            }
        }
    }

    /// Increment iteration count and validate against max limit
    /// Returns Err if max iterations exceeded (with suggested final data)
    fn increment_and_validate_iterations(
        context: &mut WorkflowContext,
        max_iterations: usize,
        conversation_id: &str,
    ) -> Result<(), Value> {
        context.iteration_count += 1;

        if context.iteration_count >= max_iterations {
            warn!(
                conversation_id = %conversation_id,
                iteration_count = context.iteration_count,
                max_iterations = max_iterations,
                "Max iterations reached, completing workflow"
            );
            // Return Err with placeholder - caller should publish final result
            return Err(Value::Null);
        }

        Ok(())
    }

    /// Add current workflow step to history and cap if needed
    /// Pure function for workflow step management
    fn add_workflow_step(
        context: &mut WorkflowContext,
        agent_id: String,
        action: String,
        conversation_id: &str,
    ) {
        context.steps_completed.push(WorkflowStep {
            agent_id,
            action,
            timestamp: Utc::now().to_rfc3339(),
        });

        // Cap workflow history to prevent unbounded growth
        if context.steps_completed.len() > MAX_WORKFLOW_HISTORY_STEPS {
            let overflow = context.steps_completed.len() - MAX_WORKFLOW_HISTORY_STEPS;
            cap_workflow_steps(&mut context.steps_completed, MAX_WORKFLOW_HISTORY_STEPS);
            debug!(
                dropped = overflow,
                kept = MAX_WORKFLOW_HISTORY_STEPS,
                conversation_id = %conversation_id,
                "Workflow history exceeded cap; dropped oldest steps"
            );
        }
    }

    /// Create next task envelope for forwarding
    /// Pure function for task construction
    fn create_next_task_envelope(
        original_task: &TaskEnvelopeV2,
        next_agent: &str,
        next_instruction: String,
        forwarded_data: Value,
        new_context: WorkflowContext,
    ) -> TaskEnvelopeV2 {
        TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: original_task.conversation_id.clone(),
            topic: format!("/control/agents/{next_agent}/input"),
            instruction: Some(next_instruction),
            input: forwarded_data,
            next: None,
            version: "2.0".to_string(),
            context: Some(new_context),
            routing_trace: original_task.routing_trace.clone(),
        }
    }

    /// Forward task to next agent with iteration limit enforcement
    async fn forward_to_agent(
        &self,
        original_task: &TaskEnvelopeV2,
        next_agent: String,
        next_instruction: String,
        forwarded_data: Value,
    ) -> Result<(), PipelineError> {
        // Validate that the target agent exists in registry
        if self.agent_registry.get_agent(&next_agent).is_none() {
            warn!(
                next_agent = %next_agent,
                conversation_id = %original_task.conversation_id,
                "Router selected non-existent agent"
            );
            return Err(PipelineError::ProcessingFailed(format!(
                "Cannot forward to unknown agent: {next_agent}"
            )));
        }

        // Prepare workflow context
        let mut new_context = Self::prepare_workflow_context(original_task);

        // Increment and validate iteration count
        if Self::increment_and_validate_iterations(
            &mut new_context,
            self.max_iterations,
            &original_task.conversation_id,
        )
        .is_err()
        {
            return self
                .publish_final_result(&original_task.conversation_id, &forwarded_data)
                .await;
        }

        // Add current step to history
        Self::add_workflow_step(
            &mut new_context,
            self.processor.config().agent.id.clone(),
            next_instruction.clone(),
            &original_task.conversation_id,
        );

        // Create task for next agent
        let next_task = Self::create_next_task_envelope(
            original_task,
            &next_agent,
            next_instruction,
            forwarded_data,
            new_context,
        );

        // Publish to next agent's input topic
        let topic = next_task.topic.clone();
        let payload = serde_json::to_vec(&next_task).map_err(|e| {
            PipelineError::ProcessingFailed(format!("Failed to serialize task: {e}"))
        })?;

        self.processor
            .transport()
            .publish(&topic, payload, false)
            .await
            .map_err(|e| PipelineError::TransportError(e.to_string()))?;

        info!(
            next_agent = %next_agent,
            iteration_count = next_task.context.as_ref().map(|c| c.iteration_count).unwrap_or(0),
            "Forwarded task to next agent"
        );

        Ok(())
    }

    /// Publish final workflow result to conversation topic
    async fn publish_final_result(
        &self,
        conversation_id: &str,
        final_output: &Value,
    ) -> Result<(), PipelineError> {
        let topic = format!(
            "/conversations/{}/{}",
            conversation_id,
            self.processor.config().agent.id
        );

        let payload = serde_json::to_vec(final_output).map_err(|e| {
            PipelineError::ProcessingFailed(format!("Failed to serialize output: {e}"))
        })?;

        self.processor
            .transport()
            .publish(&topic, payload, false)
            .await
            .map_err(|e| PipelineError::TransportError(e.to_string()))?;

        info!(
            conversation_id = %conversation_id,
            topic = %topic,
            "Published final workflow result"
        );

        Ok(())
    }

    /// Shutdown the pipeline gracefully
    pub async fn shutdown(self) -> Result<(), PipelineError> {
        info!("Shutting down agent pipeline");

        // Update status to unavailable
        let pipeline_ref = &self;
        pipeline_ref
            .update_status(crate::protocol::messages::AgentStatusType::Unavailable)
            .await?;

        // Note: AgentProcessor doesn't need explicit shutdown - cleanup is automatic
        info!("Agent pipeline shutdown complete");
        Ok(())
    }
}

/// Errors that can occur during pipeline operations
#[derive(thiserror::Error, Debug)]
pub enum PipelineError {
    #[error("Processing failed: {0}")]
    ProcessingFailed(String),

    #[error("Transport error: {0}")]
    TransportError(String),

    #[error("Pipeline depth {0} exceeded maximum")]
    PipelineDepthExceeded(usize),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Shutdown error: {0}")]
    ShutdownError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::messages::{TaskEnvelopeV2, WorkflowStep};
    use serde_json::json;
    use uuid::Uuid;

    // ===== HELPER FUNCTION TESTS =====

    #[test]
    fn test_synthesize_context_with_instruction() {
        let task = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "conv1".to_string(),
            topic: "/test".to_string(),
            instruction: Some("Research Herodotus".to_string()),
            input: json!({}),
            next: None,
            version: "2.0".to_string(),
            context: None,
            routing_trace: None,
        };

        let context = synthesize_context_from_task(&task);
        assert_eq!(context.original_query, "Research Herodotus");
        assert_eq!(context.steps_completed.len(), 0);
        assert_eq!(context.iteration_count, 0);
    }

    #[test]
    fn test_synthesize_context_with_whitespace_instruction() {
        let task = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "conv1".to_string(),
            topic: "/test".to_string(),
            instruction: Some("   ".to_string()),
            input: json!({}),
            next: None,
            version: "2.0".to_string(),
            context: None,
            routing_trace: None,
        };

        let context = synthesize_context_from_task(&task);
        assert_eq!(context.original_query, "Unknown");
    }

    #[test]
    fn test_synthesize_context_with_no_instruction() {
        let task = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "conv1".to_string(),
            topic: "/test".to_string(),
            instruction: None,
            input: json!({}),
            next: None,
            version: "2.0".to_string(),
            context: None,
            routing_trace: None,
        };

        let context = synthesize_context_from_task(&task);
        assert_eq!(context.original_query, "Unknown");
    }

    #[test]
    fn test_cap_workflow_steps_below_limit() {
        let mut steps = vec![
            WorkflowStep {
                agent_id: "agent1".to_string(),
                action: "action1".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
            },
            WorkflowStep {
                agent_id: "agent2".to_string(),
                action: "action2".to_string(),
                timestamp: "2024-01-01T00:01:00Z".to_string(),
            },
        ];

        cap_workflow_steps(&mut steps, 10);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].agent_id, "agent1");
    }

    #[test]
    fn test_cap_workflow_steps_at_limit() {
        let mut steps = vec![
            WorkflowStep {
                agent_id: "agent1".to_string(),
                action: "action1".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
            },
            WorkflowStep {
                agent_id: "agent2".to_string(),
                action: "action2".to_string(),
                timestamp: "2024-01-01T00:01:00Z".to_string(),
            },
        ];

        cap_workflow_steps(&mut steps, 2);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].agent_id, "agent1");
    }

    #[test]
    fn test_cap_workflow_steps_exceeds_limit() {
        let mut steps = vec![
            WorkflowStep {
                agent_id: "agent1".to_string(),
                action: "action1".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
            },
            WorkflowStep {
                agent_id: "agent2".to_string(),
                action: "action2".to_string(),
                timestamp: "2024-01-01T00:02:00Z".to_string(),
            },
            WorkflowStep {
                agent_id: "agent3".to_string(),
                action: "action3".to_string(),
                timestamp: "2024-01-01T00:03:00Z".to_string(),
            },
            WorkflowStep {
                agent_id: "agent4".to_string(),
                action: "action4".to_string(),
                timestamp: "2024-01-01T00:04:00Z".to_string(),
            },
            WorkflowStep {
                agent_id: "agent5".to_string(),
                action: "action5".to_string(),
                timestamp: "2024-01-01T00:05:00Z".to_string(),
            },
        ];

        cap_workflow_steps(&mut steps, 3);
        assert_eq!(steps.len(), 3);
        // Should keep the last 3 (agent3, agent4, agent5)
        assert_eq!(steps[0].agent_id, "agent3");
        assert_eq!(steps[1].agent_id, "agent4");
        assert_eq!(steps[2].agent_id, "agent5");
    }

    // ===== EXTRACTED PURE FUNCTION TESTS =====

    #[test]
    fn test_prepare_workflow_context_with_existing_context() {
        let existing_context = WorkflowContext {
            original_query: "Test query".to_string(),
            steps_completed: vec![],
            iteration_count: 5,
        };

        let task = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "conv1".to_string(),
            topic: "/test".to_string(),
            instruction: Some("Test".to_string()),
            input: json!({}),
            next: None,
            version: "2.0".to_string(),
            context: Some(existing_context.clone()),
            routing_trace: None,
        };

        let result =
            AgentPipeline::<crate::testing::mocks::MockTransport>::prepare_workflow_context(&task);
        assert_eq!(result.original_query, "Test query");
        assert_eq!(result.iteration_count, 5);
    }

    #[test]
    fn test_prepare_workflow_context_synthesizes_when_missing() {
        let task = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "conv1".to_string(),
            topic: "/test".to_string(),
            instruction: Some("Synthesized".to_string()),
            input: json!({}),
            next: None,
            version: "2.0".to_string(),
            context: None,
            routing_trace: None,
        };

        let result =
            AgentPipeline::<crate::testing::mocks::MockTransport>::prepare_workflow_context(&task);
        assert_eq!(result.original_query, "Synthesized");
        assert_eq!(result.iteration_count, 0);
    }

    #[test]
    fn test_increment_and_validate_iterations_below_limit() {
        let mut context = WorkflowContext {
            original_query: "Test".to_string(),
            steps_completed: vec![],
            iteration_count: 3,
        };

        let result = AgentPipeline::<crate::testing::mocks::MockTransport>::increment_and_validate_iterations(
            &mut context,
            10,
            "conv1",
        );

        assert!(result.is_ok(), "Should succeed when below limit");
        assert_eq!(context.iteration_count, 4, "Should increment count");
    }

    #[test]
    fn test_increment_and_validate_iterations_at_limit() {
        let mut context = WorkflowContext {
            original_query: "Test".to_string(),
            steps_completed: vec![],
            iteration_count: 9,
        };

        let result = AgentPipeline::<crate::testing::mocks::MockTransport>::increment_and_validate_iterations(
            &mut context,
            10,
            "conv1",
        );

        assert!(result.is_err(), "Should fail when at limit");
        assert_eq!(
            context.iteration_count, 10,
            "Should still increment before failing"
        );
    }

    #[test]
    fn test_increment_and_validate_iterations_exceeds_limit() {
        let mut context = WorkflowContext {
            original_query: "Test".to_string(),
            steps_completed: vec![],
            iteration_count: 15,
        };

        let result = AgentPipeline::<crate::testing::mocks::MockTransport>::increment_and_validate_iterations(
            &mut context,
            10,
            "conv1",
        );

        assert!(result.is_err(), "Should fail when exceeding limit");
        assert_eq!(context.iteration_count, 16);
    }

    #[test]
    fn test_add_workflow_step_below_cap() {
        let mut context = WorkflowContext {
            original_query: "Test".to_string(),
            steps_completed: vec![WorkflowStep {
                agent_id: "agent1".to_string(),
                action: "action1".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
            }],
            iteration_count: 1,
        };

        AgentPipeline::<crate::testing::mocks::MockTransport>::add_workflow_step(
            &mut context,
            "agent2".to_string(),
            "action2".to_string(),
            "conv1",
        );

        assert_eq!(context.steps_completed.len(), 2);
        assert_eq!(context.steps_completed[1].agent_id, "agent2");
        assert_eq!(context.steps_completed[1].action, "action2");
    }

    #[test]
    fn test_add_workflow_step_triggers_cap() {
        let mut context = WorkflowContext {
            original_query: "Test".to_string(),
            steps_completed: (0..MAX_WORKFLOW_HISTORY_STEPS)
                .map(|i| WorkflowStep {
                    agent_id: format!("agent{i}"),
                    action: format!("action{i}"),
                    timestamp: "2024-01-01T00:00:00Z".to_string(),
                })
                .collect(),
            iteration_count: MAX_WORKFLOW_HISTORY_STEPS,
        };

        let _initial_len = context.steps_completed.len();

        AgentPipeline::<crate::testing::mocks::MockTransport>::add_workflow_step(
            &mut context,
            "new_agent".to_string(),
            "new_action".to_string(),
            "conv1",
        );

        // Should be capped at max after adding
        assert_eq!(context.steps_completed.len(), MAX_WORKFLOW_HISTORY_STEPS);
        // Last step should be the new one
        assert_eq!(
            context.steps_completed.last().unwrap().agent_id,
            "new_agent"
        );
    }

    #[test]
    fn test_create_next_task_envelope() {
        let original_context = WorkflowContext {
            original_query: "Original query".to_string(),
            steps_completed: vec![],
            iteration_count: 3,
        };

        let original_task = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "conv123".to_string(),
            topic: "/control/agents/agent1/input".to_string(),
            instruction: Some("Original instruction".to_string()),
            input: json!({"key": "value"}),
            next: None,
            version: "2.0".to_string(),
            context: Some(original_context.clone()),
            routing_trace: Some(vec![]),
        };

        let new_context = WorkflowContext {
            original_query: "Original query".to_string(),
            steps_completed: vec![WorkflowStep {
                agent_id: "agent1".to_string(),
                action: "completed_action".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
            }],
            iteration_count: 4,
        };

        let result =
            AgentPipeline::<crate::testing::mocks::MockTransport>::create_next_task_envelope(
                &original_task,
                "agent2",
                "Next instruction".to_string(),
                json!({"forwarded": "data"}),
                new_context.clone(),
            );

        assert_eq!(result.conversation_id, "conv123");
        assert_eq!(result.topic, "/control/agents/agent2/input");
        assert_eq!(result.instruction, Some("Next instruction".to_string()));
        assert_eq!(result.input, json!({"forwarded": "data"}));
        assert_eq!(result.version, "2.0");
        assert_eq!(result.context.unwrap().iteration_count, 4);
    }
}
