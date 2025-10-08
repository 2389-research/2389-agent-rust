#!/bin/bash
# Start the echo agent

set -e

echo "Starting echo agent..."
echo "Press Ctrl+C to stop"
echo ""

# Check environment variables
if [ -z "$MQTT_USERNAME" ]; then
    echo "Error: MQTT_USERNAME not set"
    echo "Run: export MQTT_USERNAME=\"robots\""
    exit 1
fi

if [ -z "$MQTT_PASSWORD" ]; then
    echo "Error: MQTT_PASSWORD not set"
    echo "Run: export MQTT_PASSWORD=\"Hat-Compass-Question-Remove4-Shirt\""
    exit 1
fi

if [ -z "$ANTHROPIC_API_KEY" ] && [ -z "$OPENAI_API_KEY" ]; then
    echo "Error: Neither ANTHROPIC_API_KEY nor OPENAI_API_KEY is set"
    echo "Run: export ANTHROPIC_API_KEY=\"your-key-here\""
    exit 1
fi

# Run the agent
cargo run --release --manifest-path ../../Cargo.toml -- \
    --config "$(pwd)/echo-agent.toml" \
    run