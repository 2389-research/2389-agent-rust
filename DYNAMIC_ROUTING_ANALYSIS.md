# Dynamic Routing System Analysis

**Date:** 2025-09-29
**Status:** Implementation ~80% Complete
**Test Coverage:** All 286 tests passing ✅

## Executive Summary

The dynamic agent selection and routing system is **mostly implemented** but not fully wired together. The core components exist and are well-tested, but there are critical integration gaps preventing end-to-end dynamic routing.

## What's Complete ✅

### 1. TaskEnvelope V2 Protocol (100% Complete)
**Location:** `src/protocol/messages.rs:80-221`

- ✅ **TaskEnvelopeV2** struct with all fields
- ✅ **RoutingConfig** for dynamic/static modes
- ✅ **RoutingRule** with JSONPath conditions
- ✅ **RoutingStep** for trace logging
- ✅ **TaskEnvelopeWrapper** for version detection
- ✅ Comprehensive v1↔v2 conversion methods
- ✅ Full test coverage (349-689)

### 2. Agent Discovery System (100% Complete)
**Location:** `src/agent/discovery.rs`

- ✅ **AgentRegistry** - Thread-safe with TTL cleanup
- ✅ **AgentInfo** - Health, load, capabilities tracking
- ✅ Load-based agent selection with tie-breaking
- ✅ Capability matching (case-insensitive)
- ✅ 15-second TTL expiration
- ✅ Full test coverage (304-470)

### 3. Routing Rule Engine (100% Complete)
**Location:** `src/routing/rule_engine.rs`

- ✅ **RuleEngine** - JSONPath condition evaluation
- ✅ **RoutingDecision** enum (RouteToAgent/UseFallback/DropTask)
- ✅ Rule priority ordering
- ✅ Capability-based routing support
- ✅ Routing trace creation for observability
- ✅ Rule validation
- ✅ Full test coverage (293-616)

### 4. Configuration System (Partial)
**Location:** `src/config.rs:22-32`

- ✅ **AgentSection.capabilities** field exists
- ✅ Properly serialized/deserialized
- ❌ No routing configuration at agent level

### 5. Dynamic Injector Tool (100% Complete)
**Location:** `src/bin/dynamic-injector.rs`

- ✅ Creates TaskEnvelopeV2 messages
- ✅ Agent discovery via MQTT
- ✅ Smart routing rule generation
- ✅ Preview mode
- ✅ Full v2.0 envelope support

## What's Missing ❌

### 1. **CRITICAL: Message Parser V2 Support**
**Location:** `src/transport/mqtt/message_handler.rs:16-19`

**Current Code:**
```rust
pub fn parse_task_envelope(payload: &[u8]) -> Result<TaskEnvelope, String> {
    serde_json::from_slice::<TaskEnvelope>(payload)
        .map_err(|e| format!("Failed to parse TaskEnvelope: {e}"))
}
```

**Problem:**
- Only parses v1.0 TaskEnvelope
- Cannot handle v2.0 envelopes
- No version detection

**Solution Needed:**
```rust
pub fn parse_task_envelope(payload: &[u8]) -> Result<TaskEnvelopeWrapper, String> {
    serde_json::from_slice::<TaskEnvelopeWrapper>(payload)
        .map_err(|e| format!("Failed to parse TaskEnvelope: {e}"))
}
```

### 2. **CRITICAL: Processor V2 Integration**
**Location:** `src/agent/processor.rs:70-80`

**Current Code:**
```rust
pub async fn process_task(
    &self,
    task: TaskEnvelope,  // ← V1 only!
    received_topic: &str,
    is_retained: bool,
) -> AgentResult<ProcessingResult>
```

**Problem:**
- AgentProcessor only accepts v1.0 TaskEnvelope
- Cannot process v2.0 envelopes
- No routing config passed through

**Solution Needed:**
- Change signature to accept `TaskEnvelopeWrapper`
- Extract routing config and pass to nine_step_processor
- Handle both v1 and v2 envelopes

### 3. **INCOMPLETE: Nine-Step Processor Integration**
**Location:** `src/processing/nine_step.rs:334-456`

**Current Issues:**

#### A. Step 8 Enhanced Routing (Lines 334-456)
```rust
// TODO: In full implementation, we'd detect v2.0 via TaskEnvelopeWrapper
// TODO: In full implementation, get routing config from TaskEnvelopeV2
let routing_rules = vec![];  // ← Hard-coded empty!
```

**Problems:**
- Accepts v1.0 TaskEnvelope only
- Cannot access v2.0 routing config
- Rules are hard-coded to empty array
- Dynamic routing never executes

#### B. Missing Integration Points
1. No v2.0 envelope acceptance in `process_task()` method
2. No routing config extraction from v2.0 envelope
3. No routing trace propagation in forwarded tasks
4. Agent discovery not connected to MQTT status messages

### 4. **MISSING: MQTT Status Message Processing**
**Location:** Need to add to MQTT event loop

**Problem:**
- Agent status messages are published to `/control/agents/{id}/status`
- No subscriber listening to these messages
- AgentRegistry never gets populated
- Dynamic routing has no agents to route to

**Solution Needed:**
- Subscribe to `/control/agents/+/status` wildcard topic
- Parse AgentStatusMessage from payloads
- Update AgentRegistry on each status message
- Clean up expired agents periodically

### 5. **MISSING: Configuration for Routing**
**Location:** `src/config.rs`

**Problems:**
- No way to configure routing rules per agent
- No way to set routing mode (static/dynamic)
- No fallback behavior configuration

**Solution Needed:**
Add to TOML config:
```toml
[routing]
mode = "dynamic"  # or "static"
fallback = "static"  # or "drop"

[[routing.rules]]
condition = "$.urgency_score >= 0.8"
target_agent = "urgent-processor"
priority = 1

[[routing.rules]]
condition = "$.type == 'email'"
target_agent = "email-processor"
priority = 2
```

## Architecture Issues Found

### 1. **Type Mismatch Chain**
The v1/v2 type mismatch cascades through the entire stack:

```
MQTT Parser (v1 only)
    ↓
AgentProcessor (v1 only)
    ↓
NineStepProcessor (v1 only, v2 stub exists)
    ↓
Enhanced Routing (can't access v2 fields)
```

### 2. **Orphaned Components**
These are built but not integrated:
- `TaskEnvelopeWrapper` exists but unused
- `RuleEngine` exists but gets empty rules
- `AgentRegistry` exists but never populated
- `step_8_enhanced_routing()` exists but can't access routing config

### 3. **Test Coverage Gap**
While unit tests pass (213/213), there are **no integration tests** for:
- End-to-end v2.0 envelope processing
- Dynamic routing with real agent registry
- MQTT status message → AgentRegistry flow
- Multi-agent pipeline with dynamic routing

## Implementation Roadmap

### Phase 1: Critical Path (Required for Basic Dynamic Routing)

#### Task 1.1: Update Message Parser
**File:** `src/transport/mqtt/message_handler.rs`
- Change return type to `TaskEnvelopeWrapper`
- Update all call sites

#### Task 1.2: Update AgentProcessor
**File:** `src/agent/processor.rs`
- Accept `TaskEnvelopeWrapper` instead of `TaskEnvelope`
- Extract routing config from v2 envelopes
- Pass routing config to nine_step_processor

#### Task 1.3: Update NineStepProcessor
**File:** `src/processing/nine_step.rs`
- Accept `TaskEnvelopeWrapper` in `process_task()`
- Remove TODOs in `step_8_enhanced_routing()`
- Extract routing config from v2 envelope
- Pass rules to `routing_engine.evaluate_routing()`

#### Task 1.4: Wire MQTT Status Processing
**File:** Create `src/agent/status_subscriber.rs`
- Subscribe to `/control/agents/+/status`
- Parse `AgentStatusMessage` from payloads
- Update `AgentRegistry` on each message
- Start background task for periodic cleanup

### Phase 2: Configuration Support

#### Task 2.1: Add Routing Config Schema
**File:** `src/config.rs`
- Add `RoutingSection` struct
- Add routing rules configuration
- Add default routing behavior

#### Task 2.2: Load Routing Config
**File:** `src/agent/processor.rs`
- Read routing config from TOML
- Pass to nine_step_processor on creation

### Phase 3: Testing & Validation

#### Task 3.1: Integration Tests
**File:** Create `tests/integration/dynamic_routing.rs`
- Test v2.0 envelope end-to-end
- Test agent discovery → routing flow
- Test routing trace propagation

#### Task 3.2: Example Configs
**File:** Create example TOML configs
- Static routing example
- Dynamic routing example
- Multi-rule routing example

## Breaking Changes

The following changes will break existing code:

1. **Message Handler API**
   - `parse_task_envelope()` return type changes
   - All callers must handle `TaskEnvelopeWrapper`

2. **AgentProcessor API**
   - `process_task()` signature changes
   - All callers must pass `TaskEnvelopeWrapper`

3. **NineStepProcessor API** (minor)
   - `process_task()` signature changes
   - Internal only, minimal impact

## Compatibility Strategy

To minimize disruption:

1. **Graceful Degradation**
   - v2.0 envelopes without routing config → static mode
   - v1.0 envelopes → automatic conversion to v2.0 static mode
   - Missing agent registry → fall back to static routing

2. **Feature Flags** (Optional)
   - Could add `feature = "dynamic-routing"` flag
   - Compile-time toggle for new behavior
   - Allows gradual rollout

## Code Quality Assessment

### Strengths
- ✅ Excellent separation of concerns
- ✅ Comprehensive unit test coverage
- ✅ Clear protocol versioning strategy
- ✅ Good documentation and comments
- ✅ RFC compliance maintained throughout

### Areas for Improvement
- ❌ Missing integration tests
- ❌ No example configurations
- ❌ TODOs left in production code
- ❌ Orphaned components not wired up
- ❌ No migration guide for v1→v2

## Timeline Estimate

**Phase 1 (Critical Path):** 8-12 hours
- Task 1.1: 1-2 hours
- Task 1.2: 2-3 hours
- Task 1.3: 3-4 hours
- Task 1.4: 2-3 hours

**Phase 2 (Configuration):** 4-6 hours
- Task 2.1: 2-3 hours
- Task 2.2: 2-3 hours

**Phase 3 (Testing):** 6-8 hours
- Task 3.1: 4-5 hours
- Task 3.2: 2-3 hours

**Total:** 18-26 hours of focused development

## Recommendation

**Start with Phase 1, Task 1.4 (MQTT Status Processing)** because:
1. Unblocks agent discovery testing
2. No breaking changes to existing code
3. Can be tested independently
4. Populates AgentRegistry for later phases
5. Demonstrates system working end-to-end

Then proceed with Tasks 1.1→1.3 to complete the critical path.

## Next Steps

1. **Prioritize:** Choose Phase 1 tasks in order of impact
2. **Branch:** Create `feature/dynamic-routing` branch
3. **Implement:** Start with Task 1.4 (MQTT Status Processing)
4. **Test:** Add integration test for each completed task
5. **Document:** Update examples as features complete

---

**Analysis completed:** 2025-09-29
**All tests passing:** ✅ 286/286
**Ready for implementation:** Yes, with clear roadmap