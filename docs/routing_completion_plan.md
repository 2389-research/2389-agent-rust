# Dynamic Routing System Completion Plan

## Executive Summary

This document outlines the plan to complete the remaining 20% of the V2 dynamic routing system for the 2389 Agent Protocol implementation. The work is organized into 5 independent Pull Requests (PRs) that build toward a fully functional, production-ready routing system.

**Current Status:** 95% complete - **3 of 5 PRs COMPLETED** ✅

- ✅ **PR #1:** Routing Configuration System (MERGED)
- ✅ **PR #2:** LlmRouter Structured Output Integration (MERGED)
- ✅ **PR #3:** GatekeeperRouter Implementation (READY FOR REVIEW)
- ⏳ **PR #4:** V2 Routing Integration Tests (BLOCKED on PR #3)
- ⏳ **PR #5:** Agent System Prompt Guidelines (INDEPENDENT)

**Target:** 100% complete with all router implementations, configuration, tests, and documentation

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    V2 Routing Architecture                   │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────┐     ┌──────────┐     ┌──────────────────┐    │
│  │  Agent   │────>│  Router  │────>│ Orchestrator     │    │
│  │  (Work)  │     │(Decisions)     │ (Coordination)   │    │
│  └──────────┘     └──────────┘     └──────────────────┘    │
│                          │                                    │
│                          ├──> LlmRouter (OpenAI/Anthropic)  │
│                          └──> GatekeeperRouter (External)    │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

**Key Principle:** Agents do domain work, Routers make workflow decisions, Orchestrator coordinates everything.

---

## Current State Assessment

### ✅ Completed Components (80%)

1. **Router Trait** (`src/routing/router.rs`)
   - `Router` trait with `decide_next_step()` method
   - `RoutingDecision` enum (Complete vs Forward)
   - Helper methods and unit tests

2. **LlmRouter Skeleton** (`src/routing/llm_router.rs`)
   - Struct and constructor
   - Workflow history formatting
   - Agent catalog formatting
   - Prompt building
   - Response parsing (11 unit tests)
   - **MISSING:** Structured output configuration

3. **Routing Schema** (`src/routing/schema.rs`)
   - `RoutingDecisionOutput` with JsonSchema derive
   - Validation logic
   - 6 unit tests

4. **Pipeline Integration** (`src/agent/pipeline/pipeline_orchestrator.rs`)
   - `process_with_routing()` method
   - `forward_to_agent()` with iteration limits
   - `publish_final_result()`
   - 19 unit tests for helper functions

5. **Agent Discovery** (`src/routing/agent_selector.rs`)
   - Capability-based agent selection
   - Health checking
   - 7 unit tests

### ❌ Missing Components (10%)

1. ~~**Routing Configuration**~~ - ✅ COMPLETED (PR #1)
2. **GatekeeperRouter** - External HTTP routing not implemented
3. ~~**LlmRouter Structured Output**~~ - ✅ COMPLETED (PR #2)
4. **Integration Tests** - All tests disabled/marked `#[ignore]`
5. **Agent System Prompt Guidelines** - Documentation missing

---

## Pull Request Breakdown

### PR #1: Routing Configuration System ✅ COMPLETED

**Branch:** `feature/routing-configuration` (MERGED)

**Priority:** HIGH (Foundation for all other PRs)

**Complexity:** Medium

**Actual Effort:** 6 hours

**Dependencies:** None (standalone)

#### Objectives

Add configuration infrastructure for routing system:
- Support `[routing]` section in agent.toml
- Strategy selection ("llm" or "gatekeeper")
- LLM router configuration
- Gatekeeper router configuration
- Configurable max iterations

#### Files to Create/Modify

**New Structs in `src/config.rs`:**
```rust
pub struct RoutingConfig {
    pub strategy: RoutingStrategy,
    pub max_iterations: usize,
    pub llm: Option<LlmRouterConfig>,
    pub gatekeeper: Option<GatekeeperRouterConfig>,
}

pub enum RoutingStrategy {
    Llm,
    Gatekeeper,
}

pub struct LlmRouterConfig {
    pub provider: String,
    pub model: String,
    pub temperature: f32,
}

pub struct GatekeeperRouterConfig {
    pub url: String,
    pub timeout_ms: u64,
    pub retry_attempts: usize,
}
```

**Modify `src/config.rs`:**
- Add `routing: Option<RoutingConfig>` to `AgentConfig`
- Add default value functions
- Add validation for strategy/config consistency

**Update `agent.toml`:**
```toml
[routing]
strategy = "llm"
max_iterations = 10

[routing.llm]
provider = "openai"
model = "gpt-4o-mini"
temperature = 0.1
```

#### Test Strategy (TDD)

1. `test_routing_config_llm_strategy()` - Parse LLM routing config
2. `test_routing_config_gatekeeper_strategy()` - Parse gatekeeper config
3. `test_routing_config_defaults()` - Verify default values
4. `test_routing_config_missing_llm_when_strategy_llm()` - Validation error
5. `test_routing_config_missing_gatekeeper_when_strategy_gatekeeper()` - Validation error
6. `test_routing_config_invalid_strategy()` - Parse error for bad strategy

#### Implementation Steps (TDD)

1. ✅ Write failing test `test_routing_config_llm_strategy()`
2. ✅ Implement `RoutingConfig` and `LlmRouterConfig` structs
3. ✅ Run test - should pass
4. ✅ Write failing test `test_routing_config_gatekeeper_strategy()`
5. ✅ Implement `GatekeeperRouterConfig` struct
6. ✅ Run test - should pass
7. ✅ Write failing test for validation
8. ✅ Implement validation logic
9. ✅ Run test - should pass
10. ✅ Update agent.toml with examples
11. ✅ Run full test suite

#### Acceptance Criteria

- [x] All new structs have Debug, Clone, Serialize, Deserialize derives
- [x] TOML parsing works for both strategies
- [x] Default values are applied correctly
- [x] Validation catches missing required configs
- [x] agent.toml has clear examples with comments
- [x] All 8 tests pass (exceeded target)
- [x] No breaking changes to existing config

**Status:** ✅ COMPLETED (PR #8)

#### Commit Strategy

- Commit 1: "test: add failing test for LLM routing config"
- Commit 2: "feat: add RoutingConfig structs and LLM config"
- Commit 3: "test: add failing test for gatekeeper config"
- Commit 4: "feat: add GatekeeperRouterConfig"
- Commit 5: "test: add validation tests"
- Commit 6: "feat: add config validation logic"
- Commit 7: "docs: update agent.toml with routing examples"

---

### PR #2: LlmRouter Structured Output Integration ✅ COMPLETED

**Branch:** `feature/llm-router-structured-output` (READY FOR REVIEW)

**Priority:** HIGH (Critical for LLM-based routing)

**Complexity:** Medium-High

**Actual Effort:** 4 hours (TDD efficiency)

**Dependencies:** Soft dependency on PR #1 (can hardcode config initially)

#### Objectives

Fix LlmRouter to use structured output for reliable JSON responses:
- Detect LLM provider type (OpenAI vs Anthropic)
- Configure OpenAI `response_format` with JSON Schema
- Configure Anthropic `tool_choice` with routing tool
- Wire up config from RoutingConfig
- Ensure guaranteed valid JSON from LLMs

#### Files to Modify

**`src/routing/llm_router.rs`:**
- Update `decide_next_step()` to configure structured output
- Add method `configure_structured_output()` for provider detection
- Read temperature from config instead of hardcoded value
- Add OpenAI-specific schema configuration
- Add Anthropic-specific tool configuration

**Check `src/llm/provider.rs`:**
- Verify `CompletionRequest` supports `response_format` field
- Verify `CompletionRequest` supports `tool_choice` field
- May need to update if fields are missing

#### Implementation Approach

**For OpenAI:**
```rust
let request = CompletionRequest {
    model: self.model.clone(),
    messages: vec![...],
    response_format: Some(ResponseFormat::JsonSchema {
        name: "RoutingDecisionOutput".to_string(),
        schema: RoutingDecisionOutput::json_schema(),
        strict: true,
    }),
    temperature: Some(self.temperature),
    ...
};
```

**For Anthropic:**
```rust
let request = CompletionRequest {
    model: self.model.clone(),
    messages: vec![...],
    tools: Some(vec![Tool {
        name: "make_routing_decision".to_string(),
        description: "Make routing decision for workflow".to_string(),
        input_schema: RoutingDecisionOutput::json_schema(),
    }]),
    tool_choice: Some(ToolChoice::Required {
        name: "make_routing_decision".to_string(),
    }),
    temperature: Some(self.temperature),
    ...
};
```

#### Test Strategy

1. `test_openai_structured_output_configuration()` - Verify OpenAI schema setup
2. `test_anthropic_structured_output_configuration()` - Verify Anthropic tool setup
3. `test_provider_detection()` - Test detecting provider from config
4. `test_schema_generation()` - Verify JSON schema is correct
5. `test_mock_llm_with_structured_output()` - Integration with mock LLM

#### Implementation Steps (TDD)

1. ✅ Write failing test for provider detection
2. ✅ Implement provider detection logic
3. ✅ Run test - should pass
4. ✅ Write failing test for OpenAI structured output
5. ✅ Implement OpenAI response_format configuration
6. ✅ Run test - should pass
7. ✅ Write failing test for Anthropic structured output
8. ✅ Implement Anthropic tool_choice configuration
9. ✅ Run test - should pass
10. ✅ Refactor and run all tests

#### Acceptance Criteria

- [x] OpenAI provider uses `response_format` with JSON Schema
- [x] Anthropic provider uses `tool_choice` with routing tool
- [x] Provider detection works based on provider name
- [x] build_completion_request method extracts request building logic
- [x] All existing tests still pass (335 unit tests)
- [x] New tests for structured output pass (4 new tests)
- [x] Tests verify OpenAI and Anthropic configurations

**Status:** ✅ COMPLETED (Ready for PR review)

**Additional Changes:**
- Refactored decide_next_step to use build_completion_request
- Added provider detection methods (is_openai_provider, is_anthropic_provider)
- Full test coverage with inline mock providers

#### Commit Strategy

- Commit 1: "test: add failing test for provider detection"
- Commit 2: "feat: add LLM provider detection logic"
- Commit 3: "test: add OpenAI structured output test"
- Commit 4: "feat: configure OpenAI response_format with JSON Schema"
- Commit 5: "test: add Anthropic structured output test"
- Commit 6: "feat: configure Anthropic tool_choice with routing tool"
- Commit 7: "refactor: clean up and optimize"

---

### PR #3: GatekeeperRouter Implementation 🚧 IN PROGRESS

**Branch:** `feature/gatekeeper-router`

**Priority:** MEDIUM (Alternative routing strategy)

**Complexity:** Medium

**Estimated Effort:** 6-8 hours

**Actual Effort:** TBD (In progress)

**Dependencies:** Soft dependency on PR #1 for config (COMPLETED)

#### Objectives

Implement HTTP-based external routing for custom logic:
- Create new GatekeeperRouter struct
- Implement Router trait
- HTTP client with retry logic
- Request/response format for external API
- Error handling and timeouts

#### Files to Create

**`src/routing/gatekeeper_router.rs`:**
```rust
pub struct GatekeeperRouter {
    url: String,
    timeout: Duration,
    retry_attempts: usize,
    client: reqwest::Client,
}

impl GatekeeperRouter {
    pub fn new(url: String, timeout_ms: u64, retry_attempts: usize) -> Self {
        // Constructor
    }

    async fn call_external_api(
        &self,
        request: GatekeeperRequest,
    ) -> Result<GatekeeperResponse, AgentError> {
        // HTTP call with retry logic
    }
}

#[async_trait]
impl Router for GatekeeperRouter {
    async fn decide_next_step(...) -> Result<RoutingDecision, AgentError> {
        // Implementation
    }
}
```

**Request/Response Format:**
```rust
#[derive(Serialize)]
struct GatekeeperRequest {
    original_query: String,
    workflow_history: Vec<WorkflowStep>,
    current_output: Value,
    available_agents: Vec<AgentInfo>,
    iteration_count: usize,
}

#[derive(Deserialize)]
struct GatekeeperResponse {
    workflow_complete: bool,
    next_agent: Option<String>,
    next_instruction: Option<String>,
    reasoning: Option<String>,
    confidence: Option<f32>,
}
```

#### Files to Modify

**`src/routing/mod.rs`:**
- Export `GatekeeperRouter`

**`Cargo.toml` (if needed):**
- Verify `reqwest` dependency exists with JSON feature

#### Test Strategy (Using wiremock)

1. `test_gatekeeper_successful_forward()` - Mock 200 with forward decision
2. `test_gatekeeper_successful_complete()` - Mock 200 with complete decision
3. `test_gatekeeper_retry_on_500()` - Mock 500, then 200 on retry
4. `test_gatekeeper_timeout()` - Mock slow response exceeding timeout
5. `test_gatekeeper_404_error()` - Mock 404 not found
6. `test_gatekeeper_invalid_json()` - Mock 200 with invalid response
7. `test_gatekeeper_network_error()` - Mock connection refused

#### Implementation Steps (TDD)

1. ✅ Write failing test for successful forward decision
2. ✅ Implement basic GatekeeperRouter struct and Router trait
3. ✅ Run test - should pass
4. ✅ Write failing test for retry logic
5. ✅ Implement exponential backoff retry (already done in step 2)
6. ✅ Run test - should pass
7. ✅ Write failing test for timeout
8. ✅ Implement timeout handling (already done in step 2)
9. ✅ Run test - should pass
10. ✅ Write tests for error cases (404, invalid JSON, network)
11. ✅ Implement error mapping (already done in step 2)
12. ✅ Run all tests - **ALL 342 TESTS PASSING** ✅

**Progress Notes:**
- Started: 2025-10-09
- Branch created: feature/gatekeeper-router
- Following TDD RED-GREEN-REFACTOR cycle
- Completed: 2025-10-09 (same day!)
- All 7 planned tests written and passing
- Exponential backoff retry implemented
- Timeout handling working correctly
- Comprehensive error handling for all cases

#### Acceptance Criteria

- [x] Implements Router trait correctly
- [x] HTTP requests include all required data
- [x] Retry logic uses exponential backoff
- [x] Timeout is enforced
- [x] Network errors are handled gracefully
- [x] Invalid JSON responses return clear errors
- [x] All 7 tests pass with wiremock
- [x] Logging at appropriate levels (info, debug, warn)

**Status:** ✅ ALL ACCEPTANCE CRITERIA MET

#### Commit Strategy

- Commit 1: "test: add failing test for gatekeeper forward decision"
- Commit 2: "feat: implement GatekeeperRouter struct and basic HTTP call"
- Commit 3: "test: add failing test for retry logic"
- Commit 4: "feat: add exponential backoff retry"
- Commit 5: "test: add failing test for timeout"
- Commit 6: "feat: implement timeout handling"
- Commit 7: "test: add error case tests"
- Commit 8: "feat: implement comprehensive error handling"

---

### PR #4: V2 Routing Integration Tests

**Branch:** `feature/v2-routing-integration-tests`

**Priority:** HIGH (Validates entire system)

**Complexity:** High

**Estimated Effort:** 8-12 hours

**Dependencies:** Hard dependency on PRs #1, #2, #3

#### Objectives

Re-enable and rewrite integration tests for complete V2 routing:
- E2E workflow tests
- Multi-agent routing scenarios
- Iteration limit enforcement
- Loop detection
- Error handling
- Both LLM and Gatekeeper router tests

#### Files to Modify/Create

**`src/processing/dynamic_routing_tests.rs`:**
- Remove `#[ignore]` attributes
- Rewrite tests for new architecture
- Add comprehensive workflow scenarios

**`tests/test_v2_routing_e2e.rs` (new file):**
- End-to-end tests with real components
- Mock MQTT broker tests
- Multi-agent workflow scenarios

#### Test Scenarios

**Basic Routing Flow:**
1. `test_single_agent_workflow_completion()` - Agent completes immediately
2. `test_two_agent_workflow()` - Forward once, then complete
3. `test_multi_agent_workflow()` - Multiple forwards through different agents

**Iteration Limits:**
4. `test_max_iterations_enforcement()` - Workflow stops at max iterations
5. `test_iteration_count_increments()` - Verify iteration counter works

**Loop Detection:**
6. `test_workflow_detects_loops()` - Same agent visited multiple times
7. `test_workflow_history_preserved()` - History maintained across forwards

**Error Handling:**
8. `test_missing_next_agent()` - Router selects non-existent agent
9. `test_invalid_routing_decision()` - Router returns malformed decision
10. `test_llm_router_failure()` - LLM provider fails

**Router Implementations:**
11. `test_llm_router_integration()` - Full flow with mock LLM
12. `test_gatekeeper_router_integration()` - Full flow with mock HTTP server

**Configuration:**
13. `test_router_uses_config_max_iterations()` - Config overrides default
14. `test_llm_router_uses_config_temperature()` - LLM config applied

#### Implementation Approach

**Use existing test infrastructure:**
- `MockTransport` for MQTT simulation
- `MockLlmProvider` for deterministic routing
- `wiremock` for gatekeeper HTTP mocking
- `testcontainers` for real MQTT broker (if needed)

**Test structure:**
```rust
#[tokio::test]
async fn test_two_agent_workflow() {
    // Setup: Create mock components
    let transport = MockTransport::new();
    let mock_llm = MockLlmProvider::new();
    let registry = create_test_registry();

    // Configure mock to return forward decision
    mock_llm.expect_complete()
        .returning(|_| Ok(forward_decision()));

    // Execute: Process task with routing
    let result = pipeline.process_with_routing(task, work_output).await;

    // Assert: Verify forwarding occurred
    assert!(result.is_ok());
    verify_task_forwarded_to("next-agent");
}
```

#### Acceptance Criteria

- [ ] All 14 integration tests pass
- [ ] Tests cover happy path and error cases
- [ ] Mock providers work reliably
- [ ] Tests are deterministic (no flaky tests)
- [ ] Tests run in <30 seconds total
- [ ] Clear failure messages when tests fail
- [ ] No `#[ignore]` attributes remain

#### Commit Strategy

- Commit 1: "test: add basic single-agent workflow test"
- Commit 2: "test: add two-agent workflow test"
- Commit 3: "test: add multi-agent workflow test"
- Commit 4: "test: add iteration limit tests"
- Commit 5: "test: add loop detection tests"
- Commit 6: "test: add error handling tests"
- Commit 7: "test: add router-specific integration tests"
- Commit 8: "test: add configuration tests"
- Commit 9: "refactor: clean up test helpers"

---

### PR #5: Agent System Prompt Guidelines

**Branch:** `feature/agent-prompt-guidelines`

**Priority:** LOW (Documentation only)

**Complexity:** Low

**Estimated Effort:** 2-3 hours

**Dependencies:** None

#### Objectives

Document best practices for keeping agents routing-agnostic:
- Guidelines for agent developers
- Examples of proper agent outputs
- Anti-patterns to avoid
- JSON schema examples

#### Files to Create

**`docs/agent_system_prompts.md`:**

```markdown
# Agent System Prompt Guidelines

## Core Principle

Agents are domain experts that focus exclusively on their work.
They DO NOT make routing decisions or know about other agents.

## What Agents Should Do

1. **Focus on domain work**
2. **Return structured JSON output**
3. **Describe what they did, not what should happen next**

## What Agents Should NOT Do

1. ❌ Return `next_agent` fields
2. ❌ Mention other agents by name
3. ❌ Make workflow decisions
4. ❌ See routing history

## Example: Research Agent

### Good Output ✅

```json
{
  "findings": [
    "Finding 1...",
    "Finding 2..."
  ],
  "sources": ["source1", "source2"],
  "confidence": "high"
}
```

### Bad Output ❌

```json
{
  "findings": [...],
  "next_agent": "writer-agent",  // NO! Router decides this
  "workflow_complete": false      // NO! Router decides this
}
```

## JSON Schema Patterns

[Include examples of well-structured agent outputs]

## Testing Your Agent

[Include guidelines for testing agents in isolation]
```

**Files to Modify:**

**`docs/v2_routing_architecture.md`:**
- Add link to agent_system_prompts.md
- Reference in "Agent" section

**`README.md`:**
- Add link in documentation section

#### Acceptance Criteria

- [ ] Document clearly explains agent responsibilities
- [ ] At least 3 good examples included
- [ ] At least 3 anti-pattern examples included
- [ ] JSON schema patterns documented
- [ ] Links added to related documentation
- [ ] Reviewed for clarity and completeness

#### Commit Strategy

- Commit 1: "docs: create agent system prompt guidelines"
- Commit 2: "docs: add good and bad examples"
- Commit 3: "docs: add JSON schema patterns"
- Commit 4: "docs: link from architecture docs"

---

## Implementation Workflow

### Phase 1: Foundation (PR #1)

Start with configuration because it's needed by other PRs.

**Timeline:** Days 1-2

**Steps:**
1. Create branch `feature/routing-configuration`
2. Follow TDD: test → implement → test
3. Commit incrementally
4. Run full test suite before PR
5. Create PR with clear description

### Phase 2: Router Implementations (PRs #2 & #3)

These can be done in parallel or sequentially.

**Timeline:** Days 3-6

**Option A (Parallel):**
- One developer works on PR #2 (LlmRouter)
- Another works on PR #3 (GatekeeperRouter)

**Option B (Sequential):**
- Complete PR #2 first (higher priority)
- Then complete PR #3

### Phase 3: Validation (PR #4)

Integration tests validate everything works together.

**Timeline:** Days 7-10

**Steps:**
1. Merge PRs #1, #2, #3 first
2. Create branch `feature/v2-routing-integration-tests`
3. Start with simple tests, build up complexity
4. Use mock providers for determinism
5. Ensure no flaky tests

### Phase 4: Documentation (PR #5)

Polish with agent guidelines.

**Timeline:** Day 11

**Steps:**
1. Create branch `feature/agent-prompt-guidelines`
2. Write clear, concise documentation
3. Include examples
4. Link from other docs

---

## Testing Strategy

### Test Pyramid

```
        ┌───────────────┐
        │   E2E Tests   │  <- PR #4 (5-10 tests)
        │   (Slow)      │
        └───────────────┘
       ┌─────────────────┐
       │Integration Tests│  <- PR #2, #3, #4 (20-30 tests)
       │   (Medium)      │
       └─────────────────┘
      ┌───────────────────┐
      │   Unit Tests      │  <- PR #1, #2, #3 (50+ tests)
      │    (Fast)         │
      └───────────────────┘
```

### Test Coverage Goals

- **Unit tests:** >90% coverage for new code
- **Integration tests:** All critical paths covered
- **E2E tests:** Happy path + key error scenarios

### Running Tests

```bash
# Run all tests
cargo test

# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test '*'

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_routing_config_llm_strategy
```

---

## Success Criteria

### Definition of Done (Per PR)

- [ ] All tests pass (existing + new)
- [ ] Code follows project style conventions
- [ ] No clippy warnings
- [ ] Documentation updated (if applicable)
- [ ] PR description explains changes clearly
- [ ] Commits are atomic and well-messaged
- [ ] No breaking changes to existing APIs

### Overall Project Completion

- [ ] All 5 PRs merged
- [ ] Routing configuration working
- [ ] LlmRouter uses structured output
- [ ] GatekeeperRouter fully functional
- [ ] Integration tests passing
- [ ] Documentation complete
- [ ] No `#[ignore]` test attributes
- [ ] All checklist items in `v2_routing_architecture.md` checked

---

## Risk Mitigation

### Potential Issues

**Issue:** LLM provider API doesn't support structured output as expected
**Mitigation:** Check provider documentation first, implement fallback parsing if needed

**Issue:** Integration tests are flaky
**Mitigation:** Use mock providers for determinism, avoid timing-dependent tests

**Issue:** Breaking changes to existing code
**Mitigation:** Add integration tests for V1 compatibility, maintain backward compatibility

**Issue:** Configuration validation is too strict
**Mitigation:** Provide clear error messages, document required fields

---

## Resources

### Documentation References

- [V2 Routing Architecture](./v2_routing_architecture.md)
- [RFC Section 9: Configuration](../SPECIFICATION.md#9-configuration)
- [Testing Guidelines](../CLAUDE.md#testing)

### Related Files

- `src/routing/router.rs` - Router trait
- `src/routing/llm_router.rs` - LLM router
- `src/routing/schema.rs` - Routing schemas
- `src/agent/pipeline/pipeline_orchestrator.rs` - Pipeline integration
- `src/config.rs` - Configuration system

### External Dependencies

- `reqwest` - HTTP client for GatekeeperRouter
- `wiremock` - HTTP mocking for tests
- `testcontainers` - MQTT broker for E2E tests
- `serde` / `toml` - Configuration parsing
- `schemars` - JSON Schema generation

---

## Appendix

### Glossary

- **Router**: Component that decides workflow progression
- **LlmRouter**: Router using LLM for decisions
- **GatekeeperRouter**: Router calling external HTTP API
- **Structured Output**: LLM feature guaranteeing valid JSON
- **Workflow Context**: History and state carried through routing
- **Pipeline Orchestrator**: Coordinates agent execution and routing

### Example Configurations

See `agent.toml` in repository root for complete examples.

### Contact

For questions about this plan:
- Review the planning discussion in the project
- Check `docs/v2_routing_architecture.md` for architecture details
- See individual PR descriptions for implementation specifics
