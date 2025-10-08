# CLI Tools Reference

Complete reference for the 2389 Agent Protocol CLI utilities for testing, monitoring, and interacting with agents.

## Table of Contents

- [Overview](#overview)
- [mqtt-monitor](#mqtt-monitor)
- [inject-message](#inject-message)
- [pipeline-injector](#pipeline-injector)
- [dynamic-injector](#dynamic-injector)
- [Common Patterns](#common-patterns)
- [Examples](#examples)
- [Troubleshooting](#troubleshooting)

## Overview

The 2389 Agent Protocol provides four command-line tools for different aspects of agent interaction:

| Tool | Purpose | Use Case |
|------|---------|----------|
| **mqtt-monitor** | Real-time MQTT traffic monitoring | Debugging, observing agent communication |
| **inject-message** | Simple message injection | Quick testing, experimentation |
| **pipeline-injector** | Multi-agent pipeline testing | Testing agent chains, workflow validation |
| **dynamic-injector** | Smart agent discovery and routing | v2.0 dynamic routing testing, capability-based routing |

All tools are built with the project and available in `target/release/` after running `cargo build --release`.

## mqtt-monitor

Real-time monitoring tool for observing MQTT traffic with syntax-highlighted JSON output.

### Synopsis

```bash
mqtt-monitor [OPTIONS]
```

### Description

The MQTT monitor subscribes to agent communication channels and displays messages in real-time with color-coded labels and JSON syntax highlighting. It supports multiple monitoring modes for focused observation of specific traffic types.

### Options

#### Core Options

- `--mode <MODE>` - Monitoring mode (default: `all`)
  - `all` - Monitor all traffic for the specified agent
  - `availability` - Agent status and availability updates only
  - `conversations` - Conversation messages between agents
  - `inputs` - Agent input messages (incoming tasks)
  - `progress` - Progress reporting from agents

- `--format <FORMAT>` - Output format (default: `pretty`)
  - `pretty` - Color-coded with syntax-highlighted JSON
  - `compact` - Single line per message, minimal formatting
  - `json` - Raw JSON for programmatic processing

- `--agent-id <ID>` - Agent ID to monitor (default: `dev-agent`)

#### Connection Options

- `--broker-host <HOST>` - MQTT broker hostname (default: `localhost`)
- `--broker-port <PORT>` - MQTT broker port (default: `1883`)
- `--username <USERNAME>` - MQTT username (optional)
- `--password <PASSWORD>` - MQTT password (optional)

#### Filtering Options

- `--conversation-id <ID>` - Filter by specific conversation (conversations mode only)
- `--filter <TYPE>` - Legacy filter by message type

### Message Types

The monitor displays messages with color-coded labels:

- **AGENT_STATUS** (Cyan) - Agent health and status updates
- **INPUT** (Blue) - Incoming task requests to agents
- **CONVERSATION** (Green) - Agent-to-agent communication
- **ERROR** (Red) - Error messages
- **PROGRESS** (Bright Yellow) - Progress reporting
- **BROADCAST** (Magenta) - System-wide broadcasts
- **STATUS** (Yellow) - General status messages

### Examples

#### Monitor All Traffic

```bash
# Monitor all traffic for dev-agent
mqtt-monitor --agent-id dev-agent

# Monitor with custom broker
mqtt-monitor --agent-id prod-agent \
  --broker-host mqtt.example.com \
  --broker-port 8883 \
  --username robots \
  --password "your-password"
```

#### Focused Monitoring

```bash
# Monitor only agent availability
mqtt-monitor --mode availability

# Monitor specific conversation
mqtt-monitor --mode conversations \
  --conversation-id "project-alpha-session"

# Monitor agent inputs (incoming tasks)
mqtt-monitor --mode inputs

# Monitor progress reporting
mqtt-monitor --mode progress
```

#### Output Formats

```bash
# Pretty format with colors (default)
mqtt-monitor --format pretty

# Compact format for logs
mqtt-monitor --format compact > agent-traffic.log

# JSON format for processing
mqtt-monitor --format json | jq '.payload.task_id'
```

### Output Example

Pretty format output:

```
2389 Agent Protocol - MQTT Monitor
==================================
Mode: All
Format: Pretty
Agent ID: dev-agent
MQTT Broker: localhost:1883

Monitoring ALL traffic for agent: dev-agent
  - Agent status, errors, inputs, and conversations
  - General broadcast messages

✅ Connected to MQTT broker
✅ Successfully subscribed to topics

[AGENT_STATUS] 14:32:15 /control/agents/dev-agent/status
{
  "health": "ok",
  "load": 0.25,
  "uptime_seconds": 3600
}

[INPUT] 14:32:20 /control/agents/dev-agent/input
{
  "task_id": "550e8400-e29b-41d4-a716-446655440000",
  "instruction": "Process this message",
  "input": {
    "message": "Hello from client"
  }
}

[CONVERSATION] 14:32:22 /conversations/conv-123/dev-agent
{
  "response": "Message processed successfully",
  "task_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

### Features

- **Automatic Reconnection** - Reconnects with exponential backoff if connection drops
- **JSON Syntax Highlighting** - Color-coded keys, strings, numbers, and booleans
- **Graceful Shutdown** - Clean disconnect on Ctrl+C
- **Multiple Subscriptions** - Monitors all relevant topics simultaneously
- **Mode-Based Filtering** - Focus on specific message types

### Use Cases

- **Development** - Watch agent communication in real-time during development
- **Debugging** - Observe message flow when troubleshooting issues
- **Testing** - Verify agents are receiving and sending correct messages
- **Monitoring** - Keep track of agent health and activity

---

## inject-message

Simple tool for injecting test messages into running agents.

### Synopsis

```bash
inject-message --agent-id <AGENT_ID> --message <MESSAGE> [OPTIONS]
```

### Description

Inject-message creates properly formatted TaskEnvelope messages and sends them to agent input topics. Perfect for quick testing and experimentation without writing custom scripts.

### Options

#### Required

- `--agent-id <ID>` - Target agent ID to send message to
- `--message <MESSAGE>` - Message content to send

#### Optional

- `--conversation-id <ID>` - Conversation ID (auto-generated if not provided)
- `--tool <TOOL>` - Tool name to request execution
- `--tool-params <JSON>` - Tool parameters as JSON string
- `--next-agent <AGENT>` - Next agent in pipeline for task forwarding
- `--broker-url <URL>` - MQTT broker hostname (default: `localhost`)
- `--broker-port <PORT>` - MQTT broker port (default: `1883`)

### Examples

#### Basic Message

```bash
# Simple message to agent
inject-message \
  --agent-id dev-agent \
  --message "Hello, please introduce yourself"

# With custom conversation
inject-message \
  --agent-id dev-agent \
  --conversation-id "experiment-001" \
  --message "Start processing task A"
```

#### Tool Execution Request

```bash
# Request tool execution with parameters
inject-message \
  --agent-id dev-agent \
  --message "Fetch weather data" \
  --tool http_request \
  --tool-params '{"url": "https://api.weather.com/current", "method": "GET"}'

# File operation request
inject-message \
  --agent-id file-agent \
  --message "Read configuration" \
  --tool file_read \
  --tool-params '{"path": "/tmp/config.json"}'
```

#### Pipeline Chaining

```bash
# Forward to next agent after processing
inject-message \
  --agent-id researcher-agent \
  --message "Research topic: Rust async programming" \
  --next-agent writer-agent

# Multi-hop pipeline
inject-message \
  --agent-id intake-agent \
  --message "Process customer request #12345" \
  --next-agent validation-agent
```

#### Custom Broker

```bash
# Connect to remote broker
inject-message \
  --agent-id prod-agent \
  --message "Production test" \
  --broker-url mqtt.example.com \
  --broker-port 8883
```

### Output Example

```
Connecting to MQTT broker localhost:1883...

📤 Injecting message to /control/agents/dev-agent/input
   Conversation: experiment-1703123456
   Task ID: 550e8400-e29b-41d4-a716-446655440000
   Message: Hello, please introduce yourself
✓ Message injected successfully

💡 Monitor agent responses at:
   /conversations/experiment-1703123456/dev-agent

   Use: cargo run --bin mqtt-monitor -- --conversation experiment-1703123456
```

### Generated TaskEnvelope

The tool creates a properly formatted TaskEnvelope:

```json
{
  "task_id": "550e8400-e29b-41d4-a716-446655440000",
  "conversation_id": "experiment-1703123456",
  "topic": "/control/agents/dev-agent/input",
  "instruction": "Process this message: Hello, please introduce yourself",
  "input": {
    "message": "Hello, please introduce yourself"
  }
}
```

With tool request:

```json
{
  "task_id": "...",
  "conversation_id": "...",
  "topic": "/control/agents/dev-agent/input",
  "instruction": "Process this message: Fetch data",
  "input": {
    "message": "Fetch data",
    "tool_request": {
      "name": "http_request",
      "parameters": {
        "url": "https://api.example.com/data",
        "method": "GET"
      }
    }
  }
}
```

### Use Cases

- **Quick Testing** - Test agent responses without writing code
- **Experimentation** - Try different messages and parameters
- **Tool Testing** - Verify tool execution works correctly
- **Pipeline Testing** - Test agent forwarding and chaining

---

## pipeline-injector

Tool for creating complete multi-agent pipeline tasks with nested structure.

### Synopsis

```bash
pipeline-injector --topic <TOPIC> [OPTIONS]
```

### Description

Pipeline-injector creates properly structured pipeline tasks that flow through multiple agents in sequence. Each agent receives the previous agent's output and forwards to the next agent. Perfect for testing multi-agent workflows.

### Options

- `--topic <TOPIC>` - Topic to research/process (required)
- `--conversation-id <ID>` - Conversation ID (auto-generated if not provided)
- `--broker-url <URL>` - MQTT broker hostname (default: `localhost`)
- `--broker-port <PORT>` - MQTT broker port (default: `1883`)
- `--researcher-id <ID>` - First agent ID (default: `researcher-agent`)
- `--writer-id <ID>` - Second agent ID (default: `writer-agent`)
- `--editor-id <ID>` - Third agent ID (default: `editor-agent`)

### Pipeline Flow

The tool creates a 3-stage pipeline:

1. **Researcher** - Gathers information and creates research brief
2. **Writer** - Transforms research into comprehensive document
3. **Editor** - Polishes and finalizes the document

### Examples

#### Basic Pipeline

```bash
# Create pipeline with default agents
pipeline-injector --topic "Rust async programming"

# With custom conversation
pipeline-injector \
  --topic "Machine learning basics" \
  --conversation-id "ml-project-001"
```

#### Custom Agent Configuration

```bash
# Specify custom agent IDs
pipeline-injector \
  --topic "Climate change solutions" \
  --researcher-id data-collector \
  --writer-id report-generator \
  --editor-id quality-checker
```

#### Remote Broker

```bash
# Use production broker
pipeline-injector \
  --topic "Q4 Financial Report" \
  --broker-url mqtt.prod.example.com \
  --broker-port 8883 \
  --researcher-id prod-researcher \
  --writer-id prod-writer \
  --editor-id prod-editor
```

### Output Example

```
Connecting to MQTT broker localhost:1883...

🚀 Injecting Pipeline Task
   Topic: Rust async programming
   Conversation: conv-550e8400-e29b-41d4-a716-446655440000
   Task ID: 123e4567-e89b-12d3-a456-426614174000

📋 Pipeline Flow:
   1. researcher-agent → Research and gather information
   2. writer-agent → Create comprehensive first draft
   3. editor-agent → Edit and finalize document

📤 Publishing to: /control/agents/researcher-agent/input
✅ Pipeline task injected successfully!

💡 Monitor pipeline progress:
   Researcher: curl http://localhost:8080/health
   Writer:     curl http://localhost:8081/health
   Editor:     curl http://localhost:8082/health

   Conversation topic: /conversations/conv-550e8400.../researcher-agent
```

### Generated Structure

The pipeline creates nested TaskEnvelope structures:

```json
{
  "task_id": "123e4567-e89b-12d3-a456-426614174000",
  "conversation_id": "conv-550e8400-e29b-41d4-a716-446655440000",
  "topic": "/control/agents/researcher-agent/input",
  "instruction": "Research the topic 'Rust async programming' thoroughly...",
  "input": {
    "role": "researcher",
    "task": "research_and_brief",
    "topic": "Rust async programming"
  },
  "next": {
    "topic": "/control/agents/writer-agent/input",
    "instruction": "Write a comprehensive document...",
    "input": {
      "role": "writer",
      "task": "draft_creation"
    },
    "next": {
      "topic": "/control/agents/editor-agent/input",
      "instruction": "Edit and finalize the document...",
      "input": {
        "role": "editor",
        "task": "final_editing"
      },
      "next": null
    }
  }
}
```

### Use Cases

- **Workflow Testing** - Verify multi-agent pipelines work correctly
- **Integration Testing** - Test agent-to-agent communication
- **Performance Testing** - Measure pipeline throughput
- **Demonstration** - Show agent collaboration capabilities

---

## dynamic-injector

Smart agent discovery and dynamic routing tool for v2.0 protocol.

### Synopsis

```bash
dynamic-injector --query <QUERY> [OPTIONS]
```

### Description

Dynamic-injector discovers available agents via MQTT status messages, analyzes their capabilities, and generates optimized TaskEnvelope v2.0 messages with intelligent routing configuration. It selects the best agent for the query and creates conditional routing rules.

### Options

- `--query <QUERY>` - User query to process (required)
- `--conversation-id <ID>` - Conversation ID (auto-generated if not provided)
- `--mqtt-broker <URL>` - MQTT broker URL (default: `mqtt://localhost:1883`)
- `--discovery-timeout <SECS>` - Agent discovery timeout in seconds (default: `5`)
- `--preview-only` - Preview the generated envelope without sending
- `--target-agent <ID>` - Skip smart routing and use specific agent
- `-v, --verbose` - Verbose output for debugging

### Features

- **Real-time Agent Discovery** - Subscribes to `/control/agents/+/status`
- **Capability Analysis** - Matches query to agent capabilities
- **Load-Based Selection** - Chooses least-loaded healthy agent
- **Smart Routing Rules** - Generates JSONPath conditions for dynamic routing
- **v2.0 Protocol** - Creates TaskEnvelope v2.0 with routing configuration
- **Preview Mode** - Review envelope before sending

### Examples

#### Smart Query Processing

```bash
# Let the tool discover and route
dynamic-injector --query "Process this urgent customer email about billing"

# Preview without sending
dynamic-injector \
  --query "Analyze this financial data" \
  --preview-only

# With custom conversation
dynamic-injector \
  --query "Schedule a team meeting for next week" \
  --conversation-id "team-planning-session"
```

#### Discovery Configuration

```bash
# Longer discovery timeout for slow networks
dynamic-injector \
  --query "Help me debug this code" \
  --discovery-timeout 10

# Verbose output for debugging
dynamic-injector \
  --query "Process customer request" \
  --verbose
```

#### Specific Agent

```bash
# Skip discovery and use specific agent
dynamic-injector \
  --query "Custom processing task" \
  --target-agent specialized-agent
```

### Output Example

```
🚀 Dynamic Message Injector v2.0
🎯 Query: "Process this urgent customer email about billing"

🔍 Discovering available agents...
📡 Subscribed to agent status channels
⏳ Waiting 5 seconds for agent discovery...

✅ Agent discovery completed
📊 Found 3 agents

🤖 Discovered Agents:
================================================================================
Agent: customer-service-agent ✅
  Description: Handles customer support requests
  Load: 25.0% [█████░░░░░░░░░░░░░░░] 25%
  Capabilities: customer_support, email, urgent_processing
  Last Updated: 2024-01-01T14:30:00Z

Agent: general-agent ✅
  Description: General purpose agent
  Load: 50.0% [██████████░░░░░░░░░░] 50%
  Capabilities: general
  Last Updated: 2024-01-01T14:30:05Z

Agent: analytics-agent ✅
  Description: Data analysis and reporting
  Load: 15.0% [███░░░░░░░░░░░░░░░░░] 15%
  Capabilities: data_analysis, analytics
  Last Updated: 2024-01-01T14:30:10Z

🧠 Analyzing query: "Process this urgent customer email about billing"
🎯 Generating smart routing rules...

🔍 Query Analysis Results:
  Primary capability needed: urgent_processing
  Best available agent: customer-service-agent (load: 25.0%)
  Generated 2 routing rules

📋 Generated TaskEnvelope v2.0:
================================================================================
🆔 Task ID: 123e4567-e89b-12d3-a456-426614174000
💬 Conversation: dynamic-injector-session
📍 Target Topic: /control/agents/customer-service-agent/input
📝 Instruction: Process this user request: Process this urgent customer email...

📊 Input Data:
{
  "user_query": "Process this urgent customer email about billing",
  "type": "customer_service",
  "timestamp": "2024-01-01T14:30:15Z",
  "injector_version": "2.0.0",
  "discovery_context": {
    "available_agents": 3,
    "discovery_timeout": 5
  }
}

🛣️  Routing Configuration:
  Mode: dynamic
  Fallback: general
  Rules (2 total):
    1. Priority: 100 | Condition: $.urgency_score >= 0.8 | Target: customer-service-agent
    2. Priority: 85 | Condition: $.type == "customer_service" | Target: customer-service-agent

🔄 Sending TaskEnvelope...
✅ TaskEnvelope sent to: /control/agents/customer-service-agent/input
📤 Message size: 892 bytes

🎉 Dynamic injection completed successfully!
```

### Query Type Detection

The tool analyzes queries and generates appropriate routing rules:

| Keywords | Type | Primary Capability |
|----------|------|-------------------|
| urgent, emergency, asap | urgent | `urgent_processing` |
| email, mail, inbox | email | `email` |
| schedule, meeting, calendar | calendar | `calendar` |
| analyze, data, statistics | analytics | `data_analysis` |
| code, debug, programming | development | `code_processing` |
| customer, support, complaint | customer service | `customer_support` |
| (none of above) | general | `general` |

### Generated Routing Rules

The tool creates JSONPath-based routing rules:

```json
{
  "routing": {
    "mode": "dynamic",
    "fallback": "general",
    "rules": [
      {
        "priority": 100,
        "condition": "$.urgency_score >= 0.8",
        "target_agent": "urgent-processor"
      },
      {
        "priority": 85,
        "condition": "$.type == \"customer_service\"",
        "target_agent": "customer-service-agent"
      }
    ]
  }
}
```

### Use Cases

- **v2.0 Protocol Testing** - Test dynamic routing functionality
- **Agent Discovery** - See which agents are available
- **Capability Mapping** - Understand agent capabilities
- **Load Balancing** - Test load-based agent selection
- **Routing Validation** - Verify routing rules work correctly

---

## Common Patterns

### Development Workflow

```bash
# Terminal 1: Monitor traffic
mqtt-monitor --agent-id dev-agent

# Terminal 2: Run agent
cargo run --release -- --config config/dev-agent.toml run

# Terminal 3: Inject test messages
inject-message --agent-id dev-agent --message "Test 1"
inject-message --agent-id dev-agent --message "Test 2"
```

### Pipeline Testing

```bash
# Terminal 1: Monitor first agent
mqtt-monitor --agent-id researcher-agent

# Terminal 2: Monitor second agent
mqtt-monitor --agent-id writer-agent

# Terminal 3: Monitor third agent
mqtt-monitor --agent-id editor-agent

# Terminal 4: Inject pipeline
pipeline-injector --topic "Test Topic"
```

### v2.0 Dynamic Routing

```bash
# Discover agents
dynamic-injector --query "Test query" --preview-only

# Send with routing
dynamic-injector --query "Process this urgent request"

# Monitor the selected agent
mqtt-monitor --agent-id <selected-agent>
```

## Examples

### Example 1: Basic Testing

Test agent with echo functionality:

```bash
# Start monitoring
mqtt-monitor --agent-id echo-agent &

# Send test message
inject-message \
  --agent-id echo-agent \
  --message "Echo: Hello World"

# Watch the response in monitor
```

### Example 2: Tool Execution

Test HTTP request tool:

```bash
# Monitor inputs
mqtt-monitor --agent-id web-agent --mode inputs &

# Request HTTP fetch
inject-message \
  --agent-id web-agent \
  --message "Fetch GitHub API" \
  --tool http_request \
  --tool-params '{"url":"https://api.github.com/zen","method":"GET"}'
```

### Example 3: Pipeline Workflow

Test research → write → edit pipeline:

```bash
# Monitor all agents
mqtt-monitor --mode conversations &

# Create pipeline
pipeline-injector \
  --topic "Distributed Systems" \
  --conversation-id "research-project-001"

# Check agent health
curl http://localhost:8080/health  # researcher
curl http://localhost:8081/health  # writer
curl http://localhost:8082/health  # editor
```

### Example 4: Dynamic Routing

Test v2.0 dynamic agent selection:

```bash
# Discover available agents
dynamic-injector \
  --query "Process customer complaint about billing" \
  --preview-only \
  --verbose

# Actually send with routing
dynamic-injector \
  --query "Process customer complaint about billing" \
  --conversation-id "customer-ticket-789"

# Monitor selected agent
mqtt-monitor --mode all
```

## Troubleshooting

### Connection Issues

**Problem**: Can't connect to MQTT broker

```bash
# Test broker connectivity
telnet localhost 1883

# Check if broker is running
docker ps | grep mosquitto

# Try with explicit credentials
mqtt-monitor \
  --broker-host localhost \
  --broker-port 1883 \
  --username test \
  --password test
```

### No Agents Discovered

**Problem**: dynamic-injector finds no agents

```bash
# Increase discovery timeout
dynamic-injector --query "test" --discovery-timeout 10 --verbose

# Verify agents are publishing status
mqtt-monitor --mode availability

# Check agent configuration
grep "status" config/*.toml
```

### Messages Not Received

**Problem**: inject-message sends but agent doesn't respond

```bash
# Verify topic format
mqtt-monitor --agent-id your-agent-id --mode inputs

# Check agent logs
docker logs your-agent

# Validate message format
inject-message --agent-id test-agent --message "test" --verbose
```

### Pipeline Breaks

**Problem**: Pipeline stops at first agent

```bash
# Monitor all stages
mqtt-monitor --mode conversations

# Check agent health
curl http://localhost:8080/health
curl http://localhost:8081/health
curl http://localhost:8082/health

# Verify agent IDs match
grep agent_id config/*.toml
```

### Tool Execution Fails

**Problem**: Tool request doesn't work

```bash
# Check tool is configured
grep -A 3 "tools" config/agent.toml

# Verify tool parameters are valid JSON
echo '{"url":"https://example.com"}' | jq .

# Test with simple tool first
inject-message --agent-id test-agent --message "test" --tool echo
```

---

## See Also

### Essential References

- **[Getting Started Guide](GETTING_STARTED.md)** - First-time setup and tutorial showing CLI tool usage
- **[Configuration Reference](CONFIGURATION_REFERENCE.md)** - Agent configuration details needed for testing
- **[Troubleshooting Guide](TROUBLESHOOTING.md)** - Common agent and connectivity issues
- **[Testing Guide](TESTING.md)** - Comprehensive testing procedures using CLI tools

### Protocol & Architecture

- **[TaskEnvelope Protocol](TASKENVELOPE_PROTOCOL.md)** - v1.0 and v2.0 message format specifications
- **[Architecture Overview](ARCHITECTURE.md)** - System design and MQTT communication patterns
- **[Agent Capabilities](AGENT_CAPABILITIES.md)** - Capability system used by dynamic-injector

### Deployment & Monitoring

- **[Deployment Guide](DEPLOYMENT.md)** - Production deployment procedures
- **[Observability Guide](OBSERVABILITY.md)** - Monitoring, metrics, and logging strategies