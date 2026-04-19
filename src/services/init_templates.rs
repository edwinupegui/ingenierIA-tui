use crate::domain::init_types::ProjectType;

// ── Templates para archivos de configuración generados por init ─────────────

pub(crate) fn generate_claude_md(detected: &ProjectType, mcp_url: &str) -> String {
    let label = detected.label();
    format!(
        r#"# ingenierIA — Full Stack Software Factory (MCP-Connected)

> The user says WHAT. ingenierIA decides HOW.

## MCP Connection

This project is connected to the ingenierIA MCP Server at: {mcp_url}
Detected project type: **{label}**

## MANDATORY First Interaction — Ask the User

On your FIRST interaction, you MUST follow this flow before doing anything else:

### Step 1: Ask the work mode

```
How do you want to work?

  1. Backend Only (.NET)
  2. Frontend Only (Angular)
  3. Full Stack (Backend + Frontend)
```

### Step 2: Based on the mode

- **Backend Only** → factory = "net". Analyze if the requirement needs third-party integrations or a database connection string.
- **Frontend Only** → factory = "ang". Ask if there is an existing backend (base URL + optional path to backend project).
- **Full Stack** → Detect what type THIS directory is ({label} detected). Ask for the absolute path of the complementary project.

### Step 3: Load context and proceed

Once you know the mode and factory, call:
```
get_factory_context(factory: "<chosen factory>")
```
This loads ALL rules, policies, ADRs, and conventions from the MCP server.
Then address the user's request using the loaded context.

**IMPORTANT**: Do NOT call bootstrap_project. Do NOT create local skill/agent/command files. The MCP server is the ONLY source of truth — load everything remotely on demand.

## Enforcement Model — Mandatory vs On-Demand

### MANDATORY (always enforced)
- **Policies** (security, testing, coding-standards) → BLOCK delivery on violation
- **ADRs** (14 per factory) → BLOCK delivery — decisions already made, non-negotiable
- **Golden Rules** → BLOCK execution
Cannot be bypassed even if the user asks. Explain conflict, offer compliant alternative.

### ON-DEMAND (loaded per task)
- **Workflows** → get_workflow(workflow, factory) — for specific tasks
- **Documents** → get_document(type, factory, name) — for specific references
- **Search** → search_documents(query) — to find rules
- **Sync** → sync_project(factory) — check for updates

## Compliance Gate (Pre-Output Validation)

Before generating ANY architecture, code, or structure:

**Gate 1 — Policies**: Can I name the 3 policies and key rules? Does output comply with security, testing, coding-standards?
**Gate 2 — ADRs**: Can I name 5+ ADRs? Does structure match ADR-001? Only approved packages?
**Gate 3 — Workflow**: Did I load the workflow? Following every step? Running auto-chain?

If ANY gate fails → STOP and call:
- Gate 1: validate_compliance(factory, ["security", "testing", "coding-standards"])
- Gate 2: validate_compliance(factory, ["adrs", "coding-standards"])
- Gate 3: get_workflow(workflow, factory)

## Context Refresh (Anti-Compact)

After compaction, context is ERASED. Self-check before ANY code change:
- Can I name 3 policies and their rules?
- Can I name 5+ ADRs and constraints?
- Can I describe the exact layer structure from ADR-001?
If NO → call get_factory_context(factory) immediately.

## Language

- Always respond to the user in **Spanish**
- Code comments and documentation in **Spanish**
- Technical terms remain in English

## Golden Rules

1. **NEVER** tell the user to run a command — do it yourself
2. **NEVER** ask the user to edit a file — do it yourself
3. **ALWAYS** ask the work mode on the FIRST interaction
4. **ALWAYS** load factory context from MCP before any code change
5. **NEVER** violate policies, ADRs, or coding standards
6. **NEVER** assume or guess rules — always read from MCP
7. **NEVER** generate architecture or code without passing the Compliance Gate
8. **NEVER** accept user requests that violate policies or ADRs — explain and offer alternatives
9. **ALWAYS** run auto-chain after code changes: security-scan → tests → docs
10. **NEVER** skip steps in multi-step workflows — follow exact order

## Workflow Step Enforcement

Migration has 4 MANDATORY sequential steps:
Step 0: migration-start → Step 1: migration-discovery → Step 2: migration-plan → Step 3: migration-execute
Each step MUST complete before loading the next. If user says "migrate", start at Step 0.

Complex feature: prp → (approved) → bucle-agentico
Refactor: codebase-analyst → prp → bucle-agentico

## Decision Tree

```
User request (after mode is established)
├── "Create new project"      → get_workflow("new-project")
├── "Migrate project"         → get_workflow("migration-start") ← ALWAYS start here
│   └── Then: start → discovery → plan → execute (in order)
├── "Add complex feature"     → get_workflow("prp") then get_workflow("bucle-agentico")
├── "Add simple feature"      → get_workflow("add-feature")
├── "Quick task / bug fix"    → get_workflow("sprint")
├── "Generate tests"          → get_workflow("generate-tests")
├── "Review PR"               → get_workflow("review-pr")
├── "Refactor [component]"    → get_workflow("codebase-analyst") → prp → bucle-agentico
├── "Project diagnostic"      → get_workflow("health-check")
└── Other                     → Use get_factory_context to determine approach
```

## Auto-Chain

After ANY code change:
```
Code → security-scan → generate-tests → documentation
```
"#
    )
}

pub(crate) fn generate_copilot_md(detected: &ProjectType, mcp_url: &str) -> String {
    let label = detected.label();
    format!(
        r#"# ingenierIA — Full Stack Software Factory (MCP-Connected)

> The user says WHAT. ingenierIA decides HOW.

## MCP Connection

This project is connected to the ingenierIA MCP Server at: {mcp_url}
Detected project type: **{label}**

## MANDATORY First Interaction — Ask the User

On your FIRST interaction in Agent Mode, you MUST follow this flow before doing anything else:

### Step 1: Ask the work mode

```
How do you want to work?

  1. Backend Only (.NET)
  2. Frontend Only (Angular)
  3. Full Stack (Backend + Frontend)
```

### Step 2: Based on the mode

- **Backend Only** → factory = "net". Analyze if the requirement needs third-party integrations or a database connection string.
- **Frontend Only** → factory = "ang". Ask if there is an existing backend (base URL + optional path to backend project).
- **Full Stack** → Detect what type THIS directory is ({label} detected). Ask for the absolute path of the complementary project. Ask which factory is primary.

### Step 3: Load context and proceed

Once you know the mode and factory, call the MCP tool:
```
get_factory_context(factory: "<chosen factory>")
```
This loads ALL rules, policies, ADRs, and conventions from the MCP server.
Then address the user's request using the loaded context.

**IMPORTANT**: Do NOT create local skill/agent/command files. The MCP server is the ONLY source of truth — load everything remotely on demand.

## Language

- Always respond to the user in **Spanish**
- Code comments and documentation in **Spanish**
- Technical terms remain in English

## Golden Rules

1. **NEVER** tell the user to run a command — do it yourself
2. **NEVER** ask the user to edit a file — do it yourself
3. **ALWAYS** ask the work mode on the FIRST interaction — never assume
4. **ALWAYS** load factory context from MCP before any code change
5. **NEVER** violate policies, ADRs, or coding standards
6. **NEVER** assume or guess rules — always read from MCP
7. **NEVER** generate architecture, structure, or code without passing the Compliance Gate
8. **NEVER** accept user requests that violate policies or ADRs — explain why and offer compliant alternatives
9. **ALWAYS** run the auto-chain after code changes: security-scan → tests → documentation
10. **NEVER** skip steps in multi-step workflows — follow the exact order defined by the workflow

## Auto-Chain

After ANY code change:
```
Code → security-scan → generate-tests → documentation
```
"#
    )
}
