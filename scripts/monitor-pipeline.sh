#!/bin/bash

# Pipeline Monitoring Script
# Shows real-time status of all three agents and MQTT traffic

set -e

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

        check_agent_status 8080 "Researcher"
        check_agent_status 8081 "Writer    "
        check_agent_status 8082 "Editor    "

        echo ""
        echo "=== Recent Log Activity ==="

        if [[ -f logs/researcher.log ]]; then
            echo -e "${BLUE}Researcher (last 3 lines):${NC}"
            tail -3 logs/researcher.log 2>/dev/null | grep -E "(INFO|ERROR)" || echo "  No recent activity"
            echo ""
        fi

        if [[ -f logs/writer.log ]]; then
            echo -e "${BLUE}Writer (last 3 lines):${NC}"
            tail -3 logs/writer.log 2>/dev/null | grep -E "(INFO|ERROR)" || echo "  No recent activity"
            echo ""
        fi

        if [[ -f logs/editor.log ]]; then
            echo -e "${BLUE}Editor (last 3 lines):${NC}"
            tail -3 logs/editor.log 2>/dev/null | grep -E "(INFO|ERROR)" || echo "  No recent activity"
            echo ""
        fi

        echo "Press Ctrl+C to exit monitoring..."
        sleep 5
    done
}

# Handle script arguments
case "${1:-}" in
    "status")
        echo "=== Agent Status ==="
        check_agent_status 8080 "Researcher"
        check_agent_status 8081 "Writer    "
        check_agent_status 8082 "Editor    "
        ;;
    "mqtt")
        show_mqtt_traffic
        ;;
    "logs")
        echo "=== Tailing All Logs ==="
        tail -f logs/*.log 2>/dev/null || echo "No log files found"
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
        exit 1
        ;;
esac