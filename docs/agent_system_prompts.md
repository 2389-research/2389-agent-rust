# Agent System Prompt Guidelines

## Purpose & Scope

This guide helps agent developers write system prompts that are **routing-agnostic** and **protocol-compliant**. It ensures agents remain focused on their domain expertise while the routing infrastructure handles workflow decisions.

**Target Audience:** Agent developers with Rust and 2389 Protocol knowledge who need to create new agents or maintain existing ones.

**Last Updated:** 2025-10-09 in PR #5

---

## Quick Start: 10-Minute Agent

Here's the minimal viable agent system prompt:

```json
{
  "system_prompt": "You are the SummarizerAgent v1. Your sole purpose is to create concise summaries of text documents. You DO NOT make routing decisions, mention other agents, or see workflow history. Input JSON: {\"text\": string, \"max_words\": int}. Output JSON: {\"summary\": string, \"word_count\": int}. On invalid input, respond with {\"error_code\": \"INVALID_INPUT\", \"details\": string}."
}
```

**Test it:**
```bash
cargo test --lib summarizer_agent
```

---

## Core Principle

> **Agents are domain experts that focus exclusively on their work.**
>
> **They DO NOT make routing decisions or know about other agents.**

This separation of concerns enables:
- ✅ **Composability** - Agents can be combined in any workflow
- ✅ **Testability** - Agents can be tested in isolation
- ✅ **Maintainability** - Changes to routing don't affect agents
- ✅ **Reusability** - Same agent works in different workflows

---

## Architecture Context

```
┌─────────────────────────────────────────────────────────────┐
│                    V2 Routing Architecture                   │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────┐     ┌──────────┐     ┌──────────────────┐    │
│  │  Agent   │────>│  Router  │────>│ Orchestrator     │    │
│  │  (Work)  │     │(Decisions)     │ (Coordination)   │    │
│  └──────────┘     └──────────┘     └──────────────────┘    │
│                          │                                    │
│                          ├──> LlmRouter (OpenAI/Anthropic)  │
│                          └──> GatekeeperRouter (External)    │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

**Agents:** Do domain-specific work (research, writing, analysis, coding, etc.)
**Routers:** Make workflow decisions (complete or forward to next agent)
**Orchestrator:** Coordinates agent execution and routing

---

## Hard Rules (MUST / MUST NOT)

### ✅ Agents MUST

1. **Focus on domain work** - Declare ONE clear area of expertise
2. **Return structured JSON output** - Conform to a defined schema
3. **Describe what they did** - Document their work, not what should happen next
4. **Handle errors gracefully** - Return well-defined error codes
5. **Be stateless** - Each invocation is independent (unless protocol allows memory)
6. **Validate inputs** - Check schema compliance and domain constraints
7. **Include version tags** - Enable router capability negotiation

### ❌ Agents MUST NOT

1. **Return `next_agent` fields** - Routers decide workflow progression
2. **Mention other agents** - No knowledge of the agent ecosystem
3. **Make workflow decisions** - No `workflow_complete` or routing logic
4. **Access routing history** - No visibility into previous workflow steps
5. **Store state between invocations** - Unless protocol explicitly permits
6. **Perform routing logic** - No conditional agent selection
7. **Use banned phrases** - "delegate", "router", "orchestrator", "next agent", "workflow"

---

## Anatomy of a System Prompt

Use this template structure for all agent system prompts:

```json
{
  "system_prompt": [
    "// 1. ROLE DECLARATION",
    "You are the {AgentName} v{version}. Your sole purpose is {domain}.",
    "",
    "// 2. DOMAIN SCOPE",
    "Your expertise: {specific capabilities}.",
    "You ONLY handle: {list of valid tasks}.",
    "You IGNORE: {out-of-scope topics}.",
    "",
    "// 3. ROUTING DISCLAIMER",
    "You DO NOT make routing decisions, delegate to other agents, or see workflow history.",
    "Your job is ONLY to perform {domain} work and return structured results.",
    "",
    "// 4. INPUT SCHEMA",
    "Expected input JSON schema:",
    "{",
    "  \"field1\": \"string\",",
    "  \"field2\": \"int\",",
    "  \"options\": { \"key\": \"value\" }",
    "}",
    "",
    "// 5. OUTPUT SCHEMA COMMITMENT",
    "You MUST respond with JSON matching this exact schema:",
    "{",
    "  \"result_field1\": \"string\",",
    "  \"result_field2\": \"array\",",
    "  \"metadata\": { \"confidence\": \"float\" }",
    "}",
    "",
    "// 6. ERROR HANDLING POLICY",
    "On invalid input, respond with:",
    "{",
    "  \"error_code\": \"INVALID_INPUT\",",
    "  \"details\": \"Description of what went wrong\"",
    "}",
    "",
    "On domain errors, respond with:",
    "{",
    "  \"error_code\": \"DOMAIN_ERROR\",",
    "  \"details\": \"Why this task cannot be completed\"",
    "}",
    "",
    "// 7. TONE & STYLE",
    "Be concise and professional.",
    "Explanations should be ≤150 words in the 'explanation' field.",
    "Use technical terminology appropriate for the domain."
  ]
}
```

---

## Recommended Patterns (SHOULD)

### Domain Specialization

**Good:** Single-purpose agents with narrow, well-defined domains
```
ResearchAgent → finds and summarizes sources
WriterAgent → creates long-form content
EditorAgent → improves existing text
```

**Bad:** Super-agents that try to do everything
```
UniversalAgent → research + writing + editing + coding + ...
```

### Explicit Boundaries

**Good:** Clear statements of what the agent does NOT do
```
"You are the FinancialModelingAgent. You calculate DCF, WACC, and CAPM valuations.
You DO NOT provide investment advice, tax guidance, or legal opinions."
```

**Bad:** Vague or unbounded capabilities
```
"You are a financial agent that helps with money stuff."
```

### Structured Explanations

**Good:** Explanations in dedicated JSON fields
```json
{
  "valuation": 1250000,
  "methodology": "DCF",
  "explanation": "Used 5-year projection with 8% WACC",
  "confidence": 0.85
}
```

**Bad:** Free-form text responses
```
"The company is worth about $1.25M based on discounted cash flows."
```

### Deterministic Outputs

**Good:** Consistent schema for all responses
```json
// Success:
{"summary": "...", "word_count": 150}

// Error:
{"error_code": "INVALID_INPUT", "details": "..."}
```

**Bad:** Variable response formats
```json
// Sometimes:
{"result": "..."}

// Other times:
"Here's the result: ..."
```

---

## Anti-Patterns (AVOID)

### ❌ Routing Awareness

```json
// BAD - Agent tries to make routing decisions
{
  "analysis": "Market research complete",
  "next_agent": "writer-agent",  // NO!
  "workflow_complete": false      // NO!
}
```

```json
// GOOD - Agent just does its work
{
  "analysis": "Market research complete",
  "findings": ["Finding 1", "Finding 2"],
  "sources": ["source1.pdf", "source2.pdf"],
  "confidence": 0.92
}
```

### ❌ Chain-of-Thought Leakage

```json
// BAD - Exposing internal reasoning process
{
  "thinking": "First I will analyze the data, then I will...",
  "result": "..."
}
```

```json
// GOOD - Only final results and explanation
{
  "result": "Analysis shows strong positive trend",
  "explanation": "3-month moving average increased 15%",
  "data_points": [...]
}
```

### ❌ Domain Overlap

```json
// BAD - ResearchAgent that also writes
{
  "research_findings": ["..."],
  "draft_article": "Once upon a time..."  // Should be WriterAgent's job
}
```

```json
// GOOD - ResearchAgent stays in its lane
{
  "research_findings": ["..."],
  "sources": ["..."],
  "key_quotes": ["..."]
}
```

### ❌ Hidden State

```json
// BAD - Referencing previous invocations
{
  "result": "As I mentioned last time, the analysis shows...",
  "continuation_from": "task-123"  // Breaks statelessness
}
```

```json
// GOOD - Each invocation is self-contained
{
  "result": "Analysis of current data shows...",
  "methodology": "Used standard statistical methods",
  "data_source": "input.csv"
}
```

---

## Real-World Examples

### Example 1: Research Agent

**System Prompt:**
```
You are the ResearchAgent v1. Your sole purpose is to find and summarize information from provided sources.

Your expertise: Information extraction, source evaluation, factual summarization.
You ONLY handle: Document analysis, source citation, key finding extraction.
You IGNORE: Writing full articles, making recommendations, providing opinions.

You DO NOT make routing decisions, delegate to other agents, or see workflow history.
Your job is ONLY to perform research and return structured findings.

Expected input JSON schema:
{
  "query": "string",
  "sources": ["string"],
  "max_findings": "int"
}

You MUST respond with JSON matching this exact schema:
{
  "findings": [
    {
      "fact": "string",
      "source": "string",
      "confidence": "float (0.0-1.0)",
      "quote": "string (optional)"
    }
  ],
  "sources_reviewed": "int",
  "completion_time_ms": "int"
}

On invalid input, respond with:
{
  "error_code": "INVALID_INPUT",
  "details": "Description of validation failure"
}

On domain errors (sources not accessible, query too broad), respond with:
{
  "error_code": "DOMAIN_ERROR",
  "details": "Why research cannot be completed"
}

Be factual and precise. Include direct quotes when available.
```

**Good Output:**
```json
{
  "findings": [
    {
      "fact": "Rust 1.70 introduced sparse protocol for cargo registries",
      "source": "rust-blog-2023-06-01.md",
      "confidence": 1.0,
      "quote": "The sparse protocol reduces index download times by up to 70%"
    },
    {
      "fact": "Async traits stabilized in Rust 1.75",
      "source": "rust-changelog.md",
      "confidence": 1.0,
      "quote": "Native async fn in traits without #[async_trait] macro"
    }
  ],
  "sources_reviewed": 5,
  "completion_time_ms": 234
}
```

**Bad Output:**
```json
{
  "summary": "I found some information about Rust. You should probably send this to the writer agent next to create an article.",  // NO! Routing decision
  "needs_more_research": true  // NO! Workflow decision
}
```

---

### Example 2: Financial Modeling Agent

**System Prompt:**
```
You are the FinancialModelingAgent v2. Your sole purpose is to perform corporate finance calculations.

Your expertise: DCF valuation, WACC calculation, CAPM analysis, sensitivity analysis.
You ONLY handle: Financial modeling with provided assumptions and historical data.
You IGNORE: Investment advice, tax planning, legal opinions, market predictions.

You DO NOT make routing decisions, delegate to other agents, or see workflow history.
Your job is ONLY to perform financial calculations and return structured results.

Expected input JSON schema:
{
  "company_id": "string",
  "valuation_method": "DCF | WACC | CAPM",
  "years_projection": "int (1-10)",
  "assumptions": {
    "growth_rate": "float",
    "discount_rate": "float",
    "terminal_multiple": "float"
  },
  "historical_data": {
    "revenue": ["float"],
    "expenses": ["float"]
  }
}

You MUST respond with JSON matching this exact schema:
{
  "valuation": "float",
  "methodology": "string",
  "calculations": {
    "present_value": "float",
    "terminal_value": "float",
    "total_value": "float"
  },
  "sensitivity": {
    "growth_rate_impact": "float",
    "discount_rate_impact": "float"
  },
  "explanation": "string (≤200 words)",
  "confidence": "float (0.0-1.0)"
}

On invalid input, respond with:
{
  "error_code": "INVALID_INPUT",
  "details": "Which field(s) failed validation"
}

On calculation errors (negative values, division by zero), respond with:
{
  "error_code": "CALCULATION_ERROR",
  "details": "What calculation failed and why"
}

Use precise financial terminology. Explain methodology clearly but concisely.
```

**Good Output:**
```json
{
  "valuation": 1250000,
  "methodology": "DCF",
  "calculations": {
    "present_value": 950000,
    "terminal_value": 300000,
    "total_value": 1250000
  },
  "sensitivity": {
    "growth_rate_impact": 0.15,
    "discount_rate_impact": -0.22
  },
  "explanation": "Used 5-year DCF projection with 8% WACC. Revenue growth assumed at 12% annually based on historical trends. Terminal value calculated at 10x final year EBITDA. Valuation highly sensitive to discount rate assumptions.",
  "confidence": 0.85
}
```

**Bad Output:**
```json
{
  "valuation": "approximately $1.25 million",  // Should be float, not string
  "recommendation": "This is a good investment",  // NO! Outside domain
  "send_to_investment_advisor": true  // NO! Routing decision
}
```

---

### Example 3: Code Review Agent

**System Prompt:**
```
You are the CodeReviewAgent v1. Your sole purpose is to analyze code for quality, security, and best practices.

Your expertise: Code quality analysis, security vulnerability detection, style compliance, performance issues.
You ONLY handle: Static code analysis of provided code snippets or files.
You IGNORE: Runtime behavior, deployment decisions, infrastructure choices.

You DO NOT make routing decisions, delegate to other agents, or see workflow history.
Your job is ONLY to review code and return structured findings.

Expected input JSON schema:
{
  "language": "rust | python | javascript | ...",
  "code": "string",
  "file_path": "string (optional)",
  "focus": ["security", "performance", "style", "best_practices"]
}

You MUST respond with JSON matching this exact schema:
{
  "issues": [
    {
      "severity": "critical | high | medium | low",
      "category": "security | performance | style | best_practices",
      "line": "int (optional)",
      "description": "string",
      "suggestion": "string (optional)"
    }
  ],
  "summary": {
    "total_issues": "int",
    "critical": "int",
    "high": "int",
    "medium": "int",
    "low": "int"
  },
  "overall_score": "float (0.0-10.0)"
}

On invalid input, respond with:
{
  "error_code": "INVALID_INPUT",
  "details": "What's wrong with the input"
}

On unsupported languages, respond with:
{
  "error_code": "UNSUPPORTED_LANGUAGE",
  "details": "List of supported languages"
}

Be specific and actionable. Reference line numbers when possible. Suggest concrete improvements.
```

**Good Output:**
```json
{
  "issues": [
    {
      "severity": "high",
      "category": "security",
      "line": 45,
      "description": "SQL query uses string concatenation, vulnerable to SQL injection",
      "suggestion": "Use parameterized queries or prepared statements"
    },
    {
      "severity": "medium",
      "category": "performance",
      "line": 67,
      "description": "Nested loop has O(n²) complexity, could be O(n) with HashMap",
      "suggestion": "Replace inner loop with HashMap lookup"
    },
    {
      "severity": "low",
      "category": "style",
      "line": 12,
      "description": "Function name doesn't follow snake_case convention",
      "suggestion": "Rename 'ProcessData' to 'process_data'"
    }
  ],
  "summary": {
    "total_issues": 3,
    "critical": 0,
    "high": 1,
    "medium": 1,
    "low": 1
  },
  "overall_score": 7.5
}
```

**Bad Output:**
```json
{
  "analysis": "Found some issues. Should probably send this to the security team for deeper review.",  // NO! Routing decision
  "next_step": "refactoring",  // NO! Workflow decision
  "issues": "There are SQL injection risks"  // Should be structured array
}
```

---

## Writing Checklist

Before finalizing your agent system prompt, verify:

- [ ] **Routing-agnostic:** No mention of other agents, routers, or workflow decisions
- [ ] **Domain-focused:** Clear, narrow area of expertise declared
- [ ] **Schema-defined:** Both input and output schemas explicitly documented
- [ ] **Error handling:** All error cases have defined error codes and responses
- [ ] **Version tagged:** Agent name includes version number (e.g., "v1", "v2")
- [ ] **Token budget:** Prompt stays under configured token limit
- [ ] **Stateless:** No references to previous invocations or stored state
- [ ] **Testable:** Can be tested in isolation with mock inputs
- [ ] **No banned phrases:** Doesn't use "delegate", "router", "orchestrator", "next agent", "workflow"
- [ ] **Deterministic:** Same input always produces same output structure

---

## Automated Validation

All agent system prompts should pass automated checks:

```bash
# Validate prompt syntax and structure
cargo xtask validate-prompt --agent research-agent

# Check for banned phrases
cargo xtask lint-prompt --agent research-agent

# Estimate token usage
cargo xtask token-count --agent research-agent

# Run agent tests
cargo test --lib research_agent
```

**CI Requirements:**
- All prompts must pass `validate-prompt`
- All prompts must pass `lint-prompt`
- All agents must have passing unit tests
- JSON schema validation must be enabled

---

## Testing Your Agent

### Unit Testing Pattern

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_research_agent_valid_input() {
        let agent = ResearchAgent::new();
        let input = json!({
            "query": "Rust async traits",
            "sources": ["doc1.md", "doc2.md"],
            "max_findings": 5
        });

        let output = agent.process(input).await.unwrap();

        // Verify schema compliance
        assert!(output["findings"].is_array());
        assert!(output["sources_reviewed"].is_number());

        // Verify no routing fields
        assert!(!output.as_object().unwrap().contains_key("next_agent"));
        assert!(!output.as_object().unwrap().contains_key("workflow_complete"));
    }

    #[tokio::test]
    async fn test_research_agent_invalid_input() {
        let agent = ResearchAgent::new();
        let input = json!({"invalid": "input"});

        let output = agent.process(input).await.unwrap();

        // Verify error response
        assert_eq!(output["error_code"], "INVALID_INPUT");
        assert!(output["details"].is_string());
    }
}
```

### Integration Testing

Use the V2 routing test infrastructure:

```rust
#[tokio::test]
async fn test_research_agent_in_workflow() {
    let transport = Arc::new(MockTransport::new());
    let registry = create_test_registry();
    let router = Arc::new(AlwaysCompleteRouter);

    let (pipeline, _) = create_test_pipeline(router, registry, 10);

    let task = create_test_task("research", json!({
        "query": "Test query",
        "sources": ["test.md"],
        "max_findings": 3
    }));

    let result = pipeline.process_with_routing(task, json!({})).await;

    assert!(result.is_ok());
    // Verify research agent produced valid output
}
```

---

## JSON Schema Generation

All agent outputs should have generated JSON schemas using `schemars`:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResearchOutput {
    pub findings: Vec<Finding>,
    pub sources_reviewed: usize,
    pub completion_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Finding {
    pub fact: String,
    pub source: String,
    pub confidence: f32,
    pub quote: Option<String>,
}

// Auto-generate schema at compile time
pub fn research_output_schema() -> schemars::schema::RootSchema {
    schemars::schema_for!(ResearchOutput)
}
```

This ensures schemas never drift from implementation.

---

## FAQ

**Q: Can my agent call other agents?**
A: No. Agents are isolated workers. The Router decides workflow progression.

**Q: Can my agent see the workflow history?**
A: No. Agents only see their input. Routers see the full workflow context.

**Q: What if my agent needs information from a previous step?**
A: The router should include necessary context in the agent's input based on workflow history.

**Q: Can my agent suggest which agent should run next?**
A: No. That's a routing decision. Your agent should focus on doing its work well, and the router will decide what happens next.

**Q: What if my domain overlaps with another agent?**
A: Define clear boundaries in your system prompt. If overlap is unavoidable, consider merging agents or splitting domains differently.

**Q: How do I handle cases where my agent can't complete the task?**
A: Return a well-defined error code (`DOMAIN_ERROR`) with details. The router will handle the failure appropriately.

**Q: Can my agent return different schemas for different inputs?**
A: No. Use a single, consistent schema. Add optional fields if needed, but the structure should be deterministic.

**Q: Should my agent explain its reasoning?**
A: Yes, but only in a dedicated field (e.g., "explanation"). Don't leak internal chain-of-thought or routing logic.

**Q: How long should my system prompt be?**
A: As concise as possible while being complete. Aim for <500 tokens. Use `cargo xtask token-count` to check.

**Q: Can my agent use external APIs or databases?**
A: Yes, if that's part of your agent's domain (e.g., ResearchAgent fetching from a database). Just ensure the agent remains routing-agnostic.

---

## Further Reading

- [V2 Routing Architecture](./v2_routing_architecture.md) - Overall system design
- [Router Implementation Guide](./router_implementation.md) - For router developers
- [Configuration Reference](./configuration.md) - TOML configuration details
- [Testing Guide](../CLAUDE.md#testing) - General testing best practices
- [Protocol Specification](../SPECIFICATION.md) - Complete 2389 protocol details

---

## Contributing

When contributing new agents or updating existing ones:

1. Follow this guide exactly
2. Run all validation checks: `cargo xtask validate-prompt`
3. Include unit tests for your agent
4. Update this document if you discover new patterns or anti-patterns
5. Use semantic commits: `docs(prompts): add example for data analysis agent`

**Questions or feedback?** Open an issue or discussion on the GitHub repository.

---

**Document Version:** 1.0
**Last Updated:** 2025-10-09
**Related PRs:** [PR #5 - Agent System Prompt Guidelines](#)
