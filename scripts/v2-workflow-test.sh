#!/bin/bash
# V2 Workflow Testing Script
# Creates a tmux session with all agents, monitors, and test injection

SESSION="v2-test"

# Check for required environment variables
if [ -z "$OPENAI_API_KEY" ]; then
    echo "ERROR: OPENAI_API_KEY environment variable is not set"
    echo "Usage: OPENAI_API_KEY=key SERPER_API_KEY=key ./scripts/v2-workflow-test.sh"
    exit 1
fi

if [ -z "$SERPER_API_KEY" ]; then
    echo "ERROR: SERPER_API_KEY environment variable is not set"
    echo "The researcher agent needs this for web_search tool"
    echo "Usage: OPENAI_API_KEY=key SERPER_API_KEY=key ./scripts/v2-workflow-test.sh"
    exit 1
fi

# Check if session exists and kill it
tmux has-session -t $SESSION 2>/dev/null
if [ $? == 0 ]; then
    tmux kill-session -t $SESSION
fi

# Create new session with researcher agent (DEBUG mode, compact logging)
tmux new-session -d -s $SESSION -n agents
tmux select-pane -t $SESSION:agents.0 -T "RESEARCHER"
tmux send-keys -t $SESSION:agents.0 "LOG_LEVEL=DEBUG LOG_FORMAT=compact OPENAI_API_KEY='$OPENAI_API_KEY' SERPER_API_KEY='$SERPER_API_KEY' HEALTH_PORT=8080 cargo run --bin agent2389 -- --config config/dev-agents/researcher-agent.toml run" C-m

# Split horizontally and run writer agent (DEBUG mode, compact logging)
tmux split-window -h -t $SESSION:agents.0
tmux select-pane -t $SESSION:agents.1 -T "WRITER"
tmux send-keys -t $SESSION:agents.1 "LOG_LEVEL=DEBUG LOG_FORMAT=compact OPENAI_API_KEY='$OPENAI_API_KEY' HEALTH_PORT=8081 cargo run --bin agent2389 -- --config config/dev-agents/writer-agent.toml run" C-m

# Split vertically on the right pane and run editor agent (DEBUG mode, compact logging)
tmux split-window -v -t $SESSION:agents.1
tmux select-pane -t $SESSION:agents.2 -T "EDITOR"
tmux send-keys -t $SESSION:agents.2 "LOG_LEVEL=DEBUG LOG_FORMAT=compact OPENAI_API_KEY='$OPENAI_API_KEY' HEALTH_PORT=8082 cargo run --bin agent2389 -- --config config/dev-agents/editor-agent.toml run" C-m

# Create new window for MQTT monitors (2x2 grid)
tmux new-window -t $SESSION -n mqtt-monitors

# Create all the splits first WITHOUT sending commands
# Split horizontally - creates pane 1 on the right
tmux split-window -h -t $SESSION:mqtt-monitors.0

# Split top-left vertically - creates pane 2 below pane 0
tmux split-window -v -t $SESSION:mqtt-monitors.0

# Split top-right vertically - creates pane 3 below pane 1
tmux split-window -v -t $SESSION:mqtt-monitors.1

# Now we have stable pane layout:
# 0 (top-left)    | 1 (top-right)
# 2 (bottom-left) | 3 (bottom-right)

# Label all panes
tmux select-pane -t $SESSION:mqtt-monitors.0 -T "AVAILABILITY"
tmux select-pane -t $SESSION:mqtt-monitors.1 -T "INPUTS"
tmux select-pane -t $SESSION:mqtt-monitors.2 -T "CONVERSATIONS"
tmux select-pane -t $SESSION:mqtt-monitors.3 -T "PROGRESS"

# Now send commands to each pane
tmux send-keys -t $SESSION:mqtt-monitors.0 "cargo run --bin mqtt-monitor -- --mode availability --format pretty" C-m
tmux send-keys -t $SESSION:mqtt-monitors.1 "cargo run --bin mqtt-monitor -- --mode inputs --format pretty" C-m
tmux send-keys -t $SESSION:mqtt-monitors.2 "cargo run --bin mqtt-monitor -- --mode conversations --format pretty" C-m
tmux send-keys -t $SESSION:mqtt-monitors.3 "cargo run --bin mqtt-monitor -- --mode progress --format pretty" C-m

# Create new window for test injection
tmux new-window -t $SESSION -n injector
tmux select-pane -t $SESSION:injector -T "MESSAGE INJECTOR"
tmux send-keys -t $SESSION:injector "clear" C-m
tmux send-keys -t $SESSION:injector "cat << 'EOF'

V2 WORKFLOW TEST - INJECTOR

# Inject v1.0 message with pipeline (researcher → writer → editor):
cargo run --bin inject-message -- \\
  --agent-id researcher-agent \\
  --message \"Research the latest developments in Rust async programming\" \\
  --next-agent \"writer-agent,editor-agent\"

# Inject v2.0 message (workflow with AI routing):
cargo run --bin inject-message-v2 -- \\
  --query \"Research the latest developments in Rust async programming\" \\
  --agent researcher-agent

# With custom conversation ID:
cargo run --bin inject-message-v2 -- \\
  --query \"Write a comprehensive article about Rust async\" \\
  --agent researcher-agent \\
  --conversation-id my-test-1

EOF" C-m

# Arrange layout
tmux select-layout -t $SESSION:agents tiled
tmux select-layout -t $SESSION:mqtt-monitors tiled

# Attach to session
tmux select-window -t $SESSION:agents
tmux attach-session -t $SESSION
