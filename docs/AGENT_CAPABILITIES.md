# Agent Capabilities and Discovery System

## Overview

The 2389 Agent Protocol now supports descriptive properties in agent configurations to enable better agent discovery, routing, and orchestration. Agents can specify their capabilities through standardized configuration fields, allowing other components to make intelligent routing decisions.

## Configuration Structure

### Agent Section Schema

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentSection {
    /// Agent identifier (must match [a-zA-Z0-9._-]+)
    pub id: String,
    /// Description of what this agent does
    pub description: String,
    /// List of agent capabilities for routing and discovery
    #[serde(default)]
    pub capabilities: Vec<String>,
}
```

### TOML Configuration Format

```toml
[agent]
id = "research-agent"
description = "AI agent specialized in research and information gathering"
capabilities = ["research", "web-search", "information-gathering", "fact-checking", "source-verification", "data-extraction"]

[mqtt]
broker_url = "mqtt://localhost:1883"
# ... other sections
```

## Capability Categories

### Recommended Capability Tags

#### Data Processing
- `data-extraction` - Extract structured data from unstructured sources
- `data-transformation` - Transform data between formats
- `data-validation` - Validate data integrity and format compliance
- `data-analysis` - Analyze datasets and generate insights

#### Content Operations
- `writing` - Generate written content
- `editing` - Edit and improve existing content
- `proofreading` - Check for grammar, spelling, and style issues
- `content-creation` - Create various types of content
- `text-generation` - Generate text from prompts or data
- `content-polish` - Refine and improve content quality
- `text-refinement` - Fine-tune text for clarity and flow
- `style-improvement` - Enhance writing style and tone

#### Research and Information
- `research` - Conduct comprehensive research
- `web-search` - Search web for information
- `information-gathering` - Collect information from multiple sources
- `fact-checking` - Verify information accuracy
- `source-verification` - Validate source credibility
- `citation-management` - Manage and format citations

#### Technical Operations
- `code-generation` - Generate code in various languages
- `code-review` - Review code for quality and issues
- `testing` - Create and run tests
- `documentation` - Generate technical documentation
- `api-integration` - Integrate with external APIs
- `file-operations` - Read, write, and manipulate files

#### Communication and Formatting
- `markdown-formatting` - Format content using Markdown
- `html-generation` - Generate HTML content
- `json-processing` - Process and generate JSON data
- `xml-processing` - Handle XML data structures
- `email-composition` - Compose professional emails
- `report-generation` - Generate structured reports

#### Domain-Specific
- `financial-analysis` - Analyze financial data and trends
- `scientific-research` - Conduct scientific literature research
- `legal-document-analysis` - Analyze legal documents
- `medical-information` - Process medical and health information
- `educational-content` - Create educational materials

## Configuration Examples

### Research Agent
```toml
[agent]
id = "researcher-agent"
description = "AI agent specialized in research and information gathering"
capabilities = [
    "research",
    "web-search",
    "information-gathering",
    "fact-checking",
    "source-verification",
    "data-extraction"
]
```

### Writing Agent
```toml
[agent]
id = "writer-agent"
description = "AI agent specialized in writing and document creation"
capabilities = [
    "writing",
    "content-creation",
    "text-generation",
    "research-synthesis",
    "markdown-formatting"
]
```

### Editor Agent
```toml
[agent]
id = "editor-agent"
description = "AI agent specialized in editing and document finalization"
capabilities = [
    "editing",
    "proofreading",
    "content-polish",
    "text-refinement",
    "grammar-checking",
    "style-improvement"
]
```

### Technical Agent
```toml
[agent]
id = "tech-agent"
description = "AI agent specialized in technical tasks and code operations"
capabilities = [
    "code-generation",
    "code-review",
    "testing",
    "documentation",
    "api-integration",
    "file-operations"
]
```

## Agent Discovery and Routing

### Use Cases for Capabilities

#### 1. Dynamic Pipeline Construction
Orchestration systems can use capabilities to build optimal agent pipelines:

```rust
// Pseudo-code for pipeline construction
let research_agents = agents.filter(|a| a.capabilities.contains("research"));
let writing_agents = agents.filter(|a| a.capabilities.contains("writing"));
let editing_agents = agents.filter(|a| a.capabilities.contains("editing"));

let pipeline = build_pipeline(research_agents[0], writing_agents[0], editing_agents[0]);
```

#### 2. Load Balancing and Routing
Route tasks to appropriate agents based on required capabilities:

```rust
// Route task based on required capabilities
fn route_task(task: &Task, agents: &[Agent]) -> Option<&Agent> {
    agents.iter()
        .filter(|agent| task.required_capabilities.iter()
            .all(|req| agent.capabilities.contains(req)))
        .min_by_key(|agent| agent.current_load)
}
```

#### 3. Agent Health Monitoring
Monitor agent availability by capability type:

```rust
// Check availability by capability
let available_researchers = agents.iter()
    .filter(|a| a.capabilities.contains("research") && a.is_healthy())
    .count();
```

## Integration with TaskEnvelope Protocol

### Next Task Selection
The `next` field in TaskEnvelope can be populated based on agent capabilities:

```json
{
  "task_id": "550e8400-e29b-41d4-a716-446655440000",
  "conversation_id": "conv-123",
  "topic": "/control/agents/research-agent/input",
  "instruction": "Research the latest AI developments",
  "input": {"topic": "artificial intelligence trends"},
  "next": {
    "topic": "/control/agents/writer-agent/input",
    "instruction": "Write article from research",
    "input": null,
    "next": {
      "topic": "/control/agents/editor-agent/input",
      "instruction": "Edit and polish article",
      "input": null,
      "next": null
    }
  }
}
```

### Capability-Based Topic Routing
Agents can subscribe to capability-specific topics:

```
/control/capabilities/research/available     # Research-capable agents
/control/capabilities/writing/available      # Writing-capable agents
/control/capabilities/editing/available      # Editing-capable agents
```

## Status Publishing with Capabilities

### Enhanced Agent Status
Agent status messages can include current capability status:

```json
{
  "agent_id": "research-agent",
  "status": "available",
  "capabilities": [
    {"name": "research", "status": "available", "load": 0.3},
    {"name": "web-search", "status": "available", "load": 0.1},
    {"name": "fact-checking", "status": "rate-limited", "load": 0.8}
  ],
  "timestamp": "2024-01-15T10:30:00Z"
}
```

## Validation and Standards

### Capability Naming Conventions
- Use lowercase with hyphens: `web-search`, `fact-checking`
- Be specific but not overly granular: `research` not `internet-research-via-apis`
- Follow domain standards where applicable
- Avoid vendor-specific names: `text-generation` not `openai-completion`

### Configuration Validation
The capabilities field includes validation:

- **Optional**: Defaults to empty array if not specified
- **Format**: Array of strings
- **Case**: Lowercase recommended for consistency
- **Length**: No hard limits, but keep reasonable (< 20 capabilities per agent)

### Backward Compatibility
- Existing configurations without `capabilities` continue to work
- Default value is empty array `[]`
- No breaking changes to existing protocol

## Best Practices

### Capability Design
1. **Granular but Practical**: Balance specificity with usability
2. **Stable Naming**: Don't change capability names frequently
3. **Documentation**: Document what each capability means
4. **Testing**: Validate agents actually have claimed capabilities

### Configuration Management
1. **Version Control**: Track capability changes in git
2. **Validation**: Test configuration parsing before deployment
3. **Documentation**: Keep capability definitions up to date
4. **Monitoring**: Track which capabilities are actually used

### Pipeline Design
1. **Capability Matching**: Ensure pipeline stages match required capabilities
2. **Fallback Agents**: Have backup agents with overlapping capabilities
3. **Load Distribution**: Distribute work across agents with same capabilities
4. **Health Monitoring**: Monitor agent health per capability type

This capability system enables sophisticated agent orchestration while maintaining the protocol's simplicity and interoperability goals.

---

## See Also

### Configuration & Setup

- **[Configuration Reference](CONFIGURATION_REFERENCE.md)** - Configuring the capabilities field
- **[Getting Started Guide](GETTING_STARTED.md)** - Basic agent setup with capabilities
- **[CLI Tools Reference](CLI_TOOLS.md)** - dynamic-injector for capability-based routing

### Protocol & Architecture

- **[TaskEnvelope Protocol](TASKENVELOPE_PROTOCOL.md)** - How capabilities integrate with TaskEnvelope
- **[Architecture Overview](ARCHITECTURE.md)** - Capability system in the agent architecture
- **[Testing Guide](TESTING.md)** - Testing capability-based routing

### Operations

- **[Deployment Guide](DEPLOYMENT.md)** - Deploying agents with capability configuration
- **[Troubleshooting Guide](TROUBLESHOOTING.md)** - Debugging capability matching issues
- **[Observability Guide](OBSERVABILITY.md)** - Monitoring capability-based routing