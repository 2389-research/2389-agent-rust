# 2389 Agent Protocol - Examples

This directory contains practical, runnable examples demonstrating the 2389 Agent Protocol implementation.

## Quick Reference

| Example | Difficulty | Time | Description |
|---------|-----------|------|-------------|
| [01-echo-agent](#01-echo-agent) | Beginner | 5 min | Simplest possible agent with echo tool |
| [02-http-agent](#02-http-agent) | Beginner | 10 min | HTTP request capabilities |
| [03-file-agent](#03-file-agent) | Beginner | 10 min | File read/write operations |
| [04-pipeline](#04-pipeline) | Intermediate | 15 min | Multi-agent research-write-edit pipeline |
| [05-monitoring](#05-monitoring) | Intermediate | 10 min | Real-time monitoring with mqtt-monitor |
| [06-dynamic-routing](#06-dynamic-routing) | Advanced | 20 min | v2.0 capability-based routing |

## Prerequisites

Before running examples, ensure you have:

```bash
# Build the project
cargo build --release

# Set environment variables
export MQTT_USERNAME="robots"
export MQTT_PASSWORD="Hat-Compass-Question-Remove4-Shirt"
export ANTHROPIC_API_KEY="your-api-key-here"
# or
export OPENAI_API_KEY="your-api-key-here"
```

## 01: Echo Agent

**Directory**: `01-echo-agent/`
**Time**: 5 minutes
**Level**: Beginner

The simplest possible agent that echoes messages back. Perfect for verifying your setup works.

### Files
- `echo-agent.toml` - Minimal agent configuration
- `run.sh` - Start the agent
- `test.sh` - Send a test message

### Run It

```bash
cd 01-echo-agent

# Terminal 1: Start agent
./run.sh

# Terminal 2: Send test message
./test.sh
```

### What You'll Learn
- Basic agent configuration structure
- MQTT broker connection
- Simple tool usage
- Task injection basics

### Expected Output

```
Agent started and waiting for tasks...
Received task: "Echo this message: Hello, World!"
Calling echo tool...
Response sent to conversation topic
```

---

## 02: HTTP Agent

**Directory**: `02-http-agent/`
**Time**: 10 minutes
**Level**: Beginner

Agent with HTTP request capabilities for fetching web content.

### Files
- `http-agent.toml` - HTTP-enabled agent configuration
- `run.sh` - Start the agent
- `test-github-api.sh` - Test with GitHub API
- `test-webpage.sh` - Test with web page fetch

### Run It

```bash
cd 02-http-agent

# Terminal 1: Start agent
./run.sh

# Terminal 2: Test HTTP requests
./test-github-api.sh
./test-webpage.sh
```

### What You'll Learn
- Configuring built-in tools
- Tool configuration parameters
- Response size limits
- Content extraction

### Configuration Highlights

```toml
[[tools]]
name = "http_request"
implementation = "builtin"

[tools.config]
max_response_size = 1048576  # 1MB
timeout = 30
extract_content = true
```

---

## 03: File Agent

**Directory**: `03-file-agent/`
**Time**: 10 minutes
**Level**: Beginner

Agent with file system access (read/write in `/tmp` directory).

### Files
- `file-agent.toml` - File-enabled agent configuration
- `run.sh` - Start the agent
- `test-write.sh` - Test file writing
- `test-read.sh` - Test file reading
- `test-both.sh` - Test write then read

### Run It

```bash
cd 03-file-agent

# Terminal 1: Start agent
./run.sh

# Terminal 2: Test file operations
./test-both.sh
```

### What You'll Learn
- File operation security (allowed_paths)
- File size limits
- Read/write tool usage
- Path restrictions

### Security Note

The agent can ONLY access files in `/tmp` directory. This is configured via:

```toml
[tools.config]
max_file_size = 10485760  # 10MB
allowed_paths = ["/tmp"]
```

---

## 04: Pipeline

**Directory**: `04-pipeline/`
**Time**: 15 minutes
**Level**: Intermediate

Multi-agent pipeline: researcher → writer → editor

### Files
- `researcher-agent.toml` - Research specialist
- `writer-agent.toml` - Writing specialist
- `editor-agent.toml` - Editing specialist
- `run-all.sh` - Start all three agents
- `test-pipeline.sh` - Send pipeline task
- `monitor-pipeline.sh` - Watch pipeline execution

### Run It

```bash
cd 04-pipeline

# Terminal 1: Start all agents
./run-all.sh

# Terminal 2: Monitor traffic
./monitor-pipeline.sh

# Terminal 3: Start pipeline
./test-pipeline.sh "Rust async programming"
```

### What You'll Learn
- Multi-agent coordination
- Pipeline task structure
- Agent capabilities
- Task forwarding
- Agent-to-agent communication

### Pipeline Flow

```
User → researcher (capability: research)
       ↓ output becomes input
       writer (capability: writing)
       ↓ output becomes input
       editor (capability: editing)
       ↓ final output
       User
```

---

## 05: Monitoring

**Directory**: `05-monitoring/`
**Time**: 10 minutes
**Level**: Intermediate

Real-time monitoring and debugging with mqtt-monitor.

### Files
- `test-agent.toml` - Agent to monitor
- `run-agent.sh` - Start test agent
- `monitor-all.sh` - Monitor all traffic
- `monitor-conversations.sh` - Monitor just conversations
- `monitor-inputs.sh` - Monitor just inputs
- `monitor-availability.sh` - Monitor agent status
- `test-scenario.sh` - Generate test traffic

### Run It

```bash
cd 05-monitoring

# Terminal 1: Start agent
./run-agent.sh

# Terminal 2: Monitor (choose one)
./monitor-all.sh               # See everything
./monitor-conversations.sh     # Just agent communication
./monitor-inputs.sh            # Just incoming tasks
./monitor-availability.sh      # Just status updates

# Terminal 3: Generate traffic
./test-scenario.sh
```

### What You'll Learn
- mqtt-monitor tool modes
- Output format options (pretty, compact, JSON)
- Traffic filtering
- Real-time debugging
- Message type identification

### Monitoring Modes

```bash
# Pretty mode (default) - Color coded, formatted
mqtt-monitor --mode all

# Compact mode - One line per message
mqtt-monitor --mode all --format compact

# JSON mode - Raw messages
mqtt-monitor --mode all --format json
```

---

## 06: Dynamic Routing

**Directory**: `06-dynamic-routing/`
**Time**: 20 minutes
**Level**: Advanced

**Protocol Version**: v2.0 (80% complete)

Smart agent discovery and capability-based routing.

### Files
- `email-agent.toml` - Email handling specialist
- `calendar-agent.toml` - Calendar specialist
- `analytics-agent.toml` - Analytics specialist
- `run-all.sh` - Start all agents
- `test-discovery.sh` - Test agent discovery
- `test-routing.sh` - Test smart routing
- `monitor-routing.sh` - Watch routing decisions

### Run It

```bash
cd 06-dynamic-routing

# Terminal 1: Start all agents
./run-all.sh

# Terminal 2: Monitor routing
./monitor-routing.sh

# Terminal 3: Test discovery & routing
./test-discovery.sh
./test-routing.sh "Send email to john@example.com"
./test-routing.sh "Schedule meeting tomorrow 2pm"
./test-routing.sh "Show sales analytics for Q3"
```

### What You'll Learn
- v2.0 TaskEnvelope format
- Agent discovery process
- Capability matching
- Smart routing rules
- Load-based selection
- JSONPath routing conditions

### Dynamic Routing Flow

```
1. Query: "Send email to john@example.com"
   ↓
2. Discover all available agents (5 second timeout)
   ↓
3. Analyze query → detect "email" intent
   ↓
4. Match agents with "email" capability
   ↓
5. Generate routing rules with priority
   ↓
6. Select best agent based on load
   ↓
7. Send TaskEnvelope v2.0 with routing config
```

---

## Common Patterns

### Pattern: Health Check

Check if an agent is running:

```bash
# Using curl
curl http://localhost:8080/health

# Using MQTT monitor
mqtt-monitor --mode availability --agent-id my-agent
```

### Pattern: Debug Pipeline

When pipeline doesn't work as expected:

```bash
# Terminal 1: Monitor all conversations
mqtt-monitor --mode conversations

# Terminal 2: Monitor each agent's input
mqtt-monitor --mode inputs --agent-id researcher
mqtt-monitor --mode inputs --agent-id writer
mqtt-monitor --mode inputs --agent-id editor

# Check for broken links in the chain
```

### Pattern: Load Testing

Test agent performance under load:

```bash
# Start agent
./run.sh &

# Send multiple tasks in parallel
for i in {1..10}; do
  inject-message --agent-id test-agent --message "Task $i" &
done

# Monitor processing
mqtt-monitor --mode all --format compact
```

### Pattern: Configuration Validation

Validate configuration before deployment:

```bash
# Attempt to start agent (will fail if config invalid)
cargo run --release -- --config my-agent.toml run --dry-run

# Check specific fields
cargo run --bin validate-config -- my-agent.toml
```

---

## Troubleshooting Examples

### Can't Connect to MQTT Broker

```bash
# Test broker connectivity
telnet mqtt.2389.dev 8883

# Check credentials
echo $MQTT_USERNAME
echo $MQTT_PASSWORD

# Verify in config
grep -A 2 "\[mqtt\]" examples/01-echo-agent/echo-agent.toml
```

### Agent Starts But Doesn't Respond

```bash
# Check agent logs
cargo run -- --config agent.toml run 2>&1 | tee agent.log

# Verify agent ID matches
grep "agent_id" agent.toml
# When injecting, use same ID:
inject-message --agent-id <same-id-here> --message "test"

# Monitor MQTT to see traffic
mqtt-monitor --mode all --agent-id <agent-id>
```

### Pipeline Breaks

```bash
# Check each agent is running
curl http://localhost:8080/health  # researcher
curl http://localhost:8081/health  # writer
curl http://localhost:8082/health  # editor

# Verify agent IDs in pipeline match configs
grep "id =" 04-pipeline/*.toml

# Watch pipeline execution
mqtt-monitor --mode conversations
```

---

## Next Steps

After working through these examples:

1. **Customize Configurations**: Modify agent system prompts and tools
2. **Create Custom Agents**: Build agents for your specific use cases
3. **Deploy to Production**: See [Deployment Guide](../docs/DEPLOYMENT.md)
4. **Add Monitoring**: See [Observability Guide](../docs/OBSERVABILITY.md)
5. **Build Custom Tools**: Extend agent capabilities

## See Also

### Documentation
- **[Getting Started Guide](../docs/GETTING_STARTED.md)** - Comprehensive setup guide
- **[Configuration Reference](../docs/CONFIGURATION_REFERENCE.md)** - All configuration options
- **[CLI Tools Reference](../docs/CLI_TOOLS.md)** - Command-line tools
- **[Troubleshooting Guide](../docs/TROUBLESHOOTING.md)** - Common issues

### Architecture
- **[Architecture Overview](../docs/ARCHITECTURE.md)** - System design
- **[TaskEnvelope Protocol](../docs/TASKENVELOPE_PROTOCOL.md)** - Protocol specification
- **[Agent Capabilities](../docs/AGENT_CAPABILITIES.md)** - Capability system

### Operations
- **[Deployment Guide](../docs/DEPLOYMENT.md)** - Production deployment
- **[Observability Guide](../docs/OBSERVABILITY.md)** - Monitoring and metrics
- **[Testing Guide](../docs/TESTING.md)** - Testing procedures

---

**Note**: These examples use the test MQTT broker at `mqtt.2389.dev:8883` with shared test credentials. For production use, deploy your own MQTT broker and use secure, unique credentials.

Happy agent building! 🚀