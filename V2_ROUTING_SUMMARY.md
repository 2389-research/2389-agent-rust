# V2 Dynamic Routing Implementation Summary

## Overview

This document summarizes the complete V2 routing implementation for the 2389 Agent Protocol in Rust. V2 routing enables agents to make dynamic, LLM-based routing decisions during task processing, allowing for iterative workflows where agents collaborate naturally to refine outputs based on feedback.

## Architecture

### Core Components

1. **Router Trait** (`src/routing/mod.rs`)
   - Abstract interface for routing decision engines
   - `route()` method takes task context and work output, returns routing decision
   - Enables pluggable routing strategies

2. **LlmRouter** (`src/routing/llm_router.rs`)
   - Concrete Router implementation using LLM for routing decisions
   - Invokes LLM with routing prompt and work output
   - Parses structured routing decisions from LLM response

3. **RoutingDecisionOutput** (`src/routing/schema.rs`)
   - Structured routing decision format
   - Fields:
     - `workflow_complete: bool` - Whether workflow should terminate
     - `reasoning: String` - Explanation of routing decision (for observability)
     - `next_agent: Option<String>` - Target agent ID for next step
     - `next_instruction: Option<String>` - Instruction for next agent

4. **AgentPipeline with Router** (`src/agent/pipeline/pipeline_orchestrator.rs`)
   - `AgentPipeline::with_router()` - Creates pipeline with V2 routing support
   - `process_with_routing()` - Processes tasks using router for routing decisions
   - Max iterations enforcement to prevent infinite loops
   - WorkflowContext tracking for iteration counts and history

### Workflow Flow

```text
┌─────────────┐
│ Agent A     │
│ receives    │
│ task        │
└──────┬──────┘
       │
       ▼
┌─────────────────┐
│ 9-step RFC      │
│ processing      │
│ (agent work)    │
└──────┬──────────┘
       │
       ▼
┌─────────────────┐
│ Router invoked  │
│ with work       │
│ output          │
└──────┬──────────┘
       │
       ├──── workflow_complete = true  ──► Publish final result
       │
       └──── workflow_complete = false
                    │
                    ▼
             ┌──────────────┐
             │ Forward to   │
             │ Agent B      │
             └──────┬───────┘
                    │
                    ▼
             ┌──────────────┐
             │ Agent B      │
             │ processes    │
             │ task         │
             └──────┬───────┘
                    │
                    ▼
                  (repeat)
```

## Implementation Details

### 1. Routing Decision Format

Routing decisions are structured JSON with the following schema:

```json
{
  "workflow_complete": false,
  "reasoning": "Routing to writer-agent for article generation",
  "next_agent": "writer-agent",
  "next_instruction": "Write a comprehensive article based on the research"
}
```

OR for workflow completion:

```json
{
  "workflow_complete": true,
  "reasoning": "Workflow completed successfully",
  "next_agent": null,
  "next_instruction": null
}
```

### 2. Max Iterations Enforcement

AgentPipeline enforces a configurable `max_iterations` limit to prevent runaway workflows:

- Default: 10 iterations
- Configurable per pipeline
- When limit reached, workflow automatically terminates
- Final result published to conversation topic

### 3. Context Preservation

WorkflowContext tracks workflow state across iterations:

```rust
pub struct WorkflowContext {
    pub original_query: String,          // Initial user request
    pub steps_completed: Vec<String>,    // History of agent steps
    pub iteration_count: usize,          // Current iteration number
}
```

Context is preserved and incremented with each routing decision.

### 4. Agent Discovery Integration

Routers can query AgentRegistry to discover available agents and their capabilities:

- `registry.get_agent(agent_id)` - Get agent metadata
- `registry.list_agents()` - List all available agents
- Enables intelligent routing based on agent capabilities

## Example Workflows

### 1. Research → Write → Edit (Linear)

```rust
// Scenario: Create a polished article
// 1. Research agent gathers information
// 2. Router forwards to Writer
// 3. Writer creates content
// 4. Router forwards to Editor
// 5. Editor polishes content
// 6. Router completes workflow
```

**Demonstrated in**: `examples/v2_workflow_demo.rs` with `--workflow research-write-edit`

### 2. Iterative Quality Refinement

```rust
// Scenario: Create high-quality content through feedback
// 1. Writer creates initial draft
// 2. Router forwards to Judge
// 3. Judge reviews and finds issues
// 4. Router forwards back to Writer with feedback
// 5. Writer improves content
// 6. Router forwards to Judge again
// 7. Judge approves
// 8. Router completes workflow
```

**Demonstrated in**: `examples/v2_workflow_demo.rs` with `--workflow iterative`

## Running V2 Routing Demos

### Prerequisites

- MQTT broker running at `localhost:1883` (always available, no testcontainers)
- Agent TOML configs in `examples/v2_routing_workflow/`

### Run Example Workflows

```bash
# List available workflows
cargo run --example v2_workflow_demo -- --list

# Run research-write-edit workflow
cargo run --example v2_workflow_demo -- --workflow research-write-edit

# Run iterative refinement workflow
cargo run --example v2_workflow_demo -- --workflow iterative --timeout 15

# Use real LLM (requires API keys)
cargo run --example v2_workflow_demo -- --workflow iterative --real-llm
```

### Example Output

```console
🚀 Starting V2 Routing Workflow Demo
📍 Workflow: Iterative Write → Judge → Refine
💬 MQTT Broker: mqtt://localhost:1883
🔧 Mode: Mock LLM

🏗️  Spawning agents...
   🤖 Starting agent: writer-agent
   ✅ Agent writer-agent ready and processing
   🤖 Starting agent: judge-agent
   ✅ Agent judge-agent ready and processing

▶️  Starting workflow...
   Initial task: Write a high-quality article about Rust async

📡 Monitoring workflow messages on /conversations/demo-xyz/...
📨 Conversation message from writer-agent (total: 1)
📨 Conversation message from judge-agent (total: 2)
📨 Conversation message from writer-agent (total: 3)
📨 Conversation message from judge-agent (total: 4)

⏱️  Timeout reached after 15s
   Workflow processed 4 conversation messages

🧹 Cleaning up agents...
✅ Demo complete
```

## Configuration

### Agent TOML Configuration

Agents require standard 2389 Agent Protocol TOML configs:

```toml
[agent]
id = "writer-agent"
description = "Creates written content based on requirements"
capabilities = ["writing", "content-creation"]

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "openai"
model = "gpt-4o-mini"
api_key_env = "OPENAI_API_KEY"
system_prompt = "You are a professional writer..."
temperature = 0.7
max_tokens = 2000

[tools]
# Optional tool configurations
```

### Router Configuration

LlmRouter uses the same LLM provider as the agent for routing decisions:

```rust
let router = Arc::new(LlmRouter::new(
    llm_provider,      // Same provider used for agent work
    "gpt-4o-mini",     // Model for routing decisions
));
```

## Testing

### Mock Testing

The `MockLlmProvider` supports agent decision testing:

```rust
use agent2389::testing::mocks::{AgentDecision, MockLlmProvider};

// Create mock with predefined routing decisions
let mock_llm = MockLlmProvider::with_agent_decisions(vec![
    AgentDecision::route_to("judge", "Review content", json!({"article": "..."})),
    AgentDecision::complete(json!({"final": "approved"})),
]);
```

### AgentDecision Helper

```rust
// Route to another agent
let decision = AgentDecision::route_to(
    "writer-agent",
    "Improve the article based on feedback",
    json!({"feedback": "Add more examples"})
);

// Complete workflow
let decision = AgentDecision::complete(
    json!({"status": "success"})
);

// Convert to JSON for router
let json_output = decision.to_json();
```

## Key Files

- `src/routing/mod.rs` - Router trait definition
- `src/routing/llm_router.rs` - LLM-based router implementation
- `src/routing/schema.rs` - Routing decision schema
- `src/agent/pipeline/pipeline_orchestrator.rs` - Pipeline with routing support
- `examples/v2_workflow_demo.rs` - Runnable V2 routing demonstrations
- `tests/test_v2_routing_e2e.rs` - End-to-end routing tests
- `src/testing/mocks.rs` - Mock LLM provider with AgentDecision support

## Pull Requests

The V2 routing implementation was delivered through the following PRs:

- **PR #1**: Router trait and LlmRouter implementation
- **PR #2**: RoutingDecisionOutput schema and structured prompts
- **PR #3**: AgentPipeline V2 routing integration
- **PR #4**: Mock testing infrastructure with AgentDecision
- **PR #5**: AgentRegistry integration for capability-based routing
- **Phase 2**: Runnable v2_workflow_demo with real MQTT workflows

## Observability

### Structured Logging

All routing decisions are logged with structured fields:

```text
INFO Parsed routing decision from LLM
     workflow_complete=false
     reasoning="Routing to judge-agent for further processing"
INFO Forwarding to next agent
     task_id=abc123
     next_agent=judge-agent
     next_instruction="Review this article for quality"
INFO Forwarded task to next agent
     next_agent=judge-agent
     iteration_count=1
```

### Monitoring

The v2_workflow_demo includes MQTT message monitoring:

- Subscribes to `/conversations/{conversation_id}/#`
- Tracks all conversation messages
- Reports statistics on workflow completion
- Detects when workflows finish based on iteration count

## Best Practices

1. **Set Reasonable Max Iterations**: Default is 10, adjust based on workflow complexity
2. **Provide Clear Instructions**: Each routing decision should include descriptive next_instruction
3. **Use Reasoning Field**: Always provide reasoning for observability and debugging
4. **Register All Agents**: Ensure all agents are registered in AgentRegistry before workflows start
5. **Monitor Conversation Topics**: Subscribe to `/conversations/{id}/#` for workflow visibility
6. **Handle Completion Gracefully**: Always check `workflow_complete` flag in routing decisions

## Future Enhancements

- Real LLM mode support in v2_workflow_demo (currently mock-only)
- Workflow completion detection in monitoring (currently timeout-based)
- Ping-pong workflow scenario demonstration
- Performance benchmarks for multi-agent workflows
- Routing decision caching for repeated patterns
- Enhanced routing prompts with few-shot examples

## References

- 2389 Agent Protocol Specification
- RFC-compliant 9-step task processing
- MQTT v5 protocol with QoS 1
- LLM provider abstraction layer
