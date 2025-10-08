# Example 01: Echo Agent

The simplest possible agent - echoes messages back using the built-in echo tool.

## What This Example Demonstrates

- Minimal agent configuration
- MQTT broker connection
- LLM integration (Anthropic Claude)
- Built-in tool usage (echo)
- Task injection

## Quick Start

### 1. Set Environment Variables

```bash
export MQTT_USERNAME="robots"
export MQTT_PASSWORD="Hat-Compass-Question-Remove4-Shirt"
export ANTHROPIC_API_KEY="your-api-key-here"
```

### 2. Start the Agent

```bash
# Terminal 1
./run.sh
```

You should see:
```
Starting echo agent...
Press Ctrl+C to stop

[INFO] Starting agent: echo-agent
[INFO] Connected to MQTT broker
[INFO] Subscribed to /control/agents/echo-agent/input
[INFO] Agent ready and waiting for tasks
```

### 3. Send a Test Message

```bash
# Terminal 2
./test.sh
```

The agent will receive the message, use the echo tool, and respond.

## Files

- **echo-agent.toml** - Agent configuration
- **run.sh** - Script to start the agent
- **test.sh** - Script to send a test message
- **README.md** - This file

## Configuration Breakdown

```toml
[agent]
id = "echo-agent"                    # Unique identifier
description = "Simple echo agent..." # Human-readable description

[mqtt]
broker_url = "mqtts://mqtt.2389.dev:8883"  # Test MQTT broker with TLS
username_env = "MQTT_USERNAME"              # Environment variable for username
password_env = "MQTT_PASSWORD"              # Environment variable for password

[llm]
provider = "anthropic"                      # LLM provider
model = "claude-sonnet-4-20250514"          # Model to use
api_key_env = "ANTHROPIC_API_KEY"           # Environment variable for API key
system_prompt = "You are a helpful..."      # Agent instructions

[budget]
max_tool_calls = 15                         # Max tool calls per task
max_iterations = 8                          # Max LLM iterations per task

[[tools]]
name = "echo"                               # Built-in echo tool
implementation = "builtin"
```

## Troubleshooting

### Agent Won't Start

**Problem**: "Error: MQTT_USERNAME not set"

**Solution**: Set the required environment variables:
```bash
export MQTT_USERNAME="robots"
export MQTT_PASSWORD="Hat-Compass-Question-Remove4-Shirt"
export ANTHROPIC_API_KEY="your-key-here"
```

### Can't Connect to MQTT Broker

**Problem**: Connection timeout or refused

**Solution**:
1. Check network connectivity
2. Verify firewall allows port 8883
3. Try alternate broker: `mqtt://localhost:1883` (if running local broker)

### No Response from Agent

**Problem**: Message sent but no response

**Solution**:
1. Check agent is running: Look for "Agent ready and waiting" message
2. Verify agent ID matches: Must be "echo-agent" in both config and injection
3. Monitor MQTT traffic:
   ```bash
   cargo run --release --bin mqtt-monitor -- --mode all --agent-id echo-agent
   ```

## Next Steps

- Try modifying the system prompt in `echo-agent.toml`
- Change the test message in `test.sh`
- Explore [Example 02: HTTP Agent](../02-http-agent/) for more capabilities
- Read the [CLI Tools Reference](../../docs/CLI_TOOLS.md)

## See Also

- [Getting Started Guide](../../docs/GETTING_STARTED.md)
- [Configuration Reference](../../docs/CONFIGURATION_REFERENCE.md)
- [Troubleshooting Guide](../../docs/TROUBLESHOOTING.md)