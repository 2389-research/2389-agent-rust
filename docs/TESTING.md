# Testing Guide - 2389 Agent Protocol Rust Implementation

Comprehensive guide for testing the 2389 Agent Protocol implementation, including automated testing, manual verification procedures, and protocol compliance validation.

## Table of Contents

- [Quick Start](#quick-start)
- [Testing Infrastructure](#testing-infrastructure)
- [Automated Testing Scripts](#automated-testing-scripts)
- [Manual Testing Procedures](#manual-testing-procedures)
- [Protocol Compliance Verification](#protocol-compliance-verification)
- [MQTT Broker Testing](#mqtt-broker-testing)
- [Integration Testing](#integration-testing)
- [Performance Testing](#performance-testing)
- [Troubleshooting](#troubleshooting)
- [Test Environment Configuration](#test-environment-configuration)

## Quick Start

### Prerequisites

Before running tests, ensure you have:

```bash
# Required tools
sudo apt-get install mosquitto-clients  # Ubuntu/Debian
brew install mosquitto                   # macOS

# Environment variables
export OPENAI_API_KEY="your-openai-key"        # Optional
export ANTHROPIC_API_KEY="your-anthropic-key"  # Optional
```

### 30-Second Test

Run the complete startup verification test:

```bash
# Build and test agent startup
./scripts/test-agent-startup.sh

# In another terminal, monitor MQTT traffic
./scripts/mqtt-monitor.sh

# Send a test task
./scripts/send-test-task.sh
```

Expected output: Agent connects to MQTT, processes echo task, responds on conversation topic.

## Testing Infrastructure

### Test Configuration

The test environment uses `config/test-agent.toml` with these key settings:

```toml
# Agent configuration
agent_id = "test-agent-rust"
max_pipeline_depth = 4
task_timeout = 60

# MQTT broker (mqtt.2389.dev)
[mqtt]
broker_url = "mqtts://mqtt.2389.dev:8883"
username = "robots"
password = "Hat-Compass-Question-Remove4-Shirt"
qos = 1
```

**Security Note**: These are test-only credentials. Never use in production environments.

### MQTT Broker Details

**Test Broker**: `mqtt.2389.dev:8883`
- **Protocol**: MQTT over TLS (MQTTS)
- **Authentication**: Username/password
- **QoS Level**: 1 (At least once delivery)
- **TLS**: Self-signed certificate accepted for testing

### Agent Topics

The test agent subscribes to and publishes on these topics:

```
# Input (agent receives tasks)
/control/agents/test-agent-rust/input

# Status updates (agent publishes)
/control/agents/test-agent-rust/status

# Error reporting (agent publishes)
/control/agents/test-agent-rust/errors

# Conversation responses (agent publishes)
/conversations/{conversation_id}/test-agent-rust
```

## Automated Testing Scripts

### 1. Agent Startup Test (`test-agent-startup.sh`)

**Purpose**: Verifies complete agent startup sequence and MQTT connectivity.

```bash
./scripts/test-agent-startup.sh
```

**What it tests**:
- Rust build system and dependencies
- MQTT broker connectivity
- Agent initialization sequence
- Configuration file validation
- Log output analysis
- Graceful shutdown

**Success criteria**:
- Agent builds without errors
- MQTT connection established
- Agent reaches "ready" state
- Clean startup/shutdown cycle

### 2. MQTT Monitor (`mqtt-monitor.sh`)

**Purpose**: Real-time monitoring of MQTT topics for debugging and verification.

```bash
# Monitor specific agent
./scripts/mqtt-monitor.sh test-agent-rust

# Monitor with different agent ID
./scripts/mqtt-monitor.sh my-custom-agent
```

**Monitored topics**:
- Agent status updates
- Error messages
- Task inputs
- Response outputs
- Broadcast messages
- All agent statuses

**Output format**:
```
[STATUS] 14:30:15 /control/agents/test-agent-rust/status {"status": "ready", "timestamp": "..."}
[INPUT] 14:30:20 /control/agents/test-agent-rust/input {"task_id": "task-123", ...}
[OUTPUT] 14:30:22 /conversations/conv-456/test-agent-rust {"response": "Hello!", ...}
```

### 3. Test Task Sender (`send-test-task.sh`)

**Purpose**: Sends properly formatted test tasks to verify agent processing.

```bash
# Send default test task
./scripts/send-test-task.sh

# Send to specific agent with custom message
./scripts/send-test-task.sh my-agent "Custom test message"
```

**Task envelope format**:
```json
{
  "task_id": "task-1693837200-1234",
  "conversation_id": "conv-1693837200",
  "agent_id": "test-agent-rust",
  "content": "Please use the echo tool to echo this message: Hello from test!",
  "tools_available": ["echo"],
  "pipeline_depth": 1,
  "next_agent_id": null,
  "created_at": "2024-01-01T12:00:00Z"
}
```

## Manual Testing Procedures

### Basic Agent Functionality

#### 1. Manual Agent Startup

```bash
# Build the agent
cargo build --release

# Start agent with test configuration
cargo run --release -- --config config/test-agent.toml run
```

**Expected behavior**:
1. Configuration validation
2. MQTT connection establishment
3. Topic subscriptions
4. Status publication to `/control/agents/test-agent-rust/status`
5. Ready state achievement

#### 2. Task Processing Verification

**Step 1**: Start agent and monitor
```bash
# Terminal 1: Start agent
cargo run --release -- --config config/test-agent.toml run

# Terminal 2: Monitor MQTT
./scripts/mqtt-monitor.sh test-agent-rust
```

**Step 2**: Send test task
```bash
# Terminal 3: Send task
./scripts/send-test-task.sh test-agent-rust "Test message for verification"
```

**Step 3**: Verify response sequence
1. Task received on input topic
2. Agent processes task (check logs)
3. Tool execution (echo command)
4. Response published to conversation topic
5. Status remains "ready"

### Error Handling Verification

#### Invalid Task Format

Send malformed JSON to test error handling:

```bash
mosquitto_pub \
  -h mqtt.2389.dev \
  -p 8883 \
  -u robots \
  -P 'Hat-Compass-Question-Remove4-Shirt' \
  --capath /etc/ssl/certs \
  --insecure \
  -t '/control/agents/test-agent-rust/input' \
  -q 1 \
  -m '{"invalid": "json", "missing_required_fields": true}'
```

**Expected**: Error published to `/control/agents/test-agent-rust/errors`

#### Tool Execution Failure

Send task with non-existent tool:

```bash
./scripts/send-test-task.sh test-agent-rust "Test with invalid tool"
# Edit the generated task to include "nonexistent_tool" in tools_available
```

**Expected**: Error response indicating tool not found

#### Pipeline Depth Enforcement

Send task with excessive pipeline depth:

```json
{
  "task_id": "depth-test-123",
  "conversation_id": "conv-depth",
  "agent_id": "test-agent-rust",
  "content": "Test pipeline depth",
  "tools_available": ["echo"],
  "pipeline_depth": 17,
  "next_agent_id": null
}
```

**Expected**: Error due to pipeline depth > 16 limit

## Protocol Compliance Verification

### Topic Canonicalization Testing

Verify the agent handles malformed topics correctly:

```bash
# Test various malformed topics (should normalize to canonical form)
mosquitto_pub ... -t '//control///agents//test-agent-rust//input//' -m '...'
mosquitto_pub ... -t 'control/agents/test-agent-rust/input/' -m '...'
mosquitto_pub ... -t '/control/agents/test-agent-rust/input///' -m '...'
```

All should normalize to: `/control/agents/test-agent-rust/input`

### Message Idempotency

Send duplicate tasks with same `task_id`:

```bash
# Send first task
TASK_ID="duplicate-test-123"
./scripts/send-test-task.sh test-agent-rust "First message"

# Send duplicate (same task_id)
./scripts/send-test-task.sh test-agent-rust "Duplicate message"
```

**Expected**: Second message ignored (idempotency handling)

### QoS Level Compliance

Verify QoS 1 (at-least-once) delivery:

```bash
# Monitor with QoS verification
mosquitto_sub \
  -h mqtt.2389.dev \
  -p 8883 \
  -u robots \
  -P 'Hat-Compass-Question-Remove4-Shirt' \
  --capath /etc/ssl/certs \
  --insecure \
  -t '/conversations/+/test-agent-rust' \
  -q 1 \
  -v
```

**Expected**: All messages acknowledge receipt

## MQTT Broker Testing

### Connectivity Verification

#### Basic Connection Test

```bash
# Test MQTT connectivity
mosquitto_pub \
  -h mqtt.2389.dev \
  -p 8883 \
  -u robots \
  -P 'Hat-Compass-Question-Remove4-Shirt' \
  --capath /etc/ssl/certs \
  --insecure \
  -t '/test/connectivity' \
  -m 'Connection test' \
  -q 1
```

#### TLS Configuration

Verify TLS settings work correctly:

```bash
# Should connect successfully with --insecure flag
# Should fail without --insecure (self-signed cert)
mosquitto_pub \
  -h mqtt.2389.dev \
  -p 8883 \
  -u robots \
  -P 'Hat-Compass-Question-Remove4-Shirt' \
  --capath /etc/ssl/certs \
  -t '/test/tls' \
  -m 'TLS test' \
  -q 1
```

### Subscription Pattern Testing

Verify wildcard subscriptions work:

```bash
# Subscribe to all agent statuses
mosquitto_sub \
  -h mqtt.2389.dev \
  -p 8883 \
  -u robots \
  -P 'Hat-Compass-Question-Remove4-Shirt' \
  --capath /etc/ssl/certs \
  --insecure \
  -t '/control/agents/+/status' \
  -q 1 \
  -v

# Subscribe to all conversation responses
mosquitto_sub ... -t '/conversations/+/+' -q 1 -v
```

## Integration Testing

### Multi-Agent Pipeline Testing

#### Setup Agent Pipeline

1. **Agent A** (test-agent-rust): Processes task, forwards to Agent B
2. **Agent B** (test-agent-2): Receives forwarded task, completes pipeline

**Configuration for Agent A**:
```json
{
  "task_id": "pipeline-test-123",
  "conversation_id": "conv-pipeline",
  "agent_id": "test-agent-rust", 
  "content": "Process and forward to next agent",
  "tools_available": ["echo"],
  "pipeline_depth": 1,
  "next_agent_id": "test-agent-2"
}
```

**Expected flow**:
1. Agent A processes task
2. Agent A forwards to Agent B with `pipeline_depth = 2`
3. Agent B processes and completes (no forwarding)
4. Final response published to conversation topic

### Tool Integration Testing

#### HTTP Request Tool

```json
{
  "task_id": "http-test-123",
  "conversation_id": "conv-http",
  "agent_id": "test-agent-rust",
  "content": "Make an HTTP request to httpbin.org/get",
  "tools_available": ["http_request"],
  "pipeline_depth": 1
}
```

#### File Operations

```json
{
  "task_id": "file-test-123", 
  "conversation_id": "conv-file",
  "agent_id": "test-agent-rust",
  "content": "Write 'test content' to file /tmp/test.txt and read it back",
  "tools_available": ["file_write", "file_read"],
  "pipeline_depth": 1
}
```

## Performance Testing

### Load Testing

#### Message Throughput Test

```bash
# Generate high-frequency test messages
for i in {1..100}; do
  TASK_ID="load-test-$i"
  ./scripts/send-test-task.sh test-agent-rust "Load test message $i" &
  sleep 0.1
done
wait
```

**Metrics to monitor**:
- Message processing rate
- Response latency
- Memory usage
- CPU utilization
- MQTT connection stability

#### Concurrent Task Processing

Send multiple tasks simultaneously:

```bash
# Send 10 concurrent tasks
for i in {1..10}; do
  (
    TASK_ID="concurrent-$i-$(date +%s)"
    ./scripts/send-test-task.sh test-agent-rust "Concurrent task $i"
  ) &
done
wait
```

**Expected**: All tasks processed correctly without interference

### Memory and Resource Testing

#### Long-Running Stability

Run agent for extended periods:

```bash
# Start agent with memory monitoring
cargo run --release -- --config config/test-agent.toml run &
AGENT_PID=$!

# Monitor resource usage
while kill -0 $AGENT_PID 2>/dev/null; do
  ps -p $AGENT_PID -o pid,ppid,pcpu,pmem,etime,comm
  sleep 60
done
```

#### Pipeline Depth Limits

Test maximum pipeline depth handling:

```bash
# Send tasks with increasing pipeline depths
for depth in {1..20}; do
  echo "Testing pipeline depth: $depth"
  # Send task with pipeline_depth = $depth
  # Verify depths > 16 are rejected
done
```

## Troubleshooting

### Common Issues and Solutions

#### 1. MQTT Connection Failures

**Symptom**: Agent fails to connect to MQTT broker

**Diagnosis**:
```bash
# Test direct MQTT connection
mosquitto_pub \
  -h mqtt.2389.dev \
  -p 8883 \
  -u robots \
  -P 'Hat-Compass-Question-Remove4-Shirt' \
  --capath /etc/ssl/certs \
  --insecure \
  -t '/test' \
  -m 'test' \
  -q 1 -d
```

**Solutions**:
- Check internet connectivity
- Verify TLS configuration (`--insecure` flag)
- Confirm credentials are correct
- Check firewall settings (port 8883)

#### 2. Agent Not Responding to Tasks

**Symptom**: Tasks sent but no responses received

**Diagnosis checklist**:
```bash
# 1. Verify agent is running
ps aux | grep agent2389

# 2. Check MQTT subscriptions
./scripts/mqtt-monitor.sh test-agent-rust

# 3. Verify task format
echo "$TASK_JSON" | jq .  # Should be valid JSON

# 4. Check agent logs
cargo run -- --config config/test-agent.toml run 2>&1 | grep ERROR
```

**Common causes**:
- Invalid JSON in task envelope
- Missing required task fields
- Agent ID mismatch
- Tool not available
- LLM API key missing/invalid

#### 3. Tool Execution Failures

**Symptom**: Tasks received but tool execution fails

**Diagnosis**:
```bash
# Check tool configuration
grep -A 10 "\[\[tools\]\]" config/test-agent.toml

# Test tool execution manually
echo "test message"  # Should work for echo tool
ping -c 1 google.com # Should work for ping tool
```

**Solutions**:
- Verify tool is configured in `test-agent.toml`
- Check tool schema validation
- Ensure required system commands available
- Review tool timeout settings

#### 4. High Latency or Timeouts

**Symptom**: Slow response times or task timeouts

**Diagnosis**:
```bash
# Check network latency to MQTT broker
ping mqtt.2389.dev

# Monitor system resources
top -p $(pgrep agent2389)

# Check LLM API response times
curl -w "@curl-format.txt" -s -o /dev/null https://api.openai.com/
```

**Solutions**:
- Increase task timeout in configuration
- Check LLM API rate limits
- Verify network connectivity
- Monitor system resource usage

### Debug Mode

Enable detailed logging for troubleshooting:

```bash
# Set debug logging level
export RUST_LOG=debug

# Run agent with verbose output
cargo run -- --config config/test-agent.toml run
```

### Log Analysis

Key log patterns to search for:

```bash
# MQTT connection status
grep -i "mqtt.*connect" agent.log

# Task processing
grep -i "processing task" agent.log

# Tool execution
grep -i "executing tool" agent.log

# Error conditions
grep -i "error\|fail\|panic" agent.log
```

## Test Environment Configuration

### Development Environment Setup

#### Full Development Setup

```bash
# Clone repository
git clone https://github.com/2389-research/2389-agent-rust.git
cd 2389-agent-rust

# Install Rust (if needed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install MQTT client tools
# Ubuntu/Debian:
sudo apt-get update
sudo apt-get install mosquitto-clients curl jq

# macOS:
brew install mosquitto curl jq

# Set up environment variables
echo "export OPENAI_API_KEY=your-key-here" >> ~/.bashrc
echo "export ANTHROPIC_API_KEY=your-key-here" >> ~/.bashrc
source ~/.bashrc

# Build project
cargo build --release

# Run initial test
./scripts/test-agent-startup.sh
```

#### CI/CD Testing

For automated testing environments:

```yaml
# .github/workflows/test.yml example
- name: Install MQTT tools
  run: sudo apt-get update && sudo apt-get install -y mosquitto-clients

- name: Run integration tests
  run: |
    ./scripts/test-agent-startup.sh
    timeout 60 ./scripts/mqtt-monitor.sh &
    sleep 5
    ./scripts/send-test-task.sh
  env:
    OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}
```

### Custom Test Configurations

#### Creating Custom Test Agents

Copy and modify `config/test-agent.toml`:

```bash
# Create custom configuration
cp config/test-agent.toml config/my-test-agent.toml

# Edit agent ID and settings
sed -i 's/test-agent-rust/my-test-agent/' config/my-test-agent.toml

# Run with custom config
cargo run -- --config config/my-test-agent.toml run
```

#### Testing Against Local MQTT Broker

For offline development:

```bash
# Start local MQTT broker
docker run -it -p 1883:1883 eclipse-mosquitto:2.0

# Update configuration for local broker
[mqtt]
broker_url = "mqtt://localhost:1883"
# Remove username/password for local testing
```

### Test Data Management

#### Persistent Test Data

Create reusable test scenarios:

```bash
# Create test scenarios directory
mkdir -p tests/scenarios

# Create scenario files
cat > tests/scenarios/basic-echo.json <<EOF
{
  "task_id": "basic-echo-test",
  "conversation_id": "test-conversation",
  "agent_id": "test-agent-rust",
  "content": "Echo: Hello, World!",
  "tools_available": ["echo"],
  "pipeline_depth": 1
}
EOF

# Send scenario
mosquitto_pub ... -m "$(cat tests/scenarios/basic-echo.json)"
```

#### Test Result Collection

Automated test result collection:

```bash
#!/bin/bash
# tests/collect-results.sh

TEST_RUN_ID="test-$(date +%Y%m%d-%H%M%S)"
RESULTS_DIR="test-results/$TEST_RUN_ID"
mkdir -p "$RESULTS_DIR"

# Capture agent logs
cargo run -- --config config/test-agent.toml run \
  > "$RESULTS_DIR/agent.log" 2>&1 &

# Capture MQTT traffic
./scripts/mqtt-monitor.sh > "$RESULTS_DIR/mqtt.log" 2>&1 &

# Run test scenarios
for scenario in tests/scenarios/*.json; do
  echo "Running scenario: $scenario"
  # Send task and collect results
done

# Generate test report
echo "Test Results for $TEST_RUN_ID" > "$RESULTS_DIR/summary.txt"
echo "================================" >> "$RESULTS_DIR/summary.txt"
# Add test metrics and pass/fail status
```

---

## See Also

### Getting Started with Testing

- **[Getting Started Guide](GETTING_STARTED.md)** - Initial setup before testing
- **[CLI Tools Reference](CLI_TOOLS.md)** - inject-message, mqtt-monitor, and testing tools
- **[Configuration Reference](CONFIGURATION_REFERENCE.md)** - Test agent configuration

### Testing Requirements

- **[TaskEnvelope Protocol](TASKENVELOPE_PROTOCOL.md)** - Protocol compliance requirements to test
- **[Agent Capabilities](AGENT_CAPABILITIES.md)** - Capability system behavior to validate
- **[Architecture Overview](ARCHITECTURE.md)** - System components and test boundaries

### Production Testing

- **[Deployment Guide](DEPLOYMENT.md)** - Pre-production validation procedures
- **[Observability Guide](OBSERVABILITY.md)** - Test monitoring and metrics
- **[Troubleshooting Guide](TROUBLESHOOTING.md)** - Debugging test failures

---

This comprehensive testing guide provides all the tools and procedures needed to thoroughly test the 2389 Agent Protocol implementation. Follow the quick start for immediate validation, then use the detailed procedures for thorough protocol compliance verification and performance testing.

For additional support or to report testing issues, check the project README and issue tracker.