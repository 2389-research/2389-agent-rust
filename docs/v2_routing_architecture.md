# V2 Routing Architecture

## Overview

V2 routing introduces a clean separation between **agent work** and **routing decisions**. Agents focus exclusively on their domain expertise (research, writing, editing, quality review) while a dedicated Router component decides workflow progression.

**Core Principle**: Agents are domain experts, not workflow coordinators. They don't know about other agents, don't make routing decisions, and don't see workflow history. All routing intelligence lives in the Router.

## Architecture: Three Layers

```
┌─────────────────────────────────────────┐
│         User Request                     │
└──────────────┬──────────────────────────┘
               ↓
┌─────────────────────────────────────────┐
│    PipelineOrchestrator                  │
│  - Coordinates agent execution           │
│  - Invokes router after each agent       │
│  - Manages workflow context              │
│  - Enforces iteration limits             │
└──────────────┬──────────────────────────┘
               ↓
       ┌───────┴───────┐
       ↓               ↓
┌─────────────┐  ┌─────────────┐
│   Agent     │  │   Router    │
│  (Work)     │  │ (Routing)   │
└─────────────┘  └─────────────┘

Agent:                      Router:
- Domain expertise          - Workflow intelligence
- Task execution            - Routing decisions
- Work output only          - Context-aware
- No routing logic          - Agent registry access
```

### Layer 1: Agent (Domain Expert)

**Responsibility**: Execute domain-specific work

> 📖 **See also:** [Agent System Prompt Guidelines](./agent_system_prompts.md) - Comprehensive guide to writing routing-agnostic agents

**What agents see:**
- Static system prompt (their role/expertise)
- Task instruction (what to do)
- Input data (data to work with)

**What agents DO NOT see:**
- Available agents catalog
- Workflow history
- Original user query context
- Routing decisions

**What agents return:**
- Work output (JSON results of their work)

**Example - Research Agent:**
```rust
System Prompt:
"You are a research agent. Conduct thorough research on topics,
gathering accurate information from reliable sources. Return findings
in structured JSON format."

Input:
- Instruction: "Research Herodotus's military campaigns"
- Data: {}

Output:
{
  "findings": [
    "Marathon (490 BCE): First major Persian defeat",
    "Thermopylae (480 BCE): Spartan stand",
    "Salamis (480 BCE): Naval victory"
  ],
  "sources": ["Histories Book 6-9"],
  "key_themes": ["hubris", "divine intervention", "Greek unity"]
}
```

**Example - Judge Agent:**
```rust
System Prompt:
"You are a quality review agent. Evaluate work outputs for completeness,
accuracy, and quality. Identify specific issues and gaps. Return assessment
in structured JSON format."

Input:
- Instruction: "Evaluate this document for quality"
- Data: { "document": "..." }

Output:
{
  "quality_score": 7,
  "strengths": ["well-structured", "accurate facts"],
  "weaknesses": ["lacks strategic analysis", "missing historiographical context"],
  "recommendation": "needs_improvement",
  "specific_gaps": [
    "No analysis of Herodotus's strategic insights",
    "Missing context on his methodology"
  ]
}
```

**CRITICAL: Agents never return routing hints like `next_agent` or `workflow_complete`. That's the Router's job.**

### Layer 2: Router (Workflow Intelligence)

**Responsibility**: Decide what happens next after agent completes work

**What routers see:**
- Original user query
- Complete workflow history (all steps taken)
- Current agent's output
- Available agents catalog (from registry)
- Current iteration count

**What routers decide:**
- Is workflow complete? (satisfied user's request)
- OR which agent should handle next? (and what instruction to give them)

**Router Interface:**
```rust
pub trait Router: Send + Sync {
    async fn decide_next_step(
        &self,
        original_task: &TaskEnvelopeV2,
        work_output: &Value,
        registry: &AgentRegistry,
    ) -> Result<RoutingDecision>;
}

pub enum RoutingDecision {
    Complete { final_output: Value },
    Forward {
        next_agent: String,
        next_instruction: String,
        forwarded_data: Value
    },
}
```

**Two Router Implementations:**

#### LlmRouter - LLM-based routing decisions

Uses structured output (JSON Schema for OpenAI, Tool schemas for Anthropic) to make routing decisions.

**Router sees:**
```
ORIGINAL USER REQUEST:
Create a fully polished document on Herodotus's military campaigns

WORKFLOW HISTORY (Iteration 3/10):
1. research-agent
   Action: Researched Herodotus's military campaigns
   Time: 2024-01-15 10:30:00

2. writer-agent
   Action: Wrote comprehensive document
   Time: 2024-01-15 10:35:00

3. editor-agent
   Action: Polished document for publication
   Time: 2024-01-15 10:40:00

CURRENT AGENT OUTPUT:
{
  "polished_document": "# Herodotus: The Father of History...",
  "edits_made": ["improved clarity", "fixed grammar"],
  "quality_level": "publication-ready"
}

AVAILABLE AGENTS:
- research-agent (capabilities: research, fact-checking)
- writer-agent (capabilities: writing, structure)
- editor-agent (capabilities: editing, polish)
- judge-agent (capabilities: quality-review)

DECISION CRITERIA:
1. Has the original user request been fully satisfied?
2. What work remains to complete the request?
3. Which agent is best suited for the remaining work?
4. Are we in a loop? (Check if same agent visited multiple times)
5. Are we approaching max iterations? (Currently at 3/10)
```

**Router returns (Structured JSON):**
```json
{
  "workflow_complete": false,
  "reasoning": "Document is polished but should be quality-reviewed before completion",
  "next_agent": "judge-agent",
  "next_instruction": "Review this document for completeness and quality"
}
```

#### GatekeeperRouter - External API routing decisions

Calls external service that may use specialized logic, ML models, or custom rules.

**API Request:**
```json
POST /route
{
  "original_query": "Create polished document on Herodotus",
  "workflow_history": [
    {"agent_id": "research-agent", "action": "...", "timestamp": "..."},
    {"agent_id": "writer-agent", "action": "...", "timestamp": "..."}
  ],
  "current_output": { "document": "..." },
  "available_agents": [
    {"agent_id": "editor-agent", "capabilities": ["editing"]},
    {"agent_id": "judge-agent", "capabilities": ["quality-review"]}
  ]
}
```

**API Response:**
```json
{
  "workflow_complete": false,
  "next_agent": "editor-agent",
  "next_instruction": "Polish this document to publication quality",
  "confidence": 0.95
}
```

### Layer 3: PipelineOrchestrator (Coordinator)

**Responsibility**: Coordinate agent execution and routing

**Flow:**
```rust
pub async fn process_with_routing(&self, task: TaskEnvelopeV2) -> Result<()> {
    // 1. Agent does its work
    let work_output = self.processor.process_task(task.clone()).await?;

    // 2. Router decides next step
    let decision = self.router.decide_next_step(
        &task,
        &work_output,
        &self.registry
    ).await?;

    // 3. Act on decision
    match decision {
        RoutingDecision::Complete { final_output } => {
            // Publish final result to conversation
            self.publish_final_result(&task.conversation_id, &final_output).await?;
        }
        RoutingDecision::Forward { next_agent, next_instruction, forwarded_data } => {
            // Forward to next agent
            self.forward_to_agent(&task, next_agent, next_instruction, forwarded_data).await?;
        }
    }

    Ok(())
}
```

**Forwarding Logic:**
```rust
async fn forward_to_agent(
    &self,
    original_task: &TaskEnvelopeV2,
    next_agent: String,
    next_instruction: String,
    forwarded_data: Value,
) -> Result<()> {
    // Update workflow context
    let mut new_context = original_task.context.clone().unwrap_or_default();
    new_context.iteration_count += 1;

    // SAFETY: Enforce max iterations
    if new_context.iteration_count >= 10 {
        warn!("Max iterations reached, completing workflow");
        return self.publish_final_result(
            &original_task.conversation_id,
            &forwarded_data
        ).await;
    }

    // Add current step to history
    new_context.steps_completed.push(WorkflowStep {
        agent_id: self.processor.config().agent.id.clone(),
        action: extract_action_summary(&forwarded_data),
        timestamp: Utc::now().to_rfc3339(),
    });

    // Create task for next agent
    let next_task = TaskEnvelopeV2 {
        task_id: Uuid::new_v4(),
        conversation_id: original_task.conversation_id.clone(),
        topic: format!("/control/agents/{}/input", next_agent),
        instruction: Some(next_instruction),
        input: forwarded_data,
        next: None,
        version: "2.0".to_string(),
        context: Some(new_context),
        routing_trace: None,
    };

    // Publish to next agent's input topic
    self.processor.transport().publish_task(&next_task).await?;

    Ok(())
}
```

## Complete Workflow Example

### Scenario: "Create a fully polished document on Herodotus's military campaigns"

**Step 1: Research Agent**
```
Input: "Research Herodotus's military campaigns"
Agent work: Researches topic
Output: {"findings": [...], "sources": [...]}

Router evaluates:
- Original query needs polished DOCUMENT
- We have research but no document
- Writer agent can structure research into document
Decision: Forward to writer-agent
```

**Step 2: Writer Agent**
```
Input: "Write comprehensive document from this research"
       + research data
Agent work: Writes document
Output: {"document": "# Herodotus and Military History\n\n..."}

Router evaluates:
- We have document but query wants "polished"
- Editor agent can polish documents
Decision: Forward to editor-agent
```

**Step 3: Editor Agent**
```
Input: "Polish this document to publication quality"
       + draft document
Agent work: Edits and polishes
Output: {"polished_document": "...", "quality_level": "publication-ready"}

Router evaluates:
- Document is polished
- But should verify it's actually complete
- Judge agent can evaluate quality
Decision: Forward to judge-agent
```

**Step 4: Judge Agent**
```
Input: "Evaluate document quality and completeness"
       + polished document
Agent work: Reviews document
Output: {
  "quality_score": 7,
  "weaknesses": ["lacks strategic analysis"],
  "recommendation": "needs_improvement"
}

Router evaluates:
- Judge found gap: strategic analysis missing
- Need more research on that specific topic
- Research agent should deep-dive
Decision: Forward to research-agent with specific instruction
```

**Step 5: Research Agent (Second Pass)**
```
Input: "Research Herodotus's strategic insights and military analysis methods"
Agent work: Deep research on strategic aspects
Output: {"strategic_analysis": [...], "methodology": [...]}

Router evaluates:
- We have strategic analysis now
- Writer should integrate into existing document
Decision: Forward to writer-agent
```

**Step 6: Writer Agent (Second Pass)**
```
Input: "Integrate this strategic analysis into the existing document"
       + strategic analysis + previous document
Agent work: Updates document with new analysis
Output: {"enhanced_document": "..."}

Router evaluates:
- Document updated
- Should polish again
Decision: Forward to editor-agent
```

**Step 7: Editor Agent (Second Pass)**
```
Input: "Polish the updated document"
Agent work: Final polish
Output: {"final_document": "..."}

Router evaluates:
- Everything polished
- Let judge verify one more time
Decision: Forward to judge-agent
```

**Step 8: Judge Agent (Second Pass)**
```
Input: "Final quality check"
Agent work: Reviews enhanced document
Output: {
  "quality_score": 9,
  "strengths": ["comprehensive", "strategic depth", "well-polished"],
  "recommendation": "approved"
}

Router evaluates:
- Quality approved
- Strategic analysis added
- Document polished
- Original query fully satisfied
Decision: Complete workflow
```

**Final: Orchestrator publishes result to user**

## Structured Output Implementation

### JSON Schema Definition

```rust
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoutingDecisionOutput {
    /// Whether the workflow is complete
    pub workflow_complete: bool,

    /// Reasoning for the routing decision
    pub reasoning: String,

    /// Next agent ID (required if workflow_complete is false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_agent: Option<String>,

    /// Instruction for next agent (required if workflow_complete is false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_instruction: Option<String>,
}
```

### OpenAI Integration (JSON Schema)

```rust
client.complete_request(ChatCompletionRequest {
    model: "gpt-4o".to_string(),
    messages: vec![
        ChatMessage { role: "system", content: "You are a workflow router." },
        ChatMessage { role: "user", content: routing_prompt }
    ],
    response_format: Some(ResponseFormat::JsonSchema {
        name: "RoutingDecisionOutput".to_string(),
        schema: routing_decision_schema(),
        strict: true,  // Guaranteed valid JSON
    }),
    ..Default::default()
}).await?
```

### Anthropic Integration (Tool Schema)

```rust
client.complete_with_tools(Request {
    model: "claude-3-5-sonnet-20241022".to_string(),
    system: Some("You are a workflow router.".to_string()),
    messages: vec![
        Message { role: "user", content: routing_prompt }
    ],
    tools: vec![
        Tool {
            name: "make_routing_decision".to_string(),
            description: "Make routing decision for workflow".to_string(),
            input_schema: routing_decision_schema(),
        }
    ],
    tool_choice: ToolChoice::Required {
        name: "make_routing_decision".to_string(),
    },
    ..Default::default()
}).await?
```

## Configuration

```toml
[routing]
# Which router implementation to use
strategy = "llm"  # or "gatekeeper"

# Maximum workflow iterations before forced completion
max_iterations = 10

# LLM router configuration
[routing.llm]
provider = "openai"  # or "anthropic"
model = "gpt-4o-mini"
temperature = 0.1  # Low temperature for consistent routing

# Gatekeeper router configuration
[routing.gatekeeper]
url = "http://localhost:8080/gatekeeper"
timeout_ms = 5000
retry_attempts = 3
```

## Protocol Structures

### TaskEnvelopeV2

```rust
pub struct TaskEnvelopeV2 {
    pub task_id: Uuid,
    pub conversation_id: String,
    pub topic: String,
    pub instruction: Option<String>,
    pub input: Value,
    pub next: Option<Box<NextTask>>,  // V1 compatibility
    pub version: String,

    // V2: Workflow context
    pub context: Option<WorkflowContext>,

    // Observability
    pub routing_trace: Option<Vec<RoutingStep>>,
}
```

### WorkflowContext

```rust
pub struct WorkflowContext {
    /// Original user query preserved through entire workflow
    pub original_query: String,

    /// Steps completed so far
    pub steps_completed: Vec<WorkflowStep>,

    /// Current iteration count (safety counter)
    pub iteration_count: usize,
}

pub struct WorkflowStep {
    pub agent_id: String,
    pub action: String,
    pub timestamp: String,
}
```

## Key Design Principles

### ✅ DO: Things We Want

1. **Separation of Concerns**
   - Agents focus on domain work
   - Router focuses on workflow decisions
   - Orchestrator coordinates both

2. **Structured Output**
   - Use JSON Schema for OpenAI
   - Use Tool schemas for Anthropic
   - Guarantee valid JSON responses

3. **Context-Aware Routing**
   - Router sees full workflow history
   - Router sees original user query
   - Router knows available agents
   - Router tracks iteration count

4. **Safety Mechanisms**
   - Max iteration limit (default: 10)
   - Loop detection via history
   - Forced completion on limit

5. **Extensibility**
   - Router trait allows new implementations
   - Configuration-driven strategy selection
   - Easy to add custom routing logic

6. **Clean Agent Interface**
   - Static system prompts
   - Simple input/output
   - No routing knowledge required

### ❌ DON'T: Things We Absolutely Avoid

1. **Agents Making Routing Decisions**
   - ❌ Agents returning `next_agent` field
   - ❌ Agents knowing about other agents
   - ❌ Agents seeing workflow history
   - ❌ Agents making workflow decisions

2. **Complex Context Injection**
   - ❌ Appending workflow context to agent instructions
   - ❌ Mixing routing concerns with agent prompts
   - ❌ Agents seeing agent catalogs

3. **Dual-Purpose Outputs**
   - ❌ Agent output containing both work AND routing hints
   - ❌ Overloading response structures
   - ❌ Ambiguous "this could be work or routing" patterns

4. **Clever Abstractions**
   - ❌ Generic "handles everything" router
   - ❌ Auto-detecting routing strategies
   - ❌ Complex capability matching logic
   - ❌ Rule engines or condition evaluators

5. **Tight Coupling**
   - ❌ Agents depending on router implementation
   - ❌ Router depending on specific agent types
   - ❌ Hard-coded routing paths

6. **Parsing Nightmares**
   - ❌ Agents returning free-form text with routing hints
   - ❌ Parsing markdown or natural language for decisions
   - ❌ Extracting JSON from explanations

## Migration Path

### V1 Tasks (Still Supported)

V1 tasks with static `next` field continue working:

```json
{
  "task_id": "...",
  "conversation_id": "...",
  "topic": "/control/agents/agent1/input",
  "instruction": "Do something",
  "input": {},
  "next": {
    "topic": "/control/agents/agent2/input",
    "instruction": "Continue"
  },
  "version": "1.0"
}
```

### V2 Tasks (New Router-Based)

V2 tasks use router for dynamic decisions:

```json
{
  "task_id": "...",
  "conversation_id": "...",
  "topic": "/control/agents/agent1/input",
  "instruction": "Do something",
  "input": {},
  "version": "2.0",
  "context": {
    "original_query": "User's request",
    "steps_completed": [],
    "iteration_count": 0
  }
}
```

After agent completes, router decides next step dynamically.

## Implementation Checklist

- [x] Add `iteration_count` to `WorkflowContext`
- [x] Create `Router` trait in `src/routing/router.rs`
- [x] Implement `LlmRouter` with structured output support
- [x] Implement `GatekeeperRouter` with HTTP client
- [x] Create routing decision schema with `JsonSchema` derive
- [x] Add `format_workflow_history()` helper
- [x] Add `format_agent_catalog()` helper
- [x] Update `PipelineOrchestrator` with router integration
- [x] Add routing configuration to `config.toml`
- [x] Update forwarding logic with iteration limit
- [x] Add integration tests for router implementations
- [x] Document agent system prompt guidelines - **[See agent_system_prompts.md](./agent_system_prompts.md)**

## Summary

V2 routing achieves clean separation through three layers:

- **Agents**: Domain experts doing focused work
- **Router**: Workflow intelligence making routing decisions
- **Orchestrator**: Coordinator managing the flow

Key benefits:
- ✅ Agents stay simple and focused
- ✅ Routing logic centralized and testable
- ✅ Extensible via Router trait
- ✅ Structured output guarantees valid JSON
- ✅ Context-aware decisions with full workflow history
- ✅ Safety mechanisms prevent infinite loops
- ✅ Configuration-driven strategy selection

The design prioritizes ruthless simplicity: agents don't route, routers don't do work, and the orchestrator just coordinates. No clever tricks, no tight coupling, no parsing nightmares.