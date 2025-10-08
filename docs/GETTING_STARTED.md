# Getting Started with 2389 Agent Protocol

Welcome! This guide will have you running your first AI agent in 10 minutes.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Understanding What Just Happened](#understanding-what-just-happened)
- [Next Steps](#next-steps)
- [Tutorials](#tutorials)

## Prerequisites

### Required

- **Rust 1.75+** - Install from [rustup.rs](https://rustup.rs/)
- **MQTT Broker Access** - We'll use a test broker for this guide
- **LLM API Key** - OpenAI or Anthropic (optional for basic testing)

### Installation

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Clone the repository
git clone https://github.com/yourusername/2389-agent-rust.git
cd 2389-agent-rust

# Build the project
cargo build --release
```

## Quick Start

### Step 1: Create Your First Agent Configuration

Create a file named `my-agent.toml`:

```toml
[agent]
id = "my-first-agent"
description = "My first 2389 agent"

[mqtt]
broker_url = "mqtts://mqtt.2389.dev:8883"
username_env = "MQTT_USERNAME"
password_env = "MQTT_PASSWORD"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are a helpful AI assistant."

[llm.budget]
max_tool_calls = 15
max_iterations = 8

[[tools]]
name = "echo"
implementation = "builtin"
```

### Step 2: Set Environment Variables

```bash
# Test broker credentials (safe for testing only!)
export MQTT_USERNAME="robots"
export MQTT_PASSWORD="Hat-Compass-Question-Remove4-Shirt"

# Your LLM API key
export ANTHROPIC_API_KEY="your-api-key-here"
```

### Step 3: Start Your Agent

```bash
cargo run --release -- --config my-agent.toml run
```

You should see output like:

```
2025-09-29T10:00:00Z INFO agent2389: Starting agent: my-first-agent
2025-09-29T10:00:01Z INFO agent2389: Connected to MQTT broker
2025-09-29T10:00:01Z INFO agent2389: Subscribed to /control/agents/my-first-agent/input
2025-09-29T10:00:01Z INFO agent2389: Agent ready and waiting for tasks
```

### Step 4: Send a Test Task

In another terminal:

```bash
cargo run --bin inject-message -- \
  --agent-id my-first-agent \
  --message "Echo this message back to me: Hello, World!"
```

Check your agent terminal - you'll see it processing the task!

## Understanding What Just Happened

### The Agent Lifecycle

1. **Startup**: Agent connects to MQTT broker with credentials
2. **Availability**: Publishes status to `/control/agents/my-first-agent/status`
3. **Listening**: Subscribes to `/control/agents/my-first-agent/input`
4. **Processing**: Receives task, calls LLM, executes tools
5. **Response**: Publishes result to conversation topic

### Message Flow

```
You send task:                Your agent:
┌─────────────┐              ┌──────────────┐
│inject-message│             │ my-first-agent│
└──────┬───────┘              └──────┬───────┘
       │                             │
       │  TaskEnvelope               │
       │────────────────────────────>│
       │  /control/agents/.../input  │
       │                             │
       │                   ┌─────────┴────────┐
       │                   │ Process with LLM │
       │                   │ Execute tools    │
       │                   └─────────┬────────┘
       │                             │
       │           Response          │
       │<────────────────────────────│
       │  /conversations/{id}/...    │
```

### Configuration Breakdown

```toml
[agent]
id = "my-first-agent"              # Unique agent identifier
description = "..."                 # Human-readable description

[mqtt]
broker_url = "mqtts://..."         # MQTT broker (with TLS)
username_env = "MQTT_USERNAME"     # Env var for username
password_env = "MQTT_PASSWORD"     # Env var for password

[llm]
provider = "anthropic"             # LLM provider (anthropic/openai)
model = "claude-sonnet-4-..."      # Model name
api_key_env = "ANTHROPIC_API_KEY"  # Env var for API key
system_prompt = "..."              # System prompt for LLM

[llm.budget]
max_tool_calls = 15                # Prevent infinite loops
max_iterations = 8                 # Max back-and-forth with LLM

[[tools]]
name = "echo"                      # Tool name
implementation = "builtin"         # Built-in tool
```

## Next Steps

### Learn More About Configuration

- **[Configuration Reference](CONFIGURATION_REFERENCE.md)** - All configuration options
- **[Agent Capabilities](AGENT_CAPABILITIES.md)** - Understanding capabilities
- **[Task Injector Guide](TASK_INJECTOR_GUIDE.md)** - Advanced task injection

### Deploy Your Agent

- **[Deployment Guide](DEPLOYMENT.md)** - Docker and Kubernetes deployment
- **[Observability Guide](OBSERVABILITY.md)** - Monitoring and metrics

### Extend Functionality

- **[Custom Tools Guide](CUSTOM_TOOLS_GUIDE.md)** - Create your own tools (coming soon)
- **[Architecture Overview](ARCHITECTURE.md)** - Understand the system

## Tutorials

### Tutorial 1: Echo Agent (You Just Did This!)

The simplest possible agent that echoes messages back.

**Skills Learned:**
- Basic configuration
- Starting an agent
- Sending tasks

### Tutorial 2: HTTP Request Agent

Create an agent that can make HTTP requests.

**Configuration** (`http-agent.toml`):

```toml
[agent]
id = "http-agent"
description = "Agent that can make HTTP requests"

[mqtt]
broker_url = "mqtts://mqtt.2389.dev:8883"
username_env = "MQTT_USERNAME"
password_env = "MQTT_PASSWORD"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are an agent that can fetch web content. Use the http_request tool to retrieve information from URLs."

[llm.budget]
max_tool_calls = 15
max_iterations = 8

[[tools]]
name = "http_request"
implementation = "builtin"

[tools.config]
max_response_size = 1048576  # 1MB
timeout = 30
```

**Run it:**

```bash
cargo run --release -- --config http-agent.toml run
```

**Test it:**

```bash
cargo run --bin inject-message -- \
  --agent-id http-agent \
  --message "Fetch the content from https://api.github.com/zen"
```

**Skills Learned:**
- Adding tools
- Tool configuration
- Making HTTP requests

### Tutorial 3: File Operations Agent

Create an agent that can read and write files.

**Configuration** (`file-agent.toml`):

```toml
[agent]
id = "file-agent"
description = "Agent with file system access"

[mqtt]
broker_url = "mqtts://mqtt.2389.dev:8883"
username_env = "MQTT_USERNAME"
password_env = "MQTT_PASSWORD"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are an agent that can work with files. Use file_read and file_write tools carefully."

[llm.budget]
max_tool_calls = 15
max_iterations = 8

[[tools]]
name = "file_read"
implementation = "builtin"

[tools.config]
max_file_size = 10485760  # 10MB
allowed_paths = ["/tmp"]

[[tools]]
name = "file_write"
implementation = "builtin"

[tools.config]
max_file_size = 10485760  # 10MB
allowed_paths = ["/tmp"]
```

**Run it:**

```bash
cargo run --release -- --config file-agent.toml run
```

**Test it:**

```bash
cargo run --bin inject-message -- \
  --agent-id file-agent \
  --message "Write 'Hello from agent!' to /tmp/test.txt, then read it back"
```

**Skills Learned:**
- Multiple tools
- Tool restrictions (allowed_paths)
- File operations

### Tutorial 4: Multi-Agent Pipeline

Create a pipeline where one agent passes work to another.

**First Agent** (`researcher-agent.toml`):

```toml
[agent]
id = "researcher"
description = "Researches topics"
capabilities = ["research", "analysis"]

[mqtt]
broker_url = "mqtts://mqtt.2389.dev:8883"
username_env = "MQTT_USERNAME"
password_env = "MQTT_PASSWORD"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You research topics and provide summaries."

[llm.budget]
max_tool_calls = 25
max_iterations = 12

[[tools]]
name = "web_search"
implementation = "builtin"
```

**Second Agent** (`writer-agent.toml`):

```toml
[agent]
id = "writer"
description = "Writes content"
capabilities = ["writing", "editing"]

[mqtt]
broker_url = "mqtts://mqtt.2389.dev:8883"
username_env = "MQTT_USERNAME"
password_env = "MQTT_PASSWORD"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are a writer who creates polished content from research."

[llm.budget]
max_tool_calls = 15
max_iterations = 8

[[tools]]
name = "file_write"
implementation = "builtin"

[tools.config]
allowed_paths = ["/tmp"]
```

**Run both agents:**

```bash
# Terminal 1
cargo run --release -- --config researcher-agent.toml run

# Terminal 2
cargo run --release -- --config writer-agent.toml run
```

**Create pipeline:**

```bash
cargo run --bin inject-message -- \
  --agent-id researcher \
  --message "Research Rust async programming" \
  --next-agent writer
```

**Skills Learned:**
- Agent capabilities
- Multi-agent pipelines
- Task forwarding

## Common Issues

### "Connection refused" Error

**Problem:** Can't connect to MQTT broker

**Solution:**
```bash
# Check if broker is reachable
telnet mqtt.2389.dev 8883

# Verify credentials are set
echo $MQTT_USERNAME
echo $MQTT_PASSWORD
```

### "API key not found" Error

**Problem:** LLM API key not set

**Solution:**
```bash
# Set your API key
export ANTHROPIC_API_KEY="your-key-here"

# Verify it's set
echo $ANTHROPIC_API_KEY
```

### Agent Starts But Doesn't Respond

**Problem:** Agent running but not processing tasks

**Solution:**
1. Check agent logs for errors
2. Verify agent ID matches in config and injection
3. Monitor MQTT traffic with `mqtt-monitor`

See [Troubleshooting Guide](TROUBLESHOOTING.md) for more help.

## What's Next?

Now that you have a working agent:

1. **Customize Your Agent** - Modify the system prompt and tools
2. **Add More Tools** - Enable web search, file operations, etc.
3. **Deploy to Production** - See [Deployment Guide](DEPLOYMENT.md)
4. **Monitor Your Agent** - Set up [Observability](OBSERVABILITY.md)
5. **Build Custom Tools** - Create specialized functionality

## See Also

### Next Steps

- **[Configuration Reference](CONFIGURATION_REFERENCE.md)** - Complete configuration options and examples
- **[CLI Tools Reference](CLI_TOOLS.md)** - mqtt-monitor, inject-message, and testing utilities
- **[Deployment Guide](DEPLOYMENT.md)** - Production deployment with Docker and Kubernetes
- **[Observability Guide](OBSERVABILITY.md)** - Monitoring, metrics, and logging

### Understanding the System

- **[Architecture Overview](ARCHITECTURE.md)** - System design and components
- **[TaskEnvelope Protocol](TASKENVELOPE_PROTOCOL.md)** - Message format specification
- **[Agent Capabilities](AGENT_CAPABILITIES.md)** - Capability system and agent discovery

### Troubleshooting & Testing

- **[Troubleshooting Guide](TROUBLESHOOTING.md)** - Common problems and solutions
- **[Testing Guide](TESTING.md)** - Test strategy and procedures

## Questions?

- Check the [Troubleshooting Guide](TROUBLESHOOTING.md)
- Review the [Architecture Overview](ARCHITECTURE.md)
- Explore the [CLI Tools Reference](CLI_TOOLS.md)
- Open an issue on GitHub

Happy agent building! 🚀