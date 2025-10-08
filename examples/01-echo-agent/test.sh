#!/bin/bash
# Send a test message to the echo agent

set -e

echo "Sending test message to echo-agent..."
echo ""

cargo run --release --manifest-path ../../Cargo.toml --bin inject-message -- \
    --agent-id echo-agent \
    --message "Echo this message: Hello, World!"

echo ""
echo "Message sent! Check the agent terminal for response."