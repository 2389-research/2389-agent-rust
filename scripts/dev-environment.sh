#!/bin/bash
# 2389 Agent Development Environment Setup
# Starts local MQTT broker and provides utilities for agent experimentation

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log() {
    echo -e "${GREEN}[DEV-ENV]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[DEV-ENV]${NC} $1"
}

error() {
    echo -e "${RED}[DEV-ENV]${NC} $1"
}

check_dependencies() {
    log "Checking dependencies..."

    if ! command -v docker &> /dev/null; then
        error "Docker is required but not installed"
        exit 1
    fi

    # Check if Docker daemon is running
    if ! docker info &>/dev/null; then
        error "Docker daemon is not running!"
        echo
        echo "To fix this:"
        echo "  • If using Docker Desktop: Start Docker Desktop application"
        echo "  • If using Colima: Run 'colima start'"
        echo "  • If using OrbStack: Start OrbStack application"
        echo "  • If using other Docker setup: Start your Docker daemon"
        echo
        echo "Alternative: Use native MQTT broker instead:"
        echo "  brew install mosquitto"
        echo "  ./scripts/dev-environment.sh start-native"
        exit 1
    fi

    if ! command -v python3 &> /dev/null; then
        warn "Python3 recommended for message injection utilities"
    fi

    log "✓ Dependencies OK"
}

start_mqtt_broker() {
    log "Starting local MQTT broker..."

    # Stop any existing broker
    docker stop agent-mosquitto 2>/dev/null || true
    docker rm agent-mosquitto 2>/dev/null || true

    # Start Mosquitto MQTT broker with allow_anonymous
    docker run -d \
        --name agent-mosquitto \
        -p 1883:1883 \
        -p 9001:9001 \
        eclipse-mosquitto:latest \
        sh -c 'echo "listener 1883" > /tmp/mosquitto.conf && echo "allow_anonymous true" >> /tmp/mosquitto.conf && echo "listener 9001" >> /tmp/mosquitto.conf && echo "protocol websockets" >> /tmp/mosquitto.conf && mosquitto -c /tmp/mosquitto.conf'

    # Wait for broker to start
    sleep 2

    if docker ps | grep -q agent-mosquitto; then
        log "✓ MQTT broker running on mqtt://localhost:1883"
        log "  Web UI available on ws://localhost:9001"
    else
        error "Failed to start MQTT broker"
        exit 1
    fi
}

start_native_mqtt() {
    log "Starting native MQTT broker..."

    # Check if mosquitto is installed
    if ! command -v mosquitto &> /dev/null; then
        error "Mosquitto not installed. Install with: brew install mosquitto"
        exit 1
    fi

    # Kill any existing mosquitto processes
    pkill mosquitto 2>/dev/null || true
    sleep 1

    # Start mosquitto with custom config
    mosquitto -c "$PROJECT_ROOT/scripts/mosquitto.conf" -d

    # Wait for startup
    sleep 2

    # Check if it's running
    if pgrep mosquitto > /dev/null; then
        log "✓ Native MQTT broker running on mqtt://localhost:1883"
        log "  Process ID: $(pgrep mosquitto)"
        log "  Stop with: pkill mosquitto"
    else
        error "Failed to start native MQTT broker"
        exit 1
    fi
}

create_sample_config() {
    local config_file="$PROJECT_ROOT/agent-dev.toml"

    if [[ -f "$config_file" ]]; then
        warn "Development config already exists: $config_file"
        return
    fi

    log "Creating development agent configuration..."

    cat > "$config_file" << 'EOF'
# Development Configuration for 2389 Agent
agent_id = "dev-agent"
default_model = "gpt-4"
max_pipeline_depth = 16
task_timeout = 300
max_output_size = 1048576

[mqtt]
broker_url = "mqtt://localhost:1883"
qos = 1
keep_alive = 60

# Set these environment variables:
# export OPENAI_API_KEY="your-key-here"
# export ANTHROPIC_API_KEY="your-key-here"
openai_api_key = "${OPENAI_API_KEY}"
anthropic_api_key = "${ANTHROPIC_API_KEY}"

[[tools]]
name = "echo"
command = "echo"
timeout = 10
[tools.schema]
type = "object"
properties = { message = { type = "string" } }
required = ["message"]

[[tools]]
name = "curl"
command = "curl"
timeout = 30
[tools.schema]
type = "object"
properties = {
    url = { type = "string", format = "uri" },
    method = { type = "string", enum = ["GET", "POST", "PUT", "DELETE"] }
}
required = ["url"]

[[tools]]
name = "python_eval"
command = "python3"
timeout = 30
[tools.schema]
type = "object"
properties = {
    code = { type = "string" },
    args = { type = "array", items = { type = "string" } }
}
required = ["code"]
EOF

    log "✓ Created development config: $config_file"
    log "  Don't forget to set OPENAI_API_KEY or ANTHROPIC_API_KEY"
}

show_usage() {
    log "Development environment ready! Here's how to use it:"
    echo
    echo "1. Start your agent:"
    echo "   cargo run -- run --config agent-dev.toml"
    echo
    echo "2. In another terminal, inject messages:"
    echo "   cargo run --bin inject-message -- --agent-id dev-agent --message 'Hello world'"
    echo
    echo "3. Monitor MQTT traffic:"
    echo "   # Use external MQTT monitoring tools (Python scripts removed)"
    echo
    echo "4. Check agent health:"
    echo "   curl http://localhost:8080/health"
    echo
    echo "Environment variables needed:"
    echo "   export OPENAI_API_KEY='your-key'"
    echo "   export ANTHROPIC_API_KEY='your-key'"
    echo
}

cleanup() {
    log "Cleaning up development environment..."

    # Stop Docker container if running
    docker stop agent-mosquitto 2>/dev/null || true
    docker rm agent-mosquitto 2>/dev/null || true

    # Stop native mosquitto if running
    pkill mosquitto 2>/dev/null || true

    log "✓ Cleanup complete"
}

main() {
    case "${1:-start}" in
        "start")
            check_dependencies
            start_mqtt_broker
            create_sample_config
            show_usage
            ;;
        "start-native")
            start_native_mqtt
            create_sample_config
            show_usage
            ;;
        "stop")
            cleanup
            ;;
        "status")
            if docker ps 2>/dev/null | grep -q agent-mosquitto; then
                log "✓ Docker MQTT broker is running"
            elif pgrep mosquitto > /dev/null; then
                log "✓ Native MQTT broker is running (PID: $(pgrep mosquitto))"
            else
                warn "MQTT broker is not running"
            fi
            ;;
        *)
            echo "Usage: $0 [start|start-native|stop|status]"
            echo "  start        - Start development environment with Docker (default)"
            echo "  start-native - Start with native mosquitto (no Docker needed)"
            echo "  stop         - Stop and cleanup"
            echo "  status       - Check status"
            echo
            echo "If Docker daemon is not running, use 'start-native' instead."
            exit 1
            ;;
    esac
}

main "$@"