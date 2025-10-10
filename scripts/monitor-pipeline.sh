#!/bin/bash

# Pipeline Monitoring Script
# Shows real-time status of agents and MQTT traffic
#
# Configuration via environment variables:
#   AGENTS="agent1:8080 agent2:8081 agent3:8082"
#   LOG_DIR="logs"
#
# Default: monitors the 3-agent demo workflow (researcher, writer, editor)

set -e

# Default configuration for 3-agent demo workflow
DEFAULT_AGENTS="researcher-agent:8080 writer-agent:8081 editor-agent:8082"
DEFAULT_LOG_DIR="logs"

# Allow override via environment
AGENTS="${AGENTS:-$DEFAULT_AGENTS}"
LOG_DIR="${LOG_DIR:-$DEFAULT_LOG_DIR}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

# Check if agents are running
check_agent_status() {
    local port=$1
    local name=$2

    if curl -s http://localhost:$port/health > /dev/null 2>&1; then
        local status=$(curl -s http://localhost:$port/health | jq -r '.status' 2>/dev/null || echo "unknown")
        echo -e "  ${name}: ${GREEN}${status}${NC} (port $port)"
    else
        echo -e "  ${name}: ${RED}OFFLINE${NC} (port $port)"
    fi
}

# Show MQTT traffic
show_mqtt_traffic() {
    log_info "Monitoring MQTT traffic..."
    echo "Listening for pipeline messages..."
    echo "Press Ctrl+C to stop"
    echo ""

    mosquitto_sub -h localhost -p 1883 -t "/control/agents/+/input" -t "/conversations/+/+" -v &
    SUB_PID=$!

    trap "kill $SUB_PID 2>/dev/null || true" EXIT

    wait $SUB_PID 2>/dev/null || true
}

# Main monitoring loop
monitor_loop() {
    while true; do
        clear
        echo "=== Pipeline Agent Status ($(date)) ==="
        echo ""

        # Monitor all configured agents
        for agent_config in $AGENTS; do
            # Split "agent-name:port" into name and port
            local agent_name="${agent_config%:*}"
            local port="${agent_config#*:}"

            # Pad name for alignment
            printf -v padded_name "%-20s" "$agent_name"
            check_agent_status "$port" "$padded_name"
        done

        echo ""
        echo "=== Recent Log Activity ==="

        # Show logs for each agent
        for agent_config in $AGENTS; do
            local agent_name="${agent_config%:*}"
            local log_file="$LOG_DIR/${agent_name}.log"

            if [[ -f "$log_file" ]]; then
                echo -e "${BLUE}${agent_name} (last 3 lines):${NC}"
                tail -3 "$log_file" 2>/dev/null | grep -E "(INFO|ERROR)" || echo "  No recent activity"
                echo ""
            fi
        done

        echo "Press Ctrl+C to exit monitoring..."
        sleep 5
    done
}

# Handle script arguments
case "${1:-}" in
    "status")
        echo "=== Agent Status ==="
        for agent_config in $AGENTS; do
            local agent_name="${agent_config%:*}"
            local port="${agent_config#*:}"
            printf -v padded_name "%-20s" "$agent_name"
            check_agent_status "$port" "$padded_name"
        done
        ;;
    "mqtt")
        show_mqtt_traffic
        ;;
    "logs")
        echo "=== Tailing All Logs ==="
        tail -f "$LOG_DIR"/*.log 2>/dev/null || echo "No log files found in $LOG_DIR"
        ;;
    "")
        monitor_loop
        ;;
    *)
        echo "Usage: $0 [status|mqtt|logs]"
        echo ""
        echo "  status - Check agent health once"
        echo "  mqtt   - Monitor MQTT traffic"
        echo "  logs   - Tail all log files"
        echo "  (no arg) - Continuous monitoring"
        echo ""
        echo "Configuration:"
        echo "  AGENTS=\"agent1:8080 agent2:8081\" $0 status"
        echo "  LOG_DIR=\"/var/log/agents\" $0 logs"
        echo ""
        echo "Defaults:"
        echo "  AGENTS: $DEFAULT_AGENTS"
        echo "  LOG_DIR: $DEFAULT_LOG_DIR"
        exit 1
        ;;
esac