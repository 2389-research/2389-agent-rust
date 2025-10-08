# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This repository implements the 2389 Agent Protocol in Rust - a standardized protocol for creating interoperable AI agents that communicate via MQTT. This is a greenfield implementation that prioritizes correctness, performance, and strict protocol compliance.

## Architecture Overview

The project follows a modular, async-first design with the following key components:

### Core Module Structure
- `src/lib.rs` - Public API surface with comprehensive examples
- `src/agent/` - Agent lifecycle management (startup, shutdown, orchestration)
- `src/protocol/` - Protocol message types and validation
- `src/transport/` - MQTT transport layer with QoS handling
- `src/processing/` - 9-step task processing algorithm implementation
- `src/tools/` - Trait-based tool system with JSON schema validation
- `src/llm/` - LLM provider abstractions and adapters
- `src/error.rs` - Comprehensive error types mapping to protocol codes

### Key Architecture Principles
- **Async-first design**: All I/O operations use tokio async runtime
- **Strong typing**: Protocol messages use serde with comprehensive validation
- **Error handling**: Specific error types mapping to protocol error codes
- **Tool system**: Trait-based architecture with runtime schema validation
- **MQTT integration**: QoS 1, proper Last Will Testament, topic canonicalization

## Development Commands

Since this is a new Rust project, use these standard commands:

### Initial Setup
```bash
# Create the Rust library project
cargo new --lib agent2389
cd agent2389

# Set up development dependencies in Cargo.toml
# See TECHNICAL_REQUIREMENTS.md for complete dependency list
```

### Development Workflow
```bash
# Format code
cargo fmt

# Lint and fix issues
cargo clippy --fix --allow-dirty

# Type checking (fast feedback)
cargo check --all-targets

# Run all tests
cargo test

# Run specific test module
cargo test protocol::messages::tests

# Run integration tests
cargo test --test integration_test

# Generate documentation
cargo doc --no-deps --open

# Run benchmarks
cargo bench

# Watch for changes during development
cargo watch -x "fmt" -x "clippy --fix --allow-dirty" -x "test"
```

### Quality Gates
```bash
# Pre-commit checks
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --lib

# Integration testing with real MQTT broker
cargo test --test integration_test

# Performance benchmarking
cargo bench --bench protocol_benchmarks

# Security audit
cargo audit

# Code coverage
cargo tarpaulin --fail-under 80
```

## Implementation Strategy

### Phase-Based Development
The specification defines an 8-week implementation plan with specific phases:

1. **Week 1**: Foundation - Protocol message types and validation
2. **Week 2**: MQTT Transport - Connection, QoS, Last Will Testament
3. **Week 3**: Tool System - Trait definition and built-in tools
4. **Week 4**: Task Processing - 9-step algorithm implementation
5. **Week 5**: LLM Integration - Provider trait and adapters
6. **Week 6**: Agent Lifecycle - Startup/shutdown sequences
7. **Week 7**: Integration & Performance Testing
8. **Week 8**: Documentation & Polish

### Test-Driven Development
- Write failing tests first for each feature
- Use property-based testing with `proptest` crate for edge cases
- Integration tests with real MQTT broker using `testcontainers`
- All protocol requirements tagged `[req: X]` must have corresponding tests

## Code Quality Standards

### Required Patterns
- Use `#[must_use]` on all Result types
- Implement Display and Debug for all public types
- Include rustdoc examples for all public functions
- Follow "return early" pattern for error handling
- Use specific error types over generic ones
- Comprehensive logging with `tracing` crate

### Error Handling Strategy
- Agent MUST NOT crash on invalid input
- Errors published to MQTT conversation topics
- Error messages MUST NOT contain sensitive information
- All tool calls validated against JSON schemas
- Map all errors to protocol-defined error codes

## Protocol Compliance Requirements

This implementation MUST comply with all requirements tagged `[req: X]`:

### Critical Requirements
- **[req: FR-001]** - Interoperability with other implementations
- **[req: FR-002]** - Complete 8-step startup sequence
- **[req: FR-003]** - Proper shutdown sequence  
- **[req: FR-014]** - Exact 9-step task processing algorithm
- **[req: FR-013]** - Pipeline depth enforcement (max 16)
- **[req: FR-006]** - Topic canonicalization rules
- **[req: FR-010]** - Idempotency handling with task ID deduplication

### Message Flow
1. Agent subscribes to `/control/agents/{agent_id}/input`
2. Processes TaskEnvelope messages using 9-step algorithm
3. Publishes errors to `/conversations/{conversation_id}/{agent_id}`
4. Forwards results to next agent in pipeline if specified
5. Maintains availability status at `/control/agents/{agent_id}/status`

## Configuration System

### agent.toml Structure
```toml
[agent]
id = "agent-name"           # Format: [a-zA-Z0-9._-]+
description = "Agent description"
capabilities = ["capability1", "capability2"]  # Optional list of agent capabilities

[mqtt]
broker_url = "mqtt://localhost:1883"
username_env = "MQTT_USERNAME"
password_env = "MQTT_PASSWORD"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are a helpful AI agent."
temperature = 0.7
max_tokens = 4000

[tools]
http_request = "builtin"
file_read = "builtin"
file_write = { impl = "builtin", config = { max_file_size = 1048576 } }
```

## Tool System

### Tool Trait Implementation
All tools must implement:
- `describe()` - Returns JSON schema for parameters
- `initialize()` - Setup with configuration
- `execute()` - Process with validated parameters
- `shutdown()` - Cleanup resources (optional)

### Built-in Tools
- `http_request` - HTTP requests with size limits
- `file_read` - File reading with security checks
- `file_write` - File writing with validation

### Security Considerations
- All tool parameters validated against JSON schemas
- Tool execution in isolated contexts
- No access to sensitive environment variables
- Response size limits enforced

## Performance Requirements

- Handle 1000+ messages/second per agent
- Sub-100ms processing latency for simple tasks
- Memory usage bounded (no memory leaks)
- Graceful degradation under load

## Key Implementation Notes

### Topic Canonicalization
```rust
// Protocol rules for topic canonicalization:
// 1. Ensure single leading slash
// 2. Remove trailing slashes  
// 3. Collapse multiple consecutive slashes
canonicalize_topic("//control//agents/foo/") == "/control/agents/foo"
```

### 9-Step Processing Algorithm
Must execute ALL steps in exact sequence:
1. Receive message on input topic
2. Ignore retained messages  
3. Canonicalize and validate topic match
4. Check for duplicate task_id (idempotency)
5. Check pipeline depth (max 16)
6. Parse task envelope
7. Process with LLM and tools
8. Forward to next agent if specified
9. Mark task as completed

### Error Recovery
- Network disconnections: Reconnect with exponential backoff
- Tool failures: Log error, continue processing
- LLM failures: Publish error to conversation topic
- Pipeline depth exceeded: Publish error, stop processing

## Development Environment

### Required Dependencies
```toml
# Runtime dependencies
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
rumqttc = "0.21"
uuid = { version = "1.0", features = ["v4"] }
tracing = "0.1"
reqwest = { version = "0.11", features = ["json"] }

# Development dependencies
proptest = "1.0"
testcontainers = "0.15" 
tokio-test = "0.4"
wiremock = "0.5"
```

### Testing Strategy
- Unit tests for all modules with >80% coverage
- Property-based tests for protocol edge cases
- Integration tests with real MQTT broker
- Performance benchmarks for critical paths
- Error injection tests for robustness

## Success Criteria

### For Autonomous Implementation
- Claude Code can implement 95%+ from specification alone
- Clear error guidance when issues arise  
- No scope creep beyond protocol requirements
- Self-validating through comprehensive test suite

### For Production Deployment
- 100% protocol compliance verified
- >1000 msg/sec throughput achieved
- 24/7 operation stability demonstrated
- Complete observability and monitoring
- Security audit passed

## Important Reminders

- **Test-driven development is essential** - Write failing tests first
- **Follow exact protocol specification** - No deviations or additions
- **Quality gates must pass** before each phase completion
- **Documentation examples must be copy-pastable**
- **Error messages should guide users to solutions**
- **Think hard about edge cases** - This is production system software