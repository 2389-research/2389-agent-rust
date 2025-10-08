# Configuration Reference

Complete reference for all configuration options in 2389 Agent Protocol.

## Table of Contents

- [Configuration File Format](#configuration-file-format)
- [Agent Section](#agent-section)
- [MQTT Section](#mqtt-section)
- [LLM Section](#llm-section)
- [Budget Section](#budget-section)
- [Tools Section](#tools-section)
- [Environment Variables](#environment-variables)
- [Examples](#examples)

## Configuration File Format

Agents are configured using TOML files (`.toml`). The format is human-readable and supports comments.

**Basic Structure:**

```toml
[agent]
# Agent identification and capabilities

[mqtt]
# MQTT broker connection

[llm]
# LLM provider configuration

[budget]
# Resource limits

[[tools]]
# Tool configurations (can have multiple)
```

## Agent Section

Defines the agent's identity and capabilities.

```toml
[agent]
id = "my-agent"
description = "Agent description"
capabilities = ["capability1", "capability2"]
```

### `id` (required)

**Type:** String
**Format:** `[a-zA-Z0-9._-]+`
**Description:** Unique identifier for the agent.

**Valid Examples:**
```toml
id = "research-agent"
id = "agent-001"
id = "my_special.agent-v2"
```

**Invalid Examples:**
```toml
id = "agent with spaces"    # Spaces not allowed
id = "agent@home"           # @ not allowed
id = ""                     # Cannot be empty
```

### `description` (required)

**Type:** String
**Description:** Human-readable description of the agent's purpose.

```toml
description = "Specialized research agent for gathering information"
```

### `capabilities` (optional)

**Type:** Array of strings
**Default:** `[]`
**Description:** List of capabilities this agent provides. Used for agent discovery in v2.0 dynamic routing.

```toml
capabilities = ["research", "web-search", "fact-checking"]
```

**Common Capabilities:**
- `research` - Information gathering
- `writing` - Content creation
- `analysis` - Data analysis
- `coding` - Code generation/review
- `editing` - Content editing
- `web-search` - Web searching
- `file-operations` - File manipulation

## MQTT Section

Configures MQTT broker connection.

```toml
[mqtt]
broker_url = "mqtts://mqtt.example.com:8883"
username_env = "MQTT_USERNAME"
password_env = "MQTT_PASSWORD"
```

### `broker_url` (required)

**Type:** String
**Format:** `mqtt://host:port` or `mqtts://host:port`
**Description:** MQTT broker URL. Use `mqtts://` for TLS encryption.

**Examples:**
```toml
# Local broker (no TLS)
broker_url = "mqtt://localhost:1883"

# Remote broker with TLS
broker_url = "mqtts://mqtt.example.com:8883"

# Test broker (provided for development)
broker_url = "mqtts://mqtt.2389.dev:8883"
```

### `username_env` (required)

**Type:** String
**Description:** Environment variable name containing MQTT username.

```toml
username_env = "MQTT_USERNAME"
```

Then set:
```bash
export MQTT_USERNAME="your-username"
```

### `password_env` (required)

**Type:** String
**Description:** Environment variable name containing MQTT password.

```toml
password_env = "MQTT_PASSWORD"
```

Then set:
```bash
export MQTT_PASSWORD="your-password"
```

## LLM Section

Configures the Large Language Model provider.

```toml
[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are a helpful assistant."
temperature = 0.7
max_tokens = 2000
```

### `provider` (required)

**Type:** String
**Options:** `"anthropic"`, `"openai"`
**Description:** LLM provider to use.

```toml
# Anthropic Claude
provider = "anthropic"

# OpenAI GPT
provider = "openai"
```

### `model` (required)

**Type:** String
**Description:** Model identifier for the provider.

**Anthropic Models:**
```toml
model = "claude-sonnet-4-20250514"      # Claude Sonnet 4
model = "claude-opus-4-20250514"        # Claude Opus 4
```

**OpenAI Models:**
```toml
model = "gpt-4o"                        # GPT-4 Optimized
model = "gpt-4o-mini"                   # GPT-4 Mini
model = "gpt-4-turbo"                   # GPT-4 Turbo
```

### `api_key_env` (required)

**Type:** String
**Description:** Environment variable name containing API key.

```toml
# For Anthropic
api_key_env = "ANTHROPIC_API_KEY"

# For OpenAI
api_key_env = "OPENAI_API_KEY"
```

Then set:
```bash
export ANTHROPIC_API_KEY="sk-ant-..."
# or
export OPENAI_API_KEY="sk-..."
```

### `system_prompt` (required)

**Type:** String
**Description:** System prompt defining agent behavior and instructions.

```toml
system_prompt = "You are a research assistant specialized in gathering and synthesizing information from web sources."
```

**Tips for Good System Prompts:**
- Be specific about the agent's role
- Define input/output formats
- Set behavioral guidelines
- Specify tool usage requirements
- Include examples if complex

**Multi-line Prompts:**
```toml
system_prompt = """You are a researcher. Your job is:
1. Gather information using web_search tool
2. Verify facts using http_request tool
3. Provide comprehensive summaries with sources"""
```

### `temperature` (optional)

**Type:** Float
**Range:** 0.0 to 1.0
**Default:** 0.7
**Description:** Controls randomness in LLM responses.

```toml
temperature = 0.3  # More deterministic (good for research)
temperature = 0.7  # Balanced (default)
temperature = 0.9  # More creative (good for writing)
```

### `max_tokens` (optional)

**Type:** Integer
**Default:** 2000
**Description:** Maximum tokens in LLM response.

```toml
max_tokens = 2000  # Standard responses
max_tokens = 4000  # Longer, detailed responses
max_tokens = 1000  # Short, concise responses
```

## Budget Section

Prevents infinite loops and runaway costs by limiting LLM iterations.

```toml
[budget]
max_tool_calls = 15
max_iterations = 8
```

### `max_tool_calls` (optional)

**Type:** Integer
**Default:** 15
**Description:** Maximum number of tool calls allowed per task.

```toml
# Conservative (simple tasks)
max_tool_calls = 10

# Standard (general purpose)
max_tool_calls = 15

# Generous (research-heavy tasks)
max_tool_calls = 25
```

### `max_iterations` (optional)

**Type:** Integer
**Default:** 8
**Description:** Maximum LLM request/response iterations per task.

```toml
# Quick tasks
max_iterations = 5

# Standard tasks
max_iterations = 8

# Complex tasks
max_iterations = 12
```

## Tools Section

Configures available tools for the agent.

### Built-in Tools

```toml
[[tools]]
name = "echo"
implementation = "builtin"

[[tools]]
name = "http_request"
implementation = "builtin"

[tools.config]
max_response_size = 1048576  # 1MB
timeout = 30

[[tools]]
name = "file_read"
implementation = "builtin"

[tools.config]
max_file_size = 10485760     # 10MB
allowed_paths = ["/tmp", "/data"]

[[tools]]
name = "file_write"
implementation = "builtin"

[tools.config]
max_file_size = 10485760     # 10MB
allowed_paths = ["/tmp", "/data"]

[[tools]]
name = "web_search"
implementation = "builtin"

[tools.config]
max_results = 10
```

### Tool Configurations

#### echo

Simple echo tool for testing.

```toml
[[tools]]
name = "echo"
implementation = "builtin"
```

**No configuration options.**

#### http_request

Make HTTP requests to external APIs.

```toml
[[tools]]
name = "http_request"
implementation = "builtin"

[tools.config]
max_response_size = 1048576  # Maximum response size in bytes
timeout = 30                  # Request timeout in seconds
extract_content = true        # Extract readable content from HTML
```

**Configuration Options:**
- `max_response_size` - Maximum response size (bytes) [default: 1048576]
- `timeout` - Request timeout (seconds) [default: 30]
- `extract_content` - Extract text from HTML [default: true]

#### file_read

Read files from the filesystem.

```toml
[[tools]]
name = "file_read"
implementation = "builtin"

[tools.config]
max_file_size = 10485760         # 10MB
allowed_paths = ["/tmp", "/data"]
```

**Configuration Options:**
- `max_file_size` - Maximum file size to read (bytes) [required]
- `allowed_paths` - List of allowed directories [required]

**Security Note:** Only files within `allowed_paths` can be read.

#### file_write

Write files to the filesystem.

```toml
[[tools]]
name = "file_write"
implementation = "builtin"

[tools.config]
max_file_size = 10485760         # 10MB
allowed_paths = ["/tmp", "/data"]
```

**Configuration Options:**
- `max_file_size` - Maximum file size to write (bytes) [required]
- `allowed_paths` - List of allowed directories [required]

**Security Note:** Only files within `allowed_paths` can be written.

#### web_search

Search the web using configured search provider.

```toml
[[tools]]
name = "web_search"
implementation = "builtin"

[tools.config]
max_results = 10
api_key_env = "SEARCH_API_KEY"  # Optional, depends on provider
```

**Configuration Options:**
- `max_results` - Maximum search results [default: 10]
- `api_key_env` - Environment variable for search API key [optional]

## Environment Variables

All sensitive values are loaded from environment variables.

### Required Variables

```bash
# MQTT Credentials
export MQTT_USERNAME="your-username"
export MQTT_PASSWORD="your-password"

# LLM API Key (choose one)
export ANTHROPIC_API_KEY="sk-ant-..."    # For Anthropic
export OPENAI_API_KEY="sk-..."           # For OpenAI
```

### Optional Variables

```bash
# Web search API key (if using web_search tool)
export SEARCH_API_KEY="your-search-key"

# Custom environment variables
export MY_CUSTOM_VAR="value"
```

## Examples

### Minimal Agent

Simplest possible configuration:

```toml
[agent]
id = "simple-agent"
description = "Minimal agent configuration"

[mqtt]
broker_url = "mqtt://localhost:1883"
username_env = "MQTT_USERNAME"
password_env = "MQTT_PASSWORD"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are a helpful assistant."

[[tools]]
name = "echo"
implementation = "builtin"
```

### Research Agent

Agent specialized in research:

```toml
[agent]
id = "research-agent"
description = "Research and information gathering specialist"
capabilities = ["research", "web-search", "fact-checking"]

[mqtt]
broker_url = "mqtts://mqtt.2389.dev:8883"
username_env = "MQTT_USERNAME"
password_env = "MQTT_PASSWORD"

[llm]
provider = "openai"
model = "gpt-4o"
api_key_env = "OPENAI_API_KEY"
system_prompt = """You are a research specialist. Use web_search to find information,
then use http_request to gather detailed content. Always cite sources."""
temperature = 0.3
max_tokens = 4000

[budget]
max_tool_calls = 25
max_iterations = 12

[[tools]]
name = "web_search"
implementation = "builtin"

[tools.config]
max_results = 10

[[tools]]
name = "http_request"
implementation = "builtin"

[tools.config]
max_response_size = 2097152  # 2MB
timeout = 60
extract_content = true
```

### File Operations Agent

Agent with file system access:

```toml
[agent]
id = "file-agent"
description = "Agent with controlled file system access"
capabilities = ["file-operations", "data-processing"]

[mqtt]
broker_url = "mqtt://localhost:1883"
username_env = "MQTT_USERNAME"
password_env = "MQTT_PASSWORD"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You can read and write files. Always confirm operations with users."

[budget]
max_tool_calls = 15
max_iterations = 8

[[tools]]
name = "file_read"
implementation = "builtin"

[tools.config]
max_file_size = 10485760
allowed_paths = ["/tmp", "/data/workspace"]

[[tools]]
name = "file_write"
implementation = "builtin"

[tools.config]
max_file_size = 10485760
allowed_paths = ["/tmp", "/data/workspace"]
```

### Multi-Tool Agent

Agent with multiple capabilities:

```toml
[agent]
id = "multi-tool-agent"
description = "Agent with web, file, and search capabilities"
capabilities = ["research", "file-ops", "http", "search"]

[mqtt]
broker_url = "mqtts://mqtt.2389.dev:8883"
username_env = "MQTT_USERNAME"
password_env = "MQTT_PASSWORD"

[llm]
provider = "openai"
model = "gpt-4o"
api_key_env = "OPENAI_API_KEY"
system_prompt = "Multi-capable agent. Choose appropriate tools for each task."
temperature = 0.7
max_tokens = 2000

[budget]
max_tool_calls = 20
max_iterations = 10

[[tools]]
name = "web_search"
implementation = "builtin"

[[tools]]
name = "http_request"
implementation = "builtin"

[tools.config]
extract_content = true

[[tools]]
name = "file_read"
implementation = "builtin"

[tools.config]
max_file_size = 5242880
allowed_paths = ["/tmp"]

[[tools]]
name = "file_write"
implementation = "builtin"

[tools.config]
max_file_size = 5242880
allowed_paths = ["/tmp"]
```

## Validation

The agent validates configuration on startup. Common errors:

### Invalid Agent ID
```
Error: Invalid agent ID format: 'my agent with spaces'
Agent IDs must match: [a-zA-Z0-9._-]+
```

### Missing Environment Variable
```
Error: Environment variable not found: MQTT_USERNAME
Set with: export MQTT_USERNAME="value"
```

### Invalid MQTT URL
```
Error: Invalid MQTT broker URL: 'not-a-url'
Format: mqtt://host:port or mqtts://host:port
```

### Unknown Tool
```
Error: Unknown tool implementation: 'custom-tool'
Available implementations: builtin
```

## Best Practices

### Security

1. **Never hardcode credentials** - Always use environment variables
2. **Restrict file paths** - Use `allowed_paths` to limit file access
3. **Limit response sizes** - Prevent memory exhaustion
4. **Use TLS** - Always use `mqtts://` in production

### Performance

1. **Set appropriate budgets** - Prevent runaway costs
2. **Tune temperature** - Lower for factual tasks, higher for creative
3. **Limit max_tokens** - Reduce latency for simple tasks
4. **Use timeouts** - Prevent hanging on slow HTTP requests

### Reliability

1. **Test configurations** - Validate before deploying
2. **Monitor budgets** - Ensure limits aren't too restrictive
3. **Version control configs** - Track configuration changes
4. **Document prompts** - Explain complex system prompts

## See Also

### Getting Started

- **[Getting Started Guide](GETTING_STARTED.md)** - Step-by-step tutorial using these configurations
- **[CLI Tools Reference](CLI_TOOLS.md)** - Testing your configured agents with command-line tools

### Deployment & Operations

- **[Deployment Guide](DEPLOYMENT.md)** - Production deployment strategies
- **[Observability Guide](OBSERVABILITY.md)** - Monitoring and metrics configuration
- **[Troubleshooting Guide](TROUBLESHOOTING.md)** - Configuration-related issues

### Architecture & Protocol

- **[Architecture Overview](ARCHITECTURE.md)** - System design and component relationships
- **[TaskEnvelope Protocol](TASKENVELOPE_PROTOCOL.md)** - Message format specification
- **[Agent Capabilities](AGENT_CAPABILITIES.md)** - Understanding the capabilities field