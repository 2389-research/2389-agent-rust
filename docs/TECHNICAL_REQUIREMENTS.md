# 2389 Agent Protocol Rust Implementation

## Technical Design Document for Autonomous Coding Agents

### Executive Summary

This document provides a complete technical specification for implementing the 2389 Agent Protocol in Rust,
specifically designed for autonomous coding agents like Claude Code. The implementation will create a
production-ready, interoperable AI agent system using MQTT as the transport layer.

**Key Deliverables:**

- Complete Rust library (`agent2389`) implementing the protocol specification
- Command-line agent runner with configuration management
- Comprehensive tool system with built-in and extensible tools
- Production-ready error handling and observability
- Full test coverage and documentation

---

## Project Context for AI Agents

### CLAUDE.md Integration Points

This specification includes explicit guidance for CLAUDE.md files that will be automatically created during
implementation to ensure optimal autonomous agent performance.

### Implementation Philosophy

**For Autonomous Agents:** This crate prioritizes clear, explicit patterns over clever abstractions. Every public
API includes comprehensive documentation with examples. Error types map directly to protocol requirements. The
architecture follows "pit of success" principles - it's easier to implement correctly than incorrectly.

**Code Quality Standards:**

- Use `#[must_use]` on all Result types
- Implement Display and Debug for all public types
- Include rustdoc examples for all public functions
- Follow the "return early" pattern consistently
- Use specific error types over generic ones
- Log errors with sufficient context for troubleshooting

---

## Architecture Overview

### Core Components

```rust
// Primary crate structure
agent2389/
├── src/
│   ├── lib.rs              // Public API surface
│   ├── agent/              // Agent lifecycle management
│   │   ├── mod.rs
│   │   ├── config.rs       // Configuration loading & validation
│   │   ├── lifecycle.rs    // Startup/shutdown sequences
│   │   └── runner.rs       // Main agent orchestration
│   ├── protocol/           // Protocol message types
│   │   ├── mod.rs
│   │   ├── messages.rs     // Task envelopes, status, errors
│   │   ├── topics.rs       // Topic canonicalization
│   │   └── validation.rs   // Message validation logic
│   ├── transport/          // MQTT transport layer
│   │   ├── mod.rs
│   │   ├── client.rs       // MQTT client wrapper
│   │   └── qos.rs          // QoS and message handling
│   ├── processing/         // Task processing engine
│   │   ├── mod.rs
│   │   ├── pipeline.rs     // 9-step processing algorithm
│   │   ├── idempotency.rs  // Duplicate detection
│   │   └── depth.rs        // Pipeline depth checking
│   ├── tools/              // Tool system
│   │   ├── mod.rs
│   │   ├── registry.rs     // Tool management
│   │   ├── schema.rs       // JSON schema validation
│   │   └── builtin/        // Built-in tools
│   ├── llm/                // LLM integration
│   │   ├── mod.rs
│   │   ├── provider.rs     // Provider trait
│   │   └── adapters/       // Provider implementations
│   └── error.rs            // Comprehensive error types
├── examples/               // Complete usage examples
├── tests/                  // Integration tests
└── benches/                // Performance benchmarks
```

---

## Implementation Plan

### Phase 1: Foundation (Week 1)

**Objective:** Establish core message types and validation

#### 1.1 Protocol Message Types

**Location:** `src/protocol/messages.rs`

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Task envelope containing all task information
/// 
/// This is the primary message type for agent communication.
/// See protocol section 6.1 for full specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskEnvelope {
    /// UUID v4 task identifier for idempotency
    pub task_id: Uuid,
    /// Conversation identifier for error routing  
    pub conversation_id: String,
    /// MQTT topic (must be canonicalized)
    pub topic: String,
    /// Instruction for this agent (optional)
    pub instruction: Option<String>,
    /// Input data - SHOULD be object for structured data
    pub input: Value,
    /// Next agent in pipeline (optional)
    pub next: Option<Box<NextTask>>,
}

/// Next task in pipeline chain
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NextTask {
    /// Next agent topic or final destination
    pub topic: String,
    /// Instruction for next agent
    pub instruction: Option<String>,
    /// Input will be set to previous agent's output
    pub input: Option<Value>,
    /// Continuation of pipeline
    pub next: Option<Box<NextTask>>,
}

/// Agent status message (retained)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    pub agent_id: String,
    pub status: AgentStatusType,
    /// RFC 3339 format with Z suffix
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatusType {
    Available,
    Unavailable,
}

/// Error message format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    pub error: ErrorDetails,
    pub task_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetails {
    pub code: ErrorCode,
    /// Human-readable description (no sensitive data)
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    ToolExecutionFailed,
    LlmError,
    InvalidInput,
    PipelineDepthExceeded,
    InternalError,
}
```

#### 1.2 Topic Canonicalization

**Location:** `src/protocol/topics.rs`

```rust
/// Canonicalize MQTT topic according to protocol rules
/// 
/// Rules (section 5.2):
/// 1. Ensure single leading slash
/// 2. Remove trailing slashes  
/// 3. Collapse multiple consecutive slashes
/// 
/// # Examples
/// ```
/// use agent2389::protocol::canonicalize_topic;
/// 
/// assert_eq!(canonicalize_topic("//control//agents/foo/"), "/control/agents/foo");
/// assert_eq!(canonicalize_topic("control/agents/bar"), "/control/agents/bar");
/// ```
pub fn canonicalize_topic(topic: &str) -> String {
    if topic.is_empty() {
        return "/".to_string();
    }
    
    // Start with single leading slash
    let mut result = String::from("/");
    
    // Split by slashes and filter empty segments
    let segments: Vec<&str> = topic
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    
    result.push_str(&segments.join("/"));
    result
}

/// Validate agent ID format [a-zA-Z0-9._-]+
pub fn validate_agent_id(agent_id: &str) -> Result<(), ValidationError> {
    if agent_id.is_empty() {
        return Err(ValidationError::EmptyAgentId);
    }
    
    for ch in agent_id.chars() {
        if !ch.is_alphanumeric() && !matches!(ch, '.' | '_' | '-') {
            return Err(ValidationError::InvalidAgentIdChar(ch));
        }
    }
    
    Ok(())
}
```

#### 1.3 Configuration System

**Location:** `src/agent/config.rs`

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Complete agent configuration from agent.toml
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    pub agent: AgentInfo,
    pub mqtt: MqttConfig,
    pub llm: LlmConfig,
    pub tools: HashMap<String, ToolConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentInfo {
    /// Agent ID matching [a-zA-Z0-9._-]+
    pub id: String,
    /// Description of agent purpose
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MqttConfig {
    pub broker_url: String,
    pub username_env: String,
    pub password_env: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    pub api_key_env: String,
    pub system_prompt: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ToolConfig {
    Simple(String),
    Complex {
        r#impl: String,
        config: HashMap<String, toml::Value>,
    },
}

impl AgentConfig {
    /// Load configuration from agent.toml file
    /// 
    /// # Examples
    /// ```no_run
    /// use agent2389::AgentConfig;
    /// 
    /// let config = AgentConfig::from_file("agent.toml")?;
    /// println!("Agent ID: {}", config.agent.id);
    /// ```
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let mut config: AgentConfig = toml::from_str(&content)?;
        
        // Validate agent ID format
        validate_agent_id(&config.agent.id)?;
        
        // Resolve environment variables
        config.resolve_env_vars()?;
        
        Ok(config)
    }
    
    fn resolve_env_vars(&mut self) -> Result<(), ConfigError> {
        // Implementation details for env var resolution
        Ok(())
    }
}

fn default_temperature() -> f32 { 0.7 }
fn default_max_tokens() -> u32 { 4000 }
```

### Phase 2: MQTT Transport (Week 2)

**Objective:** Implement MQTT client with proper QoS and LWT handling

#### 2.1 MQTT Client Wrapper

**Location:** `src/transport/client.rs`

```rust
use rumqttc::{AsyncClient, EventLoop, MqttOptions, QoS, LastWill};
use tokio::sync::mpsc;

/// MQTT client wrapper with protocol-specific configuration
pub struct MqttClient {
    client: AsyncClient,
    eventloop: EventLoop,
    config: MqttConfig,
}

impl MqttClient {
    /// Create new MQTT client with Last Will Testament
    pub async fn new(config: MqttConfig, agent_id: &str) -> Result<Self, MqttError> {
        let mut mqttopts = MqttOptions::new(&agent_id, &config.broker_url, 1883);
        
        // Set credentials from environment
        let username = std::env::var(&config.username_env)?;
        let password = std::env::var(&config.password_env)?;
        mqttopts.set_credentials(username, password);
        
        // Configure Last Will Testament for unavailability
        let lwt_topic = format!("/control/agents/{}/status", agent_id);
        let lwt_payload = serde_json::to_vec(&AgentStatus {
            agent_id: agent_id.to_string(),
            status: AgentStatusType::Unavailable,
            timestamp: Utc::now(),
        })?;
        
        let lwt = LastWill::new(lwt_topic, lwt_payload, QoS::AtLeastOnce, true);
        mqttopts.set_last_will(lwt);
        
        // Create client and event loop
        let (client, eventloop) = AsyncClient::new(mqttopts, 10);
        
        Ok(Self {
            client,
            eventloop,
            config,
        })
    }
    
    /// Subscribe to agent input topic with QoS 1
    pub async fn subscribe_input(&self, agent_id: &str) -> Result<(), MqttError> {
        let topic = format!("/control/agents/{}/input", agent_id);
        self.client.subscribe(topic, QoS::AtLeastOnce).await?;
        Ok(())
    }
    
    /// Publish agent status (retained)
    pub async fn publish_status(
        &self, 
        agent_id: &str, 
        status: AgentStatusType
    ) -> Result<(), MqttError> {
        let topic = format!("/control/agents/{}/status", agent_id);
        let payload = serde_json::to_vec(&AgentStatus {
            agent_id: agent_id.to_string(),
            status,
            timestamp: Utc::now(),
        })?;
        
        self.client.publish(topic, QoS::AtLeastOnce, true, payload).await?;
        Ok(())
    }
    
    /// Publish task envelope to next agent
    pub async fn publish_task(
        &self,
        topic: &str,
        task: &NextTask
    ) -> Result<(), MqttError> {
        let canonical_topic = canonicalize_topic(topic);
        let payload = serde_json::to_vec(task)?;
        
        // Tasks are NOT retained, QoS 1
        self.client.publish(canonical_topic, QoS::AtLeastOnce, false, payload).await?;
        Ok(())
    }
    
    /// Publish error to conversation topic
    pub async fn publish_error(
        &self,
        conversation_id: &str,
        agent_id: &str,
        error: ErrorMessage
    ) -> Result<(), MqttError> {
        let topic = format!("/conversations/{}/{}", conversation_id, agent_id);
        let payload = serde_json::to_vec(&error)?;
        
        self.client.publish(topic, QoS::AtLeastOnce, false, payload).await?;
        Ok(())
    }
}
```

### Phase 3: Tool System (Week 3)

**Objective:** Implement comprehensive tool interface and registry

#### 3.1 Tool Trait Definition

**Location:** `src/tools/mod.rs`

```rust
use async_trait::async_trait;
use serde_json::Value;
use jsonschema::{JSONSchema, ValidationError as SchemaValidationError};

/// Tool interface that all tools must implement
/// 
/// Every tool must implement four methods as per protocol section 8.
/// Tools operate independently and should handle errors gracefully.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Return tool description with JSON schema
    /// 
    /// Must conform to JSON Schema Draft 2020-12 subset.
    /// Used for LLM function calling and parameter validation.
    async fn describe(&self) -> ToolDescription;
    
    /// Initialize tool with configuration
    /// 
    /// Called once at agent startup. Must return success/failure status.
    /// Configuration comes from agent.toml [tools] section.
    async fn initialize(&mut self, config: &Value) -> Result<(), ToolError>;
    
    /// Execute tool with validated parameters
    /// 
    /// Parameters are pre-validated against schema from describe().
    /// Must return JSON-serializable result and handle errors gracefully.
    async fn execute(&self, parameters: &Value) -> Result<Value, ToolError>;
    
    /// Cleanup resources (optional)
    /// 
    /// Called during agent shutdown. Default implementation does nothing.
    async fn shutdown(&mut self) -> Result<(), ToolError> {
        Ok(())
    }
}

/// Tool description for LLM integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescription {
    pub name: String,
    pub description: String,
    pub parameters: Value,  // JSON Schema
}

/// Tool registry managing available tools
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
    schemas: HashMap<String, JSONSchema>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            schemas: HashMap::new(),
        }
    }
    
    /// Register a tool in the registry
    pub async fn register_tool<T: Tool + 'static>(
        &mut self,
        name: String,
        mut tool: T,
        config: &Value
    ) -> Result<(), ToolError> {
        // Initialize the tool
        tool.initialize(config).await?;
        
        // Get schema for validation
        let description = tool.describe().await;
        let schema = JSONSchema::compile(&description.parameters)
            .map_err(|e| ToolError::InvalidSchema(e.to_string()))?;
        
        self.schemas.insert(name.clone(), schema);
        self.tools.insert(name, Box::new(tool));
        
        Ok(())
    }
    
    /// Validate tool parameters against schema
    pub fn validate_parameters(
        &self, 
        tool_name: &str, 
        parameters: &Value
    ) -> Result<(), ValidationError> {
        let schema = self.schemas.get(tool_name)
            .ok_or_else(|| ValidationError::UnknownTool(tool_name.to_string()))?;
        
        let result = schema.validate(parameters);
        if let Err(errors) = result {
            return Err(ValidationError::SchemaValidation(
                errors.collect::<Vec<_>>()
            ));
        }
        
        Ok(())
    }
    
    /// Execute tool with parameter validation
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        parameters: &Value
    ) -> Result<Value, ToolError> {
        // Validate parameters first
        self.validate_parameters(tool_name, parameters)?;
        
        // Execute tool
        let tool = self.tools.get(tool_name)
            .ok_or_else(|| ToolError::UnknownTool(tool_name.to_string()))?;
            
        tool.execute(parameters).await
    }
    
    /// Get all tool descriptions for LLM
    pub async fn get_tool_descriptions(&self) -> Vec<ToolDescription> {
        let mut descriptions = Vec::new();
        
        for (name, tool) in &self.tools {
            let mut desc = tool.describe().await;
            desc.name = name.clone();
            descriptions.push(desc);
        }
        
        descriptions
    }
}
```

#### 3.2 Built-in Tools

**Location:** `src/tools/builtin/`

```rust
// src/tools/builtin/http_request.rs

/// HTTP request tool for web API calls
pub struct HttpRequestTool {
    client: reqwest::Client,
    max_response_size: usize,
}

#[async_trait]
impl Tool for HttpRequestTool {
    async fn describe(&self) -> ToolDescription {
        ToolDescription {
            name: "http_request".to_string(),
            description: "Make HTTP requests to external APIs".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to request"
                    },
                    "method": {
                        "type": "string",
                        "enum": ["GET", "POST", "PUT", "DELETE"],
                        "default": "GET",
                        "description": "HTTP method"
                    },
                    "headers": {
                        "type": "object",
                        "description": "HTTP headers as key-value pairs"
                    },
                    "body": {
                        "type": "string",
                        "description": "Request body for POST/PUT requests"
                    }
                },
                "required": ["url"]
            }),
        }
    }
    
    async fn initialize(&mut self, config: &Value) -> Result<(), ToolError> {
        self.max_response_size = config.get("max_response_size")
            .and_then(|v| v.as_u64())
            .unwrap_or(1024 * 1024) as usize; // 1MB default
            
        self.client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
            
        Ok(())
    }
    
    async fn execute(&self, parameters: &Value) -> Result<Value, ToolError> {
        let url = parameters["url"].as_str()
            .ok_or(ToolError::InvalidParameter("url must be string"))?;
            
        let method = parameters.get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("GET");
        
        let mut request = match method {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "DELETE" => self.client.delete(url),
            _ => return Err(ToolError::InvalidParameter("Invalid HTTP method")),
        };
        
        // Add headers if provided
        if let Some(headers) = parameters.get("headers") {
            if let Some(headers_obj) = headers.as_object() {
                for (key, value) in headers_obj {
                    if let Some(value_str) = value.as_str() {
                        request = request.header(key, value_str);
                    }
                }
            }
        }
        
        // Add body for POST/PUT
        if matches!(method, "POST" | "PUT") {
            if let Some(body) = parameters.get("body") {
                if let Some(body_str) = body.as_str() {
                    request = request.body(body_str.to_string());
                }
            }
        }
        
        let response = request.send().await?;
        let status = response.status().as_u16();
        
        // Check response size
        let content_length = response.content_length().unwrap_or(0);
        if content_length > self.max_response_size as u64 {
            return Err(ToolError::ResponseTooLarge(content_length));
        }
        
        let body = response.text().await?;
        
        Ok(json!({
            "status": status,
            "body": body,
            "headers": {} // Could extract response headers if needed
        }))
    }
}
```

### Phase 4: Task Processing Engine (Week 4)

**Objective:** Implement the complete 9-step processing algorithm

#### 4.1 Processing Pipeline

**Location:** `src/processing/pipeline.rs`

```rust
use crate::protocol::{TaskEnvelope, ErrorMessage, ErrorCode};
use crate::tools::ToolRegistry;
use crate::llm::LlmProvider;

/// Task processor implementing 9-step algorithm
/// 
/// This is the core of the agent, implementing the exact algorithm
/// specified in protocol section 7.4.
pub struct TaskProcessor {
    agent_id: String,
    mqtt_client: Arc<MqttClient>,
    tool_registry: Arc<ToolRegistry>,
    llm_provider: Arc<dyn LlmProvider>,
    idempotency_cache: IdempotencyCache,
    max_pipeline_depth: usize,
}

impl TaskProcessor {
    pub fn new(
        agent_id: String,
        mqtt_client: Arc<MqttClient>,
        tool_registry: Arc<ToolRegistry>,
        llm_provider: Arc<dyn LlmProvider>,
    ) -> Self {
        Self {
            agent_id,
            mqtt_client,
            tool_registry,
            llm_provider,
            idempotency_cache: IdempotencyCache::new(1000), // Keep 1000 recent task IDs
            max_pipeline_depth: 16, // Default from protocol
        }
    }
    
    /// Process task envelope using 9-step algorithm
    /// 
    /// This method MUST execute all 9 steps in sequence as per FR-014.
    /// Steps are documented inline to match protocol specification.
    pub async fn process_task(&mut self, 
        envelope: TaskEnvelope,
        received_topic: &str,
        is_retained: bool
    ) -> Result<(), ProcessingError> {
        
        // Step 1: Receive message on /control/agents/{agent_id}/input
        // (Already completed by caller)
        
        // Step 2: Ignore if message is retained [req: FR-014]
        if is_retained {
            tracing::debug!(
                task_id = %envelope.task_id,
                "Ignoring retained message as per protocol requirement"
            );
            return Ok(());
        }
        
        // Step 3: Canonicalize and validate topic match [req: FR-014]
        let canonical_received = canonicalize_topic(received_topic);
        let canonical_envelope = canonicalize_topic(&envelope.topic);
        
        if canonical_received != canonical_envelope {
            tracing::error!(
                task_id = %envelope.task_id,
                received_topic = %canonical_received,
                envelope_topic = %canonical_envelope,
                "Topic mismatch - discarding message"
            );
            return Ok(()); // Discard without error
        }
        
        // Step 4: Check for duplicate task_id (idempotency) [req: FR-014]
        if self.idempotency_cache.contains(&envelope.task_id) {
            tracing::debug!(
                task_id = %envelope.task_id,
                "Discarding duplicate task"
            );
            return Ok(());
        }
        
        // Step 5: Check pipeline depth [req: FR-013, FR-014]
        let pipeline_depth = self.calculate_pipeline_depth(&envelope);
        if pipeline_depth > self.max_pipeline_depth {
            let error = ErrorMessage {
                error: ErrorDetails {
                    code: ErrorCode::PipelineDepthExceeded,
                    message: format!(
                        "Pipeline depth {} exceeds maximum {}", 
                        pipeline_depth, 
                        self.max_pipeline_depth
                    ),
                },
                task_id: envelope.task_id,
            };
            
            self.mqtt_client.publish_error(
                &envelope.conversation_id,
                &self.agent_id,
                error
            ).await?;
            
            return Ok(());
        }
        
        // Step 6: Parse task envelope [req: FR-014]
        // (Already parsed by serde during message reception)
        
        // Step 7: Process [req: FR-014]
        let llm_response = match self.process_with_llm(&envelope).await {
            Ok(response) => response,
            Err(e) => {
                let error = ErrorMessage {
                    error: ErrorDetails {
                        code: ErrorCode::LlmError,
                        message: format!("LLM processing failed: {}", e),
                    },
                    task_id: envelope.task_id,
                };
                
                self.mqtt_client.publish_error(
                    &envelope.conversation_id,
                    &self.agent_id,
                    error
                ).await?;
                
                return Ok(());
            }
        };
        
        // Step 8: If next is not null [req: FR-014]
        if let Some(mut next) = envelope.next {
            next.input = Some(llm_response);
            self.mqtt_client.publish_task(&next.topic, &next).await?;
        }
        
        // Step 9: Complete [req: FR-014]
        self.idempotency_cache.insert(envelope.task_id);
        
        tracing::info!(
            task_id = %envelope.task_id,
            pipeline_depth = pipeline_depth,
            "Task processing completed successfully"
        );
        
        Ok(())
    }
    
    /// Calculate pipeline depth by counting nested next objects
    fn calculate_pipeline_depth(&self, envelope: &TaskEnvelope) -> usize {
        let mut depth = 1;
        let mut current = &envelope.next;
        
        while let Some(next) = current {
            depth += 1;
            current = &next.next;
            
            // Safety check to prevent infinite loops
            if depth > 1000 {
                break;
            }
        }
        
        depth
    }
    
    /// Process task with LLM and tools
    async fn process_with_llm(&self, envelope: &TaskEnvelope) -> Result<Value, LlmError> {
        // Get available tools
        let tool_descriptions = self.tool_registry.get_tool_descriptions().await;
        
        // Prepare LLM request
        let llm_request = LlmRequest {
            instruction: envelope.instruction.clone(),
            input: envelope.input.clone(),
            tools: tool_descriptions,
        };
        
        // Call LLM with tool support
        let mut llm_response = self.llm_provider.process(llm_request).await?;
        
        // Execute any tool calls
        while let Some(tool_calls) = &llm_response.tool_calls {
            let mut tool_results = Vec::new();
            
            for tool_call in tool_calls {
                // Validate tool call against allow-list [req: NFR-005]
                // (All registered tools are in allow-list)
                
                match self.tool_registry.execute_tool(
                    &tool_call.name,
                    &tool_call.parameters
                ).await {
                    Ok(result) => {
                        tool_results.push(ToolResult {
                            call_id: tool_call.id.clone(),
                            success: true,
                            result: Some(result),
                            error: None,
                        });
                    }
                    Err(e) => {
                        tool_results.push(ToolResult {
                            call_id: tool_call.id.clone(),
                            success: false,
                            result: None,
                            error: Some(e.to_string()),
                        });
                    }
                }
            }
            
            // Send tool results back to LLM
            let tool_request = LlmToolResultRequest {
                tool_results,
                previous_response: llm_response,
            };
            
            llm_response = self.llm_provider.process_tool_results(tool_request).await?;
        }
        
        Ok(llm_response.content)
    }
}
```

#### 4.2 Idempotency Cache

**Location:** `src/processing/idempotency.rs`

```rust
use uuid::Uuid;
use std::collections::VecDeque;
use std::sync::Mutex;

/// LRU cache for task ID deduplication
/// 
/// Maintains recently processed task IDs to prevent duplicate processing.
/// Thread-safe and bounded to prevent memory leaks.
pub struct IdempotencyCache {
    cache: Mutex<VecDeque<Uuid>>,
    max_size: usize,
}

impl IdempotencyCache {
    /// Create new cache with maximum size
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: Mutex::new(VecDeque::with_capacity(max_size)),
            max_size,
        }
    }
    
    /// Check if task ID exists in cache
    pub fn contains(&self, task_id: &Uuid) -> bool {
        let cache = self.cache.lock().unwrap();
        cache.contains(task_id)
    }
    
    /// Insert task ID into cache
    pub fn insert(&self, task_id: Uuid) {
        let mut cache = self.cache.lock().unwrap();
        
        // Remove if already exists (move to front)
        if let Some(pos) = cache.iter().position(|&id| id == task_id) {
            cache.remove(pos);
        }
        
        // Add to front
        cache.push_front(task_id);
        
        // Maintain max size
        while cache.len() > self.max_size {
            cache.pop_back();
        }
    }
}
```

### Phase 5: LLM Integration (Week 5)

**Objective:** Implement LLM provider trait and adapters

#### 5.1 LLM Provider Trait

**Location:** `src/llm/provider.rs`

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Process LLM request with tools
    async fn process(&self, request: LlmRequest) -> Result<LlmResponse, LlmError>;
    
    /// Process tool results and get final response
    async fn process_tool_results(
        &self, 
        request: LlmToolResultRequest
    ) -> Result<LlmResponse, LlmError>;
    
    /// Test connectivity and model access
    async fn verify_connectivity(&self) -> Result<(), LlmError>;
}

#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub instruction: Option<String>,
    pub input: Value,
    pub tools: Vec<ToolDescription>,
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: Value,
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub parameters: Value,
}
```

### Phase 6: Agent Lifecycle (Week 6)

**Objective:** Implement complete startup and shutdown sequences

#### 6.1 Agent Runner

**Location:** `src/agent/runner.rs`

```rust
/// Main agent orchestrator implementing full lifecycle
/// 
/// This struct manages the complete agent lifecycle as specified
/// in protocol sections 7.1 (startup) and 7.2 (shutdown).
pub struct AgentRunner {
    config: AgentConfig,
    mqtt_client: Option<MqttClient>,
    task_processor: Option<TaskProcessor>,
    tool_registry: Arc<ToolRegistry>,
    llm_provider: Arc<dyn LlmProvider>,
    shutdown_signal: Option<tokio::sync::oneshot::Receiver<()>>,
}

impl AgentRunner {
    pub fn new(config: AgentConfig) -> Self {
        Self {
            config,
            mqtt_client: None,
            task_processor: None,
            tool_registry: Arc::new(ToolRegistry::new()),
            llm_provider: Arc::new(create_llm_provider(&config.llm)?),
            shutdown_signal: None,
        }
    }
    
    /// Execute complete startup sequence [req: FR-002]
    /// 
    /// All steps must complete successfully before publishing availability.
    /// This implements the exact sequence from protocol section 7.1.
    pub async fn startup(&mut self) -> Result<(), StartupError> {
        tracing::info!(
            agent_id = %self.config.agent.id,
            "Starting agent startup sequence"
        );
        
        // Step 1: Load and parse configuration (already done in constructor)
        
        // Step 2: Establish MQTT connection with credentials
        let mqtt_client = MqttClient::new(
            self.config.mqtt.clone(),
            &self.config.agent.id
        ).await?;
        
        // Step 3: Configure Last Will Testament (done in MqttClient::new)
        
        // Step 4: Subscribe to input topic and wait for SUBACK
        mqtt_client.subscribe_input(&self.config.agent.id).await?;
        
        // Step 5: Initialize all configured tools successfully
        let mut tool_registry = Arc::try_unwrap(self.tool_registry).unwrap_or_else(|arc| {
            // If we can't unwrap, clone the data (shouldn't happen in startup)
            panic!("Tool registry has multiple references during startup");
        });
        
        for (name, tool_config) in &self.config.tools {
            let tool = create_tool(name, tool_config)?;
            let config_value = tool_config_to_value(tool_config);
            tool_registry.register_tool(name.clone(), tool, &config_value).await?;
        }
        
        self.tool_registry = Arc::new(tool_registry);
        
        // Step 6: Verify LLM adapter connectivity
        self.llm_provider.verify_connectivity().await?;
        
        // Step 7: Publish availability ONLY after all previous steps succeed
        mqtt_client.publish_status(&self.config.agent.id, AgentStatusType::Available).await?;
        
        // Step 8: Enter idle state awaiting tasks
        self.mqtt_client = Some(mqtt_client);
        
        let mqtt_arc = Arc::new(self.mqtt_client.as_ref().unwrap());
        self.task_processor = Some(TaskProcessor::new(
            self.config.agent.id.clone(),
            mqtt_arc.clone(),
            self.tool_registry.clone(),
            self.llm_provider.clone(),
        ));
        
        tracing::info!(
            agent_id = %self.config.agent.id,
            "Agent startup completed successfully"
        );
        
        Ok(())
    }
    
    /// Main event loop processing MQTT messages
    pub async fn run(&mut self) -> Result<(), RuntimeError> {
        let mqtt_client = self.mqtt_client.as_ref()
            .ok_or(RuntimeError::NotStarted)?;
        
        let mut task_processor = self.task_processor.take()
            .ok_or(RuntimeError::NotStarted)?;
        
        let mut eventloop = mqtt_client.eventloop.clone();
        
        loop {
            tokio::select! {
                // Handle MQTT events
                mqtt_event = eventloop.poll() => {
                    match mqtt_event? {
                        Event::Incoming(Packet::Publish(publish)) => {
                            if let Err(e) = self.handle_publish(&mut task_processor, publish).await {
                                tracing::error!(
                                    error = %e,
                                    "Error processing MQTT publish"
                                );
                            }
                        }
                        _ => {} // Ignore other packet types
                    }
                }
                
                // Handle shutdown signal
                _ = self.shutdown_signal.take().unwrap_or_else(|| {
                    let (_tx, rx) = tokio::sync::oneshot::channel();
                    rx
                }) => {
                    tracing::info!("Received shutdown signal");
                    break;
                }
            }
        }
        
        self.shutdown().await?;
        Ok(())
    }
    
    /// Handle incoming MQTT publish message
    async fn handle_publish(
        &self,
        task_processor: &mut TaskProcessor,
        publish: Publish
    ) -> Result<(), ProcessingError> {
        // Parse task envelope
        let envelope: TaskEnvelope = serde_json::from_slice(&publish.payload)?;
        
        // Process with full 9-step algorithm
        task_processor.process_task(
            envelope,
            &publish.topic,
            publish.retain
        ).await?;
        
        Ok(())
    }
    
    /// Execute shutdown sequence [req: FR-003]
    pub async fn shutdown(&mut self) -> Result<(), ShutdownError> {
        tracing::info!(
            agent_id = %self.config.agent.id,
            "Starting agent shutdown sequence"
        );
        
        if let Some(mqtt_client) = &self.mqtt_client {
            // Step 1: Publish unavailability status (retained)
            mqtt_client.publish_status(
                &self.config.agent.id, 
                AgentStatusType::Unavailable
            ).await?;
            
            // Step 2: Disconnect from MQTT broker
            // (Handled automatically when mqtt_client is dropped)
        }
        
        // Shutdown all tools
        let tool_registry = Arc::try_unwrap(self.tool_registry.clone())
            .unwrap_or_else(|arc| {
                // Create new registry if we can't unwrap (multiple references)
                ToolRegistry::new()
            });
        
        // Note: In practice, we would call shutdown on all tools here
        // but Arc prevents us from getting mutable access
        
        tracing::info!(
            agent_id = %self.config.agent.id,
            "Agent shutdown completed"
        );
        
        Ok(())
    }
}

/// Create tool instance from configuration
fn create_tool(name: &str, config: &ToolConfig) -> Result<Box<dyn Tool>, ToolError> {
    match name {
        "http_request" => Ok(Box::new(HttpRequestTool::default())),
        "file_read" => Ok(Box::new(FileReadTool::default())),
        "file_write" => Ok(Box::new(FileWriteTool::default())),
        _ => {
            // In practice, this would use a registry pattern
            // or dynamic loading based on the impl field
            Err(ToolError::UnknownTool(name.to_string()))
        }
    }
}
```

---

## Error Handling Strategy

### Comprehensive Error Types

**Location:** `src/error.rs`

```rust
/// Top-level error type mapping to protocol error codes
#[derive(Debug, thiserror::Error)]
pub enum Agent2389Error {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    
    #[error("MQTT transport error: {0}")]
    Transport(#[from] MqttError),
    
    #[error("Tool execution failed: {0}")]
    Tool(#[from] ToolError),
    
    #[error("LLM processing error: {0}")]
    Llm(#[from] LlmError),
    
    #[error("Message validation error: {0}")]
    Validation(#[from] ValidationError),
    
    #[error("Pipeline depth exceeded")]
    PipelineDepthExceeded,
    
    #[error("Internal error: {0}")]
    Internal(String),
}

impl Agent2389Error {
    /// Convert to protocol error code
    pub fn to_error_code(&self) -> ErrorCode {
        match self {
            Agent2389Error::Tool(_) => ErrorCode::ToolExecutionFailed,
            Agent2389Error::Llm(_) => ErrorCode::LlmError,
            Agent2389Error::Validation(_) => ErrorCode::InvalidInput,
            Agent2389Error::PipelineDepthExceeded => ErrorCode::PipelineDepthExceeded,
            _ => ErrorCode::InternalError,
        }
    }
    
    /// Get human-readable message (no sensitive data)
    pub fn to_safe_message(&self) -> String {
        match self {
            Agent2389Error::Tool(e) => format!("Tool execution failed: {}", e.safe_message()),
            Agent2389Error::Llm(_) => "LLM processing failed".to_string(),
            Agent2389Error::Validation(e) => format!("Invalid input: {}", e),
            Agent2389Error::PipelineDepthExceeded => "Pipeline depth exceeded maximum".to_string(),
            _ => "Internal error occurred".to_string(),
        }
    }
}
```

---

## Test-Driven Development Strategy

### Why Rust Excels at TDD

Rust is uniquely suited for TDD because:

- **Compile-time guarantees**: Many bugs that require tests in Python are caught by the compiler
- **Built-in testing**: `cargo test` is first-class, no external test runner needed
- **Type-driven development**: Write function signatures first, let compiler guide implementation
- **Property-based testing**: Excellent support with `proptest` crate
- **Integration testing**: Built-in support for integration tests with real external systems

### Rust Quality Toolchain (Python equivalents)

```toml
# Cargo.toml - Development dependencies
[dev-dependencies]
proptest = "1.0"           # Property-based testing
tokio-test = "0.4"         # Testing utilities for async code
criterion = "0.5"          # Benchmarking (like pytest-benchmark)
wiremock = "0.5"          # HTTP mocking for tool tests

# Additional tooling (installed globally)
# cargo install cargo-watch cargo-nextest cargo-tarpaulin cargo-audit
# cargo install cargo-deny cargo-machete cargo-outdated
```

#### Quality Tools Comparison

| Python Tool | Rust Equivalent | Purpose |
|-------------|-----------------|---------|
| `mypy` | Built-in compiler | Type checking (much stronger) |
| `ruff`/`flake8` | `cargo clippy` | Linting and best practices |
| `black` | `cargo fmt` | Code formatting |
| `pytest` | `cargo test` | Testing framework |
| `coverage.py` | `cargo tarpaulin` | Code coverage |
| `safety` | `cargo audit` | Security vulnerability scanning |
| `pip-audit` | `cargo deny` | Dependency license/security checking |
| `isort` | `rustfmt` (imports) | Import organization |

### TDD Workflow for Autonomous Agents

#### Pre-Implementation Quality Gates

**Location:** `scripts/quality-check.sh`

```bash
#!/bin/bash
set -e

echo "🔍 Running Rust quality checks..."

# 1. Format check (like black --check)
echo "📝 Checking code formatting..."
cargo fmt --check || (echo "❌ Code not formatted. Run: cargo fmt" && exit 1)

# 2. Lint check (like ruff)
echo "🔧 Running clippy lints..."
cargo clippy --all-targets --all-features -- -D warnings || (echo "❌ Clippy warnings found" && exit 1)

# 3. Type check (faster than full compile)
echo "🏗️  Type checking..."
cargo check --all-targets --all-features || (echo "❌ Type errors found" && exit 1)

# 4. Test compilation (don't run yet)
echo "🧪 Checking test compilation..."
cargo test --no-run || (echo "❌ Tests don't compile" && exit 1)

# 5. Documentation check
echo "📚 Checking documentation..."
cargo doc --no-deps --document-private-items || (echo "❌ Documentation errors" && exit 1)

# 6. Dependency audit
echo "🔒 Auditing dependencies..."
cargo audit || echo "⚠️  Audit warnings (review manually)"

# 7. Unused dependency check
echo "🧹 Checking for unused dependencies..."
cargo machete || echo "ℹ️  Unused dependencies found (review manually)"

echo "✅ All quality checks passed!"
```

#### Continuous TDD Loop

**Location:** `scripts/tdd-loop.sh`

```bash
#!/bin/bash
# Continuous TDD loop using cargo-watch
cargo watch -x "fmt" -x "clippy --fix --allow-dirty" -x "test" -s "echo '✅ TDD cycle complete'"
```

### Protocol Compliance Testing

#### 1. Unit Tests with Property-Based Testing

**Location:** `src/protocol/messages.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    
    // Property: All canonicalized topics should be idempotent
    proptest! {
        #[test]
        fn canonicalize_topic_is_idempotent(topic in ".*") {
            let first = canonicalize_topic(&topic);
            let second = canonicalize_topic(&first);
            prop_assert_eq!(first, second);
        }
        
        #[test] 
        fn canonicalize_topic_starts_with_slash(topic in ".*") {
            let result = canonicalize_topic(&topic);
            prop_assert!(result.starts_with('/'), "Topic should start with /: {}", result);
        }
        
        #[test]
        fn canonicalize_topic_no_consecutive_slashes(topic in ".*") {
            let result = canonicalize_topic(&topic);
            prop_assert!(!result.contains("//"), "No consecutive slashes: {}", result);
        }
    }
    
    #[test]
    fn test_protocol_examples() {
        // Test exact examples from protocol specification
        assert_eq!(canonicalize_topic("//control//agents/foo/"), "/control/agents/foo");
        assert_eq!(canonicalize_topic("control/agents/bar"), "/control/agents/bar");
        assert_eq!(canonicalize_topic("/control/agents/baz"), "/control/agents/baz");
    }
}
```

#### 2. TDD for 9-Step Algorithm

**Location:** `src/processing/pipeline.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    // TDD: Write the test FIRST, then implement each step
    
    #[tokio::test]
    async fn test_step_2_ignores_retained_messages() {
        let mut processor = create_mock_processor().await;
        let task = create_test_task();
        
        // Step 2: Should ignore retained messages
        let result = processor.process_task(task, "/test/topic", true).await;
        
        // Should complete without processing
        assert!(result.is_ok());
        // Verify no LLM calls were made
        assert_eq!(processor.llm_call_count(), 0);
    }
    
    #[tokio::test] 
    async fn test_step_3_topic_validation() {
        let mut processor = create_mock_processor().await;
        let mut task = create_test_task();
        task.topic = "//different//topic//".to_string();
        
        // Step 3: Should canonicalize and validate topic match
        let result = processor.process_task(task, "//test//topic//", false).await;
        
        // Should discard message without error
        assert!(result.is_ok());
        assert_eq!(processor.llm_call_count(), 0);
    }
    
    #[tokio::test]
    async fn test_step_4_idempotency() {
        let mut processor = create_mock_processor().await;
        let task = create_test_task();
        let task_id = task.task_id;
        
        // Process once
        processor.process_task(task.clone(), "/test/topic", false).await.unwrap();
        
        // Process same task again - should be ignored
        let result = processor.process_task(task, "/test/topic", false).await;
        assert!(result.is_ok());
        assert_eq!(processor.llm_call_count(), 1); // Should only be called once
    }
    
    #[tokio::test]
    async fn test_step_5_pipeline_depth_enforcement() {
        let mut processor = create_mock_processor().await;
        let task = create_deep_pipeline_task(20); // Exceeds limit of 16
        
        let result = processor.process_task(task, "/test/topic", false).await;
        
        // Should publish error and stop processing
        assert!(result.is_ok());
        assert_eq!(processor.published_errors().len(), 1);
        assert_eq!(
            processor.published_errors()[0].error.code, 
            ErrorCode::PipelineDepthExceeded
        );
    }
    
    // Helper functions for test setup
    async fn create_mock_processor() -> MockTaskProcessor {
        // Implementation for creating test doubles
    }
    
    fn create_test_task() -> TaskEnvelope {
        TaskEnvelope {
            task_id: Uuid::new_v4(),
            conversation_id: "test-conversation".to_string(),
            topic: "/test/topic".to_string(),
            instruction: Some("Test instruction".to_string()),
            input: json!({"test": "data"}),
            next: None,
        }
    }
}
```

#### 3. Protocol Requirements Testing

**Location:** `tests/protocol_compliance.rs`

```rust
//! Protocol compliance tests
//! 
//! Each test corresponds to a specific requirement [req: X] from the specification.
//! These tests ensure we build exactly what's specified without scope creep.

use agent2389::*;

/// Test [req: FR-001] - Interoperability requirement
#[tokio::test]
async fn test_fr_001_interoperability() {
    // Any conforming implementation must interoperate
    // Test by creating messages that should work with any other implementation
    
    let task = TaskEnvelope {
        task_id: Uuid::new_v4(),
        conversation_id: "test".to_string(),
        topic: "/control/agents/test/input".to_string(),
        instruction: Some("test".to_string()),
        input: json!({"key": "value"}),
        next: None,
    };
    
    // Should serialize to spec-compliant JSON
    let json = serde_json::to_string(&task).unwrap();
    let parsed: TaskEnvelope = serde_json::from_str(&json).unwrap();
    assert_eq!(task, parsed);
}

/// Test [req: FR-002] - Complete startup sequence
#[tokio::test]
async fn test_fr_002_startup_sequence() {
    // Must complete ALL 8 startup steps before publishing availability
    let config = create_test_config();
    let mut agent = AgentRunner::new(config);
    
    // Mock the external dependencies to verify order
    let mut startup_monitor = StartupMonitor::new();
    agent.set_startup_monitor(startup_monitor);
    
    agent.startup().await.unwrap();
    
    // Verify all steps completed in correct order
    let events = startup_monitor.events();
    assert_eq!(events[0], StartupEvent::ConfigLoaded);
    assert_eq!(events[1], StartupEvent::MqttConnected);
    assert_eq!(events[2], StartupEvent::LwtConfigured);
    assert_eq!(events[3], StartupEvent::InputTopicSubscribed);
    assert_eq!(events[4], StartupEvent::ToolsInitialized);
    assert_eq!(events[5], StartupEvent::LlmVerified);
    assert_eq!(events[6], StartupEvent::AvailabilityPublished);
    assert_eq!(events[7], StartupEvent::IdleStateEntered);
}

/// Test [req: FR-014] - Complete 9-step algorithm
#[tokio::test]
async fn test_fr_014_nine_step_algorithm() {
    // Must execute ALL 9 steps in sequence
    let mut processor = create_instrumented_processor().await;
    let task = create_test_task();
    
    processor.process_task(task, "/test/topic", false).await.unwrap();
    
    let steps = processor.executed_steps();
    assert_eq!(steps.len(), 9);
    assert_eq!(steps[0], ProcessingStep::MessageReceived);
    assert_eq!(steps[1], ProcessingStep::RetainedCheck);
    assert_eq!(steps[2], ProcessingStep::TopicValidation);
    assert_eq!(steps[3], ProcessingStep::IdempotencyCheck);
    assert_eq!(steps[4], ProcessingStep::PipelineDepthCheck);
    assert_eq!(steps[5], ProcessingStep::EnvelopeParsing);
    assert_eq!(steps[6], ProcessingStep::LlmProcessing);
    assert_eq!(steps[7], ProcessingStep::NextHandling);
    assert_eq!(steps[8], ProcessingStep::Completion);
}

/// Property-based test for [req: RULE-001] - Agent ID format
proptest! {
    #[test]
    fn test_rule_001_agent_id_format(
        id in "[a-zA-Z0-9._-]{1,64}" // Valid agent ID pattern
    ) {
        prop_assert!(validate_agent_id(&id).is_ok());
    }
    
    #[test]
    fn test_rule_001_invalid_agent_id(
        id in "[^a-zA-Z0-9._-]+.*" // Contains invalid characters  
    ) {
        prop_assume!(!id.is_empty());
        prop_assert!(validate_agent_id(&id).is_err());
    }
}
```

### Integration Test Structure

**Location:** `tests/integration_test.rs`

```rust
use agent2389::*;
use serde_json::json;

/// Complete integration test using real MQTT broker
#[tokio::test]
async fn test_complete_agent_workflow() {
    // Start test MQTT broker
    let docker = clients::Cli::default();
    let mqtt_container = docker.run(images::eclipse_mosquitto::EclipseMosquitto::default());
    let mqtt_port = mqtt_container.get_host_port_ipv4(1883);
    
    // Create test configuration
    let config = AgentConfig {
        agent: AgentInfo {
            id: "test-agent".to_string(),
            description: "Test agent for integration testing".to_string(),
        },
        mqtt: MqttConfig {
            broker_url: format!("mqtt://localhost:{}", mqtt_port),
            username_env: "MQTT_USER".to_string(),
            password_env: "MQTT_PASS".to_string(),
        },
        llm: LlmConfig {
            provider: "mock".to_string(),
            model: "mock-model".to_string(),
            api_key_env: "MOCK_API_KEY".to_string(),
            system_prompt: "You are a test agent".to_string(),
            temperature: 0.0,
            max_tokens: 1000,
        },
        tools: HashMap::new(),
    };
    
    // Set environment variables
    std::env::set_var("MQTT_USER", "test");
    std::env::set_var("MQTT_PASS", "test");
    std::env::set_var("MOCK_API_KEY", "mock-key");
    
    // Start agent
    let mut agent = AgentRunner::new(config);
    agent.startup().await.expect("Agent startup should succeed");
    
    // Create test client
    let test_client = create_test_mqtt_client(mqtt_port).await;
    
    // Send test task
    let task = TaskEnvelope {
        task_id: Uuid::new_v4(),
        conversation_id: "test-conversation".to_string(),
        topic: "/control/agents/test-agent/input".to_string(),
        instruction: Some("Process this test data".to_string()),
        input: json!({"test": "data"}),
        next: None,
    };
    
    let task_json = serde_json::to_string(&task).unwrap();
    test_client.publish("/control/agents/test-agent/input", task_json).await;
    
    // Wait for processing and verify results
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    
    // Verify agent is still available
    let status_topic = "/control/agents/test-agent/status";
    let status_message = test_client.get_retained_message(status_topic).await;
    let status: AgentStatus = serde_json::from_str(&status_message).unwrap();
    assert_eq!(status.status, AgentStatusType::Available);
    
    // Shutdown agent
    agent.shutdown().await.expect("Agent shutdown should succeed");
}

/// Test pipeline depth enforcement
#[tokio::test]  
async fn test_pipeline_depth_limit() {
    // Create deeply nested pipeline
    let mut next = None;
    for i in (0..20).rev() {
        next = Some(Box::new(NextTask {
            topic: format!("/control/agents/agent-{}/input", i),
            instruction: Some(format!("Step {}", i)),
            input: None,
            next,
        }));
    }
    
    let task = TaskEnvelope {
        task_id: Uuid::new_v4(),
        conversation_id: "test-conversation".to_string(),
        topic: "/control/agents/test-agent/input".to_string(),
        instruction: Some("Start deep pipeline".to_string()),
        input: json!({"test": "data"}),
        next,
    };
    
    // Process should fail with pipeline depth exceeded
    let mut processor = create_test_processor().await;
    let result = processor.process_task(task, "/control/agents/test-agent/input", false).await;
    
    // Should not return error but should publish error message to MQTT
    assert!(result.is_ok());
    
    // Verify error was published (would need mock MQTT client to verify)
}
```

---

## Documentation Requirements

### CLAUDE.md Template

**Location:** `CLAUDE.md` (to be created in project root)

```markdown
# 2389 Agent Protocol Rust Implementation

## Project Overview
This is a Rust implementation of the 2389 Agent Protocol, a standard for creating interoperable AI agents that\ncommunicate via MQTT. The implementation prioritizes correctness, performance, and strict protocol compliance.

## Architecture
- **Async-first design**: All I/O operations use tokio async runtime
- **Strong typing**: Protocol messages use serde with validation
- **Error handling**: Comprehensive error types mapping to protocol error codes
- **Tool system**: Trait-based with JSON schema validation
- **MQTT integration**: QoS 1, proper LWT handling, topic canonicalization

## Key Files
- `src/lib.rs` - Public API surface with examples
- `src/agent/runner.rs` - Main agent orchestration and lifecycle
- `src/protocol/messages.rs` - All protocol message types
- `src/processing/pipeline.rs` - 9-step task processing algorithm
- `src/tools/mod.rs` - Tool trait and registry
- `agent.toml` - Agent configuration file

## Code Style
- Use `#[must_use]` on Result types
- Implement Display and Debug for public types
- Include rustdoc examples for public functions
- Return early pattern for error handling
- Specific error types over generic ones
- Comprehensive logging with tracing crate

## Testing Strategy
- Unit tests for all modules
- Integration tests with real MQTT broker (localhost:1883 in dev, Mosquitto container in CI)
- Property-based tests for protocol compliance
- Error injection tests for robustness

## Development Workflow
1. Run `cargo check` before committing
2. Use `cargo fmt` and `cargo clippy`
3. All tests must pass: `cargo test`
4. Update documentation: `cargo doc`
5. Benchmark critical paths: `cargo bench`

## Protocol Requirements
This implementation MUST comply with all protocol requirements tagged [req: X] in the specification. Key requirements:
- Interoperability [req: FR-001]
- Complete startup/shutdown sequences [req: FR-002, FR-003]
- 9-step task processing algorithm [req: FR-014]
- Pipeline depth enforcement [req: FR-013]
- Topic canonicalization [req: FR-006]
- Idempotency handling [req: FR-010]

## Error Handling
- Agent MUST NOT crash on invalid input
- Errors MUST be published to conversation topics
- Error messages MUST NOT contain sensitive information
- All tool calls MUST be validated against schemas

## Tools System
- All tools implement the Tool trait
- Parameters validated against JSON schemas
- Built-in tools: http_request, file_read, file_write
- Tools can be configured via agent.toml

## Think hard about edge cases and error conditions. This is a production system that must handle malformed\ninputs gracefully.
```

### README.md Template

**Location:** `README.md`

```markdown
# Agent 2389 - Rust Implementation

A production-ready Rust implementation of the 2389 Agent Protocol for interoperable AI agents.

## Features

- 🚀 **High Performance**: Async-first design with tokio
- 🔒 **Type Safety**: Strong typing with comprehensive validation
- 🔄 **Protocol Compliant**: 100% compliant with 2389 Agent Protocol specification
- 🛠️ **Extensible Tools**: Trait-based tool system with JSON schema validation
- 📡 **MQTT Integration**: Proper QoS, Last Will Testament, and topic handling
- 🧪 **Well Tested**: Comprehensive test suite with integration tests

## Quick Start

```bash
# Install
cargo install agent2389

# Create configuration
cat > agent.toml << EOF
[agent]
id = "my-agent"
description = "Example agent"

[mqtt]
broker_url = "mqtt://localhost:1883"
username_env = "MQTT_USERNAME"
password_env = "MQTT_PASSWORD"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are a helpful AI agent."

[tools]
http_request = "builtin"
file_read = "builtin"
EOF

# Run agent
agent2389 run agent.toml
```

## Library Usage

```rust
use agent2389::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = AgentConfig::from_file("agent.toml")?;
    
    // Create and start agent
    let mut agent = AgentRunner::new(config);
    agent.startup().await?;
    
    // Run until shutdown signal
    agent.run().await?;
    
    Ok(())
}
```

## Documentation

- [Protocol Specification](https://example.com/2389-protocol)
- [API Documentation](https://docs.rs/agent2389)
- [Configuration Guide](docs/configuration.md)
- [Tool Development](docs/tools.md)

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

---

## Implementation Checklist for Autonomous Agents

### Week 1: Foundation with TDD ✅

- [ ] **Set up project structure** with `cargo new agent2389 --lib`
- [ ] **Configure quality toolchain** - install clippy, rustfmt, cargo-watch, etc.
- [ ] **Write failing tests first** for protocol message types
- [ ] **Implement protocol message types** to make tests pass
- [ ] **Add property-based tests** for canonicalization with proptest
- [ ] **Create configuration system** with comprehensive validation
- [ ] **Verify Phase 1 checkpoint** - all tests pass, clippy clean

**TDD Loop:** Write test → Watch it fail → Implement minimal code → Watch it pass → Refactor

### Week 2: MQTT Transport with Integration Tests ✅

- [ ] **Start MQTT broker** (localhost:1883) for integration tests
- [ ] **Write failing integration test** for MQTT connectivity
- [ ] **Implement MqttClient wrapper** with proper QoS handling
- [ ] **Add Last Will Testament** configuration and testing
- [ ] **Test retained message handling** and topic publishing
- [ ] **Benchmark MQTT throughput** - must handle 1000+ msg/sec
- [ ] **Verify Phase 2 checkpoint** - integration tests pass

**Quality Gate:** `cargo test --test mqtt_integration && cargo bench mqtt_throughput`

### Week 3: Tool System with Schema Validation ✅

- [ ] **Write failing tests** for Tool trait and registry
- [ ] **Implement Tool trait** with async methods and error handling
- [ ] **Create ToolRegistry** with JSON schema validation
- [ ] **Implement 3 built-in tools** (http_request, file_read, file_write)
- [ ] **Add property-based tests** for tool parameter validation
- [ ] **Test tool error handling** and security validation
- [ ] **Verify Phase 3 checkpoint** - all tool tests pass

**Quality Gate:** `cargo test tools::registry::tests && cargo test tools::builtin::tests`

### Week 4: Task Processing with 9-Step Algorithm ✅

- [ ] **Write test for each of the 9 steps** - TDD each step individually
- [ ] **Implement step 1-3** (receive, retained check, topic validation)
- [ ] **Implement step 4-6** (idempotency, depth check, parsing)
- [ ] **Implement step 7-9** (LLM processing, next handling, completion)
- [ ] **Add comprehensive error handling** for each failure mode
- [ ] **Test pipeline depth enforcement** with property-based testing
- [ ] **Verify Phase 4 checkpoint** - protocol compliance tests pass

**Quality Gate:** `cargo test --test protocol_compliance::test_fr_014`

### Week 5: LLM Integration with Mock Testing ✅

- [ ] **Write failing tests** for LLM provider trait
- [ ] **Create mock LLM provider** for testing
- [ ] **Implement provider for Anthropic Claude**
- [ ] **Add tool calling support** in LLM requests and responses
- [ ] **Test tool result processing** and error handling
- [ ] **Add connectivity verification** and retry logic
- [ ] **Verify Phase 5 checkpoint** - LLM integration tests pass

**Quality Gate:** `cargo test llm::provider::tests && cargo test --test llm_integration`

### Week 6: Agent Lifecycle with Real MQTT ✅

- [ ] **Write complete integration test** for startup/shutdown sequence
- [ ] **Implement 8-step startup sequence** - TDD each step
- [ ] **Add proper shutdown sequence** with graceful cleanup
- [ ] **Create main event loop** with MQTT message handling
- [ ] **Test abnormal termination** and LWT handling
- [ ] **Add graceful shutdown signals** (SIGTERM, SIGINT)
- [ ] **Verify Phase 6 checkpoint** - end-to-end tests pass

**Quality Gate:** `cargo test --test integration_test && ./scripts/quality-check.sh`

### Week 7: Integration & Performance Testing ✅

- [ ] **Run complete protocol compliance suite** - all [req: X] tested
- [ ] **Performance benchmarking** - meet 1000+ msg/sec requirement
- [ ] **Error injection testing** - verify graceful degradation
- [ ] **Long-running stability test** - 24h continuous operation
- [ ] **Memory leak detection** with valgrind/sanitizers
- [ ] **Security audit** with cargo-audit and manual review
- [ ] **Verify Phase 7 checkpoint** - production readiness

**Quality Gate:** `./scripts/check-requirements.py && cargo bench`

### Week 8: Documentation & Polish ✅

- [ ] **Complete API documentation** with rustdoc examples
- [ ] **Update CLAUDE.md** with implementation learnings
- [ ] **Create usage examples** and configuration guides
- [ ] **Polish error messages** - remove any sensitive information
- [ ] **Final code review** - check for scope creep
- [ ] **Prepare release** - semantic versioning and changelog
- [ ] **Verify final checkpoint** - ready for production deployment

**Quality Gate:** `cargo doc --no-deps && cargo package --dry-run`

---

## Quality Gates Summary

### Automated Checks (run on every commit)

```bash
# The "make it work" pipeline - must pass before any commit
cargo fmt --check                              # Code formatting
cargo clippy --all-targets -- -D warnings     # Linting
cargo check --all-targets                     # Type checking
cargo test --lib                              # Unit tests
./scripts/check-requirements.py               # Requirement coverage
```

### Phase Gates (run before moving to next phase)

```bash
# Phase completion verification
cargo test                                     # All tests pass
cargo bench --bench protocol_benchmarks       # Performance acceptable
cargo audit                                   # No security issues
cargo tarpaulin --fail-under 80               # >80% code coverage
./scripts/quality-check.sh                    # Complete quality check
```

### Production Gates (run before release)

```bash
# Production readiness verification
cargo test --release                          # Release mode testing
cargo test --test integration_test            # Real MQTT integration
cargo doc --no-deps --document-private-items  # Complete documentation
cargo package --dry-run                       # Package verification
cargo publish --dry-run                       # Publish verification
```

---

## Success Criteria for Autonomous Agents

### Technical Success ✅

- [ ] **100% protocol compliance** - all [req: X] requirements implemented
- [ ] **>1000 msg/sec throughput** - meets performance requirements
- [ ] **>80% test coverage** - comprehensive test suite
- [ ] **Zero clippy warnings** - production code quality
- [ ] **Complete documentation** - every public API has examples
- [ ] **Security audit clean** - no known vulnerabilities

### Autonomous Agent Success ✅

- [ ] **Claude Code can implement 95%+ from this spec** - minimal human intervention needed
- [ ] **Clear error guidance** - when something breaks, error messages guide to fix
- [ ] **No scope creep** - implements exactly what protocol requires, nothing more
- [ ] **Self-validating** - comprehensive test suite catches regressions
- [ ] **Incremental progress** - each phase builds on previous, clear checkpoints

### Production Success ✅

- [ ] **Interoperability verified** - works with other 2389 implementations
- [ ] **24/7 operation stable** - handles errors gracefully, no memory leaks
- [ ] **Observable** - comprehensive logging and metrics for operations
- [ ] **Maintainable** - clear code structure, good separation of concerns
- [ ] **Extensible** - new tools can be added without breaking changes

**The ultimate test:** Could another autonomous agent take over maintenance of this codebase using only the
documentation and tests as guidance? If yes, we've succeeded.

---

## Final Notes for Autonomous Implementation

### Key Principles for AI Agents

1. **Test-Driven Development is Essential**
   - Write failing test first, then minimal implementation
   - Property-based testing catches edge cases humans miss
   - Integration tests with real systems prevent surprises

2. **Rust's Type System is Your Friend**
   - Let the compiler guide implementation
   - Use `cargo check` for fast feedback
   - Strong typing prevents entire classes of bugs

3. **Quality Gates Prevent Technical Debt**
   - Run quality checks on every commit
   - Clippy catches common mistakes and anti-patterns
   - Automated benchmarking prevents performance regressions

4. **Scope Control Prevents Feature Creep**
   - Only implement features with `[req: X]` tags
   - Question every "nice to have" feature
   - Prefer explicit, boring code over clever solutions

5. **Documentation is Implementation**
   - Every public API needs rustdoc examples
   - Error messages should guide users to solutions
   - CLAUDE.md should be complete guide for future maintenance

**Remember:** The goal is not just working code, but maintainable, tested, documented code that other agents
(or humans) can easily understand and extend.

---

## Success Criteria

### For Autonomous Agents

1. **Claude Code can implement 90%+ of the codebase** from this specification alone
2. **Error messages guide agents to correct implementations** when issues arise
3. **Examples are comprehensive** and copy-pastable
4. **All requirements are explicitly tagged** and traceable
5. **Configuration is self-documenting** with clear field descriptions

### For Production Use

1. **Protocol compliance**: 100% compliant with 2389 Agent Protocol
2. **Performance**: Handle 1000+ messages/second per agent
3. **Reliability**: 99.9% uptime with graceful error handling
4. **Observability**: Comprehensive logging and metrics
5. **Security**: No credential leakage, proper input validation

---

## Getting Started for Autonomous Agents

When implementing this specification:

1. **Start with the protocol types** in `src/protocol/messages.rs`
2. **Think step-by-step** - each method should do exactly what the docstring says
3. **Use the error types** - they map directly to protocol error codes
4. **Follow the patterns** - async/await, proper error handling, comprehensive logging
5. **Test incrementally** - build up the functionality piece by piece

The specification is designed to be implemented by autonomous agents. Every public API includes examples, error
conditions are explicit, and the architecture follows clear patterns that are easy to replicate.

**Key insight for autonomous agents**: This is not just a code specification - it's a complete system design that
includes configuration, deployment, testing, and operational considerations. Implement each phase completely
before moving to the next.
