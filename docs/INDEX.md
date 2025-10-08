# 2389 Agent Protocol - Documentation Index

**Last Updated:** 2025-09-29

Welcome to the 2389 Agent Protocol Rust implementation documentation. This index organizes all documentation by user type and common tasks.

## Quick Links

- 📚 [Main README](../README.md) - Project overview and quick start
- 🚀 [Getting Started Guide](GETTING_STARTED.md) - Step-by-step tutorial for new users
- ⚙️ [Configuration Reference](CONFIGURATION_REFERENCE.md) - Complete configuration options
- 🐛 [Troubleshooting Guide](TROUBLESHOOTING.md) - Common issues and solutions
- 📊 [Current Status](../DOCUMENTATION_STATUS.md) - Implementation and documentation status

## Documentation by User Type

### 🎯 New Users (Getting Started)

Start here if you're new to the 2389 Agent Protocol or this implementation.

1. **[Getting Started Guide](GETTING_STARTED.md)** - Your first agent in 10 minutes
2. **[Configuration Reference](CONFIGURATION_REFERENCE.md)** - Understanding agent configuration
3. **[Task Injector Guide](TASK_INJECTOR_GUIDE.md)** - Sending tasks to your agent
4. **[Agent Capabilities](AGENT_CAPABILITIES.md)** - Understanding agent capabilities
5. **[Troubleshooting Guide](TROUBLESHOOTING.md)** - When things go wrong

### 🚀 Operators (Deploying & Running)

For those deploying and operating agents in production.

1. **[Deployment Guide](DEPLOYMENT.md)** - Docker and Kubernetes deployment
2. **[Configuration Reference](CONFIGURATION_REFERENCE.md)** - All configuration options
3. **[Observability Guide](OBSERVABILITY.md)** - Monitoring, metrics, and health checks
4. **[Troubleshooting Guide](TROUBLESHOOTING.md)** - Debugging and diagnostics
5. **[Testing Guide](TESTING.md)** - Test strategy, execution, and validation

### 💻 Developers (Building & Extending)

For developers building custom agents or extending functionality.

1. **[Architecture Overview](ARCHITECTURE.md)** - System design and components
2. **[TaskEnvelope Protocol](TASKENVELOPE_PROTOCOL.md)** - Protocol specification
3. **[Custom Tools Guide](CUSTOM_TOOLS_GUIDE.md)** - Creating your own tools
4. **[Development Tools Guide](DEVELOPMENT_TOOLS.md)** - CLI utilities for development
5. **[Testing Guide](TESTING.md)** - Writing and running tests
6. **[Test Coverage Summary](../TEST_COVERAGE_SUMMARY.md)** - Current test statistics

### 🔬 Contributors (Improving the Project)

For those contributing to the 2389 agent implementation.

1. **[Architecture Overview](ARCHITECTURE.md)** - Understanding the codebase
2. **[Technical Requirements](TECHNICAL_REQUIREMENTS.md)** - Design specification
3. **[Testing Guide](TESTING.md)** - Test requirements and coverage
4. **[Dynamic Routing Analysis](../DYNAMIC_ROUTING_ANALYSIS.md)** - v2.0 implementation status
5. **[Test Coverage Summary](../TEST_COVERAGE_SUMMARY.md)** - Test statistics

## Documentation by Topic

### Core Concepts

- **[Architecture Overview](ARCHITECTURE.md)** - System design, components, and data flow
- **[TaskEnvelope Protocol](TASKENVELOPE_PROTOCOL.md)** - Message format and pipeline architecture
- **[Agent Capabilities](AGENT_CAPABILITIES.md)** - Capability system and agent discovery

### Configuration & Setup

- **[Getting Started Guide](GETTING_STARTED.md)** - First-time setup walkthrough
- **[Configuration Reference](CONFIGURATION_REFERENCE.md)** - Complete configuration guide
- **[Deployment Guide](DEPLOYMENT.md)** - Production deployment strategies

### Development Tools

- **[Task Injector Guide](TASK_INJECTOR_GUIDE.md)** - Sending test tasks (v1.0)
- **[MQTT Monitor Guide](MQTT_MONITOR_GUIDE.md)** - Real-time MQTT monitoring
- **[Pipeline Injector Guide](PIPELINE_INJECTOR_GUIDE.md)** - Multi-agent pipeline testing
- **[Dynamic Injector Guide](DYNAMIC_INJECTOR_GUIDE.md)** - v2.0 task envelope creation

### Protocol Versions

#### Version 1.0 (Production Ready) ✅
- **[TaskEnvelope Protocol](TASKENVELOPE_PROTOCOL.md)** - v1.0 specification
- **[Task Injector Guide](TASK_INJECTOR_GUIDE.md)** - v1.0 task creation
- Status: Complete, 286 tests passing

#### Version 2.0 (80% Complete) 🚧
- **[Dynamic Routing Analysis](../DYNAMIC_ROUTING_ANALYSIS.md)** - Implementation status
- **[Dynamic Routing Guide](DYNAMIC_ROUTING_GUIDE.md)** - Configuration and usage (coming soon)
- **[Migration Guide](MIGRATION_GUIDE.md)** - v1.0 → v2.0 upgrade path (coming soon)
- Status: See [DYNAMIC_ROUTING_ANALYSIS.md](../DYNAMIC_ROUTING_ANALYSIS.md)

### Operations & Monitoring

- **[Deployment Guide](DEPLOYMENT.md)** - Container deployment and orchestration
- **[Observability Guide](OBSERVABILITY.md)** - Metrics, logging, and health checks
- **[Troubleshooting Guide](TROUBLESHOOTING.md)** - Common issues and solutions

### Testing & Quality

- **[Testing Guide](TESTING.md)** - Test strategy and execution
- **[Test Coverage Summary](../TEST_COVERAGE_SUMMARY.md)** - Current coverage statistics

## Common Tasks

### Running Your First Agent
1. Read [Getting Started Guide](GETTING_STARTED.md)
2. Configure your agent: [Configuration Reference](CONFIGURATION_REFERENCE.md)
3. Send a test task: [Task Injector Guide](TASK_INJECTOR_GUIDE.md)

### Deploying to Production
1. Review [Deployment Guide](DEPLOYMENT.md)
2. Configure monitoring: [Observability Guide](OBSERVABILITY.md)
3. Set up health checks: [Observability Guide](OBSERVABILITY.md#health-checks)

### Debugging Issues
1. Check [Troubleshooting Guide](TROUBLESHOOTING.md)
2. Review logs: [Observability Guide](OBSERVABILITY.md#logging)
3. Monitor metrics: [Observability Guide](OBSERVABILITY.md#metrics)

### Creating Custom Tools
1. Read [Custom Tools Guide](CUSTOM_TOOLS_GUIDE.md)
2. Review [Architecture Overview](ARCHITECTURE.md#tool-system)
3. Test your tool: [Testing Guide](TESTING.md)

### Understanding v2.0 Dynamic Routing
1. Review [Dynamic Routing Analysis](../DYNAMIC_ROUTING_ANALYSIS.md)
2. Check implementation status
3. Follow [Dynamic Routing Guide](DYNAMIC_ROUTING_GUIDE.md) (coming soon)

## Reference Documentation

### Protocol Specifications
- [TaskEnvelope Protocol](TASKENVELOPE_PROTOCOL.md) - Complete protocol specification
- [Agent Capabilities](AGENT_CAPABILITIES.md) - Capability system details
- [Technical Requirements](TECHNICAL_REQUIREMENTS.md) - Design specification

### API Documentation
- Run `cargo doc --open` for Rust API documentation
- Online docs: [docs.rs/agent2389](https://docs.rs/agent2389) (coming soon)

### Configuration
- [Configuration Reference](CONFIGURATION_REFERENCE.md) - All configuration options
- [Example Configurations](../config/dev-agents/) - Sample configurations

## Status & Roadmap

### Current Status
- ✅ v1.0 Protocol: Complete and production-ready
- 🚧 v2.0 Dynamic Routing: 80% complete
- ✅ Testing: 286 tests passing (100% pass rate)
- ✅ Documentation: Comprehensive with ongoing improvements

### Documentation Roadmap
- 🚧 Getting Started Guide (in progress)
- 🚧 Configuration Reference (in progress)
- 🚧 Troubleshooting Guide (in progress)
- 📅 Custom Tools Guide (planned)
- 📅 Dynamic Routing Guide (planned for v2.0 release)
- 📅 Migration Guide (planned for v2.0 release)
- 📅 Performance Tuning Guide (planned)

See [DOCUMENTATION_STATUS.md](../DOCUMENTATION_STATUS.md) for detailed status.

## Contributing to Documentation

Found an issue or want to improve documentation?

1. Check existing documentation for gaps
2. Follow the documentation style guide (coming soon)
3. Submit pull requests with improvements
4. Keep examples practical and runnable

## Questions or Feedback?

- Open an issue for documentation bugs or improvements
- Check [Troubleshooting Guide](TROUBLESHOOTING.md) for common questions
- Review existing documentation before asking

---

**Navigation Tips:**
- Use your browser's search (Ctrl/Cmd+F) to find specific topics
- Most guides include a table of contents for quick navigation
- Cross-references link to related documentation sections
- All file paths are relative to the repository root

**Last Updated:** 2025-09-29 | **Documentation Version:** 1.0