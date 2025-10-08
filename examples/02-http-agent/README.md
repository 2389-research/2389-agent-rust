# Example 02: HTTP Agent

Agent with HTTP request capabilities for fetching web content.

## Quick Start

```bash
# 1. Configure environment (see ../01-echo-agent/README.md)
# 2. Start agent
./run.sh

# 3. Test HTTP requests (in another terminal)
./test-github-api.sh
```

## What This Example Demonstrates

- HTTP request tool configuration
- Response size limits
- Timeout configuration
- Content extraction from HTML
- Testing external API calls

## Configuration Highlights

```toml
[[tools]]
name = "http_request"
implementation = "builtin"

[tools.config]
max_response_size = 1048576  # 1MB max
timeout = 30                 # 30 second timeout
extract_content = true       # Extract text from HTML
```

## Files

- **http-agent.toml** - Configuration with HTTP tool
- **run.sh** - Start script
- **test-github-api.sh** - Test with GitHub API
- **test-webpage.sh** - Test with web page

## See Also

- [Configuration Reference](../../docs/CONFIGURATION_REFERENCE.md) - Tool configuration details
- [Example 01](../01-echo-agent/) - If you haven't done the basic example first