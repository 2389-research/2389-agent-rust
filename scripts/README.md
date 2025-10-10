# Scripts Directory

This directory contains utility scripts for development, testing, quality checks, and monitoring of the 2389 Agent system.

---

## Development Scripts

### `dev-environment.sh`

**Purpose**: Start local MQTT broker and create development environment

**Usage**:
```bash
# Start with Docker (default)
./scripts/dev-environment.sh start

# Start with native mosquitto (no Docker needed)
./scripts/dev-environment.sh start-native

# Stop environment
./scripts/dev-environment.sh stop

# Check status
./scripts/dev-environment.sh status
```

**What it does**:
- Starts Mosquitto MQTT broker on port 1883
- Enables WebSocket support on port 9001
- Creates `agent-dev.toml` configuration file
- Provides development utilities

**Requirements**:
- Docker (for default mode) OR
- Native mosquitto (`brew install mosquitto`)

**Status**: ✅ Production-ready

---

### `v2-workflow-test.sh`

**Purpose**: Launch full V2 workflow demo with tmux (3 agents + 4 monitors)

**Usage**:
```bash
# Set API keys
export OPENAI_API_KEY="sk-..."
export SERPER_API_KEY="..."

# Launch demo
./scripts/v2-workflow-test.sh
```

**What it does**:
- Creates tmux session with 3 windows:
  - **Window 1**: 3 agents (researcher, writer, editor)
  - **Window 2**: 4 MQTT monitors (availability, inputs, conversations, progress)
  - **Window 3**: Message injector with examples
- Uses compact logging format for readability
- Configures health check ports (8080, 8081, 8082)

**Requirements**:
- tmux
- OPENAI_API_KEY environment variable
- SERPER_API_KEY environment variable (for researcher agent)

**Status**: ✅ Excellent for demos

---

## Quality & Testing Scripts

### `quality-check.sh`

**Purpose**: Run all quality gates (format, lint, type check, test compile, docs)

**Usage**:
```bash
./scripts/quality-check.sh
```

**What it does**:
1. `cargo fmt --check` - Verify code formatting
2. `cargo clippy --all-targets --all-features -- -D warnings` - Lint code
3. `cargo check --all-targets --all-features` - Type check
4. `cargo test --no-run` - Verify tests compile
5. `cargo doc --no-deps --document-private-items` - Check docs

**Status**: ✅ Production-ready, suitable for CI/CD

---

### `lint-docs.sh`

**Purpose**: Lint markdown files and validate Mermaid diagrams

**Usage**:
```bash
./scripts/lint-docs.sh
```

**What it does**:
- Installs markdownlint-cli if needed
- Installs @mermaid-js/mermaid-cli if needed
- Lints all markdown files in `docs/`
- Validates Mermaid diagram syntax

**Requirements**:
- Node.js and npm

**Status**: ✅ Good, minor Mermaid extraction improvements possible

---

## Git Hooks

### `git-hooks/install-hooks.sh`

**Purpose**: Install pre-commit hooks for automatic quality checks

**Usage**:
```bash
./scripts/git-hooks/install-hooks.sh
```

**What it does**:
- Copies `pre-commit` hook to `.git/hooks/`
- Makes hook executable
- Enables automatic checks on every commit

**Status**: ✅ Ready to use

---

### `git-hooks/pre-commit`

**Purpose**: Pre-commit hook that runs quality checks

**What it does**:
1. Identifies staged `.rs` files
2. Runs `cargo fmt --check`
3. Runs `cargo clippy --all-targets -- -D warnings`
4. Runs `cargo check --all-targets`
5. Prevents commit if any check fails

**Bypass** (if needed):
```bash
git commit --no-verify
```

**Status**: ✅ Excellent developer experience with helpful error messages

---

## Monitoring Scripts

### `monitor-pipeline.sh`

**Purpose**: Monitor agent health and MQTT traffic

**Usage**:
```bash
# Continuous monitoring (default: 3-agent demo)
./scripts/monitor-pipeline.sh

# One-time status check
./scripts/monitor-pipeline.sh status

# Monitor MQTT traffic
./scripts/monitor-pipeline.sh mqtt

# Tail all log files
./scripts/monitor-pipeline.sh logs

# Custom agent configuration
AGENTS="agent1:8080 agent2:8081" ./scripts/monitor-pipeline.sh status
LOG_DIR="/var/log/agents" ./scripts/monitor-pipeline.sh logs
```

**What it monitors**:
- Agent health status (via health endpoints)
- Recent log activity
- MQTT message flow

**Configuration**:
- `AGENTS`: Space-separated list of "name:port" pairs (default: researcher-agent:8080 writer-agent:8081 editor-agent:8082)
- `LOG_DIR`: Log directory path (default: logs)

**Status**: ✅ Ready, now configurable for any agent setup

---

## Configuration Files

### `mosquitto.conf`

**Purpose**: Mosquitto MQTT broker configuration for development

**What it configures**:
- Port 1883 (MQTT)
- Port 9001 (WebSockets)
- MQTT 5.0 support
- Anonymous access (dev only)
- Message size limit: 512KB
- Persistence disabled (dev)
- Comprehensive logging

**Used by**: `dev-environment.sh`

**Status**: ✅ Production-ready (for dev environment)

---

## Archived Scripts

### `archived/debugging/`

Contains historical debugging scripts from resolved issues.

See `archived/debugging/README.md` for details.

---

## Quick Reference

### Common Workflows

**Start development environment**:
```bash
./scripts/dev-environment.sh start
```

**Run quality checks before commit**:
```bash
./scripts/quality-check.sh
```

**Launch V2 demo workflow**:
```bash
export OPENAI_API_KEY="sk-..."
export SERPER_API_KEY="..."
./scripts/v2-workflow-test.sh
```

**Monitor running agents**:
```bash
./scripts/monitor-pipeline.sh
```

**Install git hooks** (one-time):
```bash
./scripts/git-hooks/install-hooks.sh
```

---

## Script Maintenance

### Adding New Scripts

1. Create script in appropriate category directory
2. Make executable: `chmod +x scripts/your-script.sh`
3. Add entry to this README
4. Test thoroughly
5. Update SCRIPTS_AUDIT_REPORT.md if needed

### Script Standards

All scripts should:
- Include shebang (`#!/bin/bash`)
- Use `set -e` for error handling
- Have a comment header explaining purpose
- Include usage examples
- Provide helpful error messages
- Use colors for output clarity

### Testing Scripts

Before committing new scripts:
```bash
shellcheck scripts/your-script.sh  # Lint bash scripts
./scripts/your-script.sh --help    # Test help output
```

---

## Related Documentation

- **[SCRIPTS_AUDIT_REPORT.md](../SCRIPTS_AUDIT_REPORT.md)** - Complete audit of all scripts
- **[DEPLOYMENT_TESTING.md](../DEPLOYMENT_TESTING.md)** - Deployment test procedures
- **[RUNBOOK.md](../RUNBOOK.md)** - Production operations guide
- **[DEPLOYMENT.md](../docs/DEPLOYMENT.md)** - Deployment procedures

---

**Last Updated**: 2025-10-10
**Scripts Version**: v1.0
