# Agent Configuration Examples

This directory contains example agent configurations demonstrating both good practices and common anti-patterns.

## Directory Structure

```
examples/
├── README.md                    # This file
├── complete_configs/            # Full agent.toml files (USE THESE!)
│   ├── good_research_agent.toml      # ✅ Proper research agent
│   ├── good_writer_agent.toml        # ✅ Proper writer agent
│   ├── bad_routing_aware_agent.toml  # ❌ Anti-pattern: routing decisions
│   ├── bad_super_agent.toml          # ❌ Anti-pattern: no domain focus
│   └── bad_stateful_agent.toml       # ❌ Anti-pattern: maintains state
└── prompts/                     # Conceptual prompt examples (JSON)
    ├── research_agent.json           # Illustrative: shows schemas
    ├── writer_agent.json             # Illustrative: shows schemas
    ├── routing_aware_agent.json      # Illustrative: anti-pattern
    ├── stateful_agent.json           # Illustrative: anti-pattern
    └── super_agent.json              # Illustrative: anti-pattern
```

## How to Use These Examples

### 1. Complete Configurations (START HERE!)

The `complete_configs/` directory contains **actual TOML files** you can use as templates:

```bash
# Copy a good example as your starting point
cp docs/examples/complete_configs/good_research_agent.toml agent.toml

# Edit for your use case
vim agent.toml

# Run your agent
cargo run -- run agent.toml
```

#### ✅ Good Examples

- **`good_research_agent.toml`** - Research and information extraction
  - Shows: Domain-focused system prompt, clear I/O schemas, proper error handling
  - Use for: Any agent that gathers and analyzes information

- **`good_writer_agent.toml`** - Content creation and writing
  - Shows: Single responsibility, no routing logic, deterministic output
  - Use for: Any agent that generates text or documentation

#### ❌ Bad Examples (Learn What NOT to Do)

- **`bad_routing_aware_agent.toml`** - Agent making routing decisions
  - Problem: Agent knows about other agents and makes workflow decisions
  - Fix: Remove routing logic, let Router handle workflow progression

- **`bad_super_agent.toml`** - Jack-of-all-trades agent
  - Problem: Tries to do everything, no focused domain expertise
  - Fix: Split into multiple focused agents (research, write, edit)

- **`bad_stateful_agent.toml`** - Agent maintaining state across calls
  - Problem: References previous invocations, breaks statelessness
  - Fix: Make agent fully self-contained, Router provides context if needed

### 2. Prompt Schemas (Reference Only)

The `prompts/` directory contains **JSON documents** that illustrate:
- What input/output schemas look like
- How agent responses should be structured
- Conceptual examples of good vs bad patterns

These are **NOT** configuration files - they're documentation artifacts showing the structure of data that flows through agents.

## Key Principles

### ✅ DO: Write Routing-Agnostic Agents

```toml
[llm]
system_prompt = """
You are the ResearchAgent v1.
Your sole purpose is to find and summarize information.

You DO NOT make routing decisions, delegate to other agents,
or see workflow history.

Expected input: { "query": "string", "sources": ["string"] }
Output: { "findings": [...], "sources_reviewed": 5 }
"""
```

### ❌ DON'T: Include Routing Logic

```toml
[llm]
system_prompt = """
You perform research and then decide which agent should handle the next step.

You can forward work to:
- WriterAgent for content creation
- EditorAgent for improvements

Return: { "findings": [...], "next_agent": "writer-agent" }
"""
# ^^^ BAD: Agent making routing decisions!
```

## Understanding System Prompts

The `system_prompt` field in your `agent.toml` defines your agent's **behavior**:

```toml
[llm]
provider = "openai"
model = "gpt-4o-mini"
api_key_env = "OPENAI_API_KEY"

# This prompt defines what your agent DOES:
system_prompt = """
You are the DataAnalyzerAgent v1.
You analyze datasets and produce statistical summaries.

Input: { "data": [...], "metrics": ["mean", "median"] }
Output: { "summary": {...}, "visualizations": [...] }

You DO NOT make routing decisions or mention other agents.
"""
```

### Anatomy of a Good System Prompt

1. **Role Declaration** - Who is this agent?
   ```
   "You are the ResearchAgent v1. Your sole purpose is..."
   ```

2. **Domain Scope** - What does it handle?
   ```
   "Your expertise: Information extraction, source evaluation"
   "You ONLY handle: Document analysis, citation"
   "You IGNORE: Writing, editing, publishing"
   ```

3. **Routing Disclaimer** - Establish boundaries
   ```
   "You DO NOT make routing decisions, delegate to other agents,
   or see workflow history."
   ```

4. **Input Schema** - What does it expect?
   ```json
   { "query": "string", "sources": ["string"], "max_findings": "int" }
   ```

5. **Output Schema** - What does it produce?
   ```json
   { "findings": [...], "sources_reviewed": "int" }
   ```

6. **Error Handling** - How does it fail gracefully?
   ```json
   { "error_code": "INVALID_INPUT", "details": "..." }
   ```

7. **Tone & Style** - How should it communicate?
   ```
   "Be factual and precise. Include direct quotes when available."
   ```

## Common Mistakes

### Mistake 1: Routing in System Prompt

```toml
# ❌ BAD
system_prompt = """
After completing your analysis, decide if the workflow is complete
or which agent should handle the next step.
"""

# ✅ GOOD
system_prompt = """
Complete your analysis and return the results.
The Router will decide what happens next.
"""
```

### Mistake 2: Knowing About Other Agents

```toml
# ❌ BAD
system_prompt = """
If you need more information, forward to ResearchAgent.
If you need writing, forward to WriterAgent.
"""

# ✅ GOOD
system_prompt = """
Work with the information provided.
If input is insufficient, return an error.
"""
```

### Mistake 3: Vague Domain Boundaries

```toml
# ❌ BAD
system_prompt = """
You are a helpful agent that assists with various tasks.
"""

# ✅ GOOD
system_prompt = """
You are the CodeReviewAgent v1.
You ONLY analyze code for quality and security issues.
You do NOT write code, run tests, or fix bugs.
"""
```

## Testing Your Agent

After creating your agent configuration:

```bash
# 1. Validate configuration syntax
cargo run -- validate agent.toml

# 2. Test with a simple task
cargo run --bin inject-message -- \
  --agent-id your-agent-id \
  --message '{"query": "test", "sources": ["test.md"]}'

# 3. Monitor agent output
cargo run --bin mqtt-monitor -- --agent-id your-agent-id

# 4. Check that output schema is correct
# Agent should return JSON matching your documented schema
```

## Next Steps

1. **Read the Guidelines**: See [agent_system_prompts.md](../agent_system_prompts.md) for complete documentation
2. **Copy a Good Example**: Start with `good_research_agent.toml` or `good_writer_agent.toml`
3. **Customize for Your Domain**: Adjust the system prompt for your use case
4. **Test Thoroughly**: Verify your agent produces valid output
5. **Avoid Anti-Patterns**: Review bad examples to know what NOT to do

## Questions?

See the [FAQ section](../agent_system_prompts.md#faq) in the main guidelines document.
