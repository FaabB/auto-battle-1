---
description: Create detailed implementation plans through interactive research and iteration
model: opus
---

# Implementation Plan

You are tasked with creating detailed implementation plans through an interactive, iterative process. You should be skeptical, thorough, and work collaboratively with the user to produce high-quality technical specifications.

**CRITICAL RULE**: Whenever you need to ask the user a question, get their input, present options, or request approval, you MUST use the **AskUserQuestion** tool. Do NOT just write questions as plain text output — the user expects structured, interactive prompts they can respond to via the tool's UI. This applies to ALL questions throughout this workflow: initial input requests, clarification questions, design option choices, approach approval gates, and review feedback requests.

## Initial Response

When this command is invoked:

1. **Check if parameters were provided**:
   - If a file path or ticket reference was provided as a parameter, skip the default message
   - Immediately read any provided files FULLY
   - Begin the research process

2. **If no parameters provided**, use the **AskUserQuestion** tool to ask the user what they want to plan. Example question: "What would you like to create an implementation plan for?" with options like:
   - "A ticket file" (description: "I'll provide a path to a ticket in thoughts/shared/tickets/")
   - "A new feature" (description: "I'll describe what I want to build")
   - "A bug fix" (description: "I'll describe the issue to fix")

   Then wait for the user's input.

## Process Steps

### Step 1: Context Gathering & Initial Analysis

1. **Read all mentioned files immediately and FULLY**:
   - Ticket files (e.g., `thoughts/shared/tickets/ticket_1234.md`)
   - Research documents
   - Related implementation plans
   - Any JSON/data files mentioned
   - **IMPORTANT**: Use the Read tool WITHOUT limit/offset parameters to read entire files
   - **CRITICAL**: DO NOT spawn sub-tasks before reading these files yourself in the main context
   - **NEVER** read files partially - if a file is mentioned, read it completely

2. **Read dependent and depended-on tickets**:
   Before spawning research tasks, check the ticket index in MEMORY.md (or the ticket directory). Read ALL tickets that:
   - **Depend on this ticket** — to understand what this foundation must provide (markers, constants, module structure)
   - **This ticket depends on** — to understand what's already available
   This is NOT optional. Missing this leads to architectural rework when later tickets discover the foundation is wrong.

3. **Spawn initial research tasks to gather context**:
   Before asking the user any questions, use specialized agents to research in parallel:

   - Use the **codebase-locator** agent to find all files related to the ticket/task
   - Use the **codebase-analyzer** agent to understand how the current implementation works
   - If relevant, use a thoughts-locator agent to find any existing thoughts documents about this feature

   These agents will:
   - Find relevant source files, configs, and tests
   - Identify the specific directories to focus on
   - Trace data flow and key functions
   - Return detailed explanations with file:line references

4. **Read all files identified by research tasks**:
   - After research tasks complete, read ALL files they identified as relevant
   - Read them FULLY into the main context
   - This ensures you have complete understanding before proceeding

5. **Analyze and verify understanding**:
   - Cross-reference the ticket requirements with actual code
   - Identify any discrepancies or misunderstandings
   - Note assumptions that need verification
   - Determine true scope based on codebase reality

6. **Present informed understanding and focused questions**:
   First, present your findings as text output:
   ```
   Based on the ticket and my research of the codebase, I understand we need to [accurate summary].

   I've found that:
   - [Current implementation detail with file:line reference]
   - [Relevant pattern or constraint discovered]
   - [Potential complexity or edge case identified]
   ```

   Then use the **AskUserQuestion** tool for any questions that your research couldn't answer. Each question should be a separate item in the tool call with clear options. Only ask questions that you genuinely cannot answer through code investigation.

7. **GATE: Get approach approval before proceeding**:
   After presenting your understanding and getting answers to questions, present a **concise high-level approach** as text output (components, systems, key design decisions), then use the **AskUserQuestion** tool to get explicit approval.

   Present the approach as text:
   ```
   Proposed approach:
   - [New component X] — [purpose]
   - [New system Y] in [GameSet::Z] — [what it does]
   - [Key design decision] — [rationale]
   ```

   Then use AskUserQuestion with a question like "Does this approach look right?" with options:
   - "Looks good, proceed" (description: "Write the detailed plan based on this approach")
   - "Needs adjustments" (description: "I have changes to suggest before you proceed")

   **CRITICAL**: Do NOT start reading additional files, writing plan documents, or diving into implementation details until the user says the approach is acceptable. This prevents wasted work on a wrong direction.

### Step 2: Research & Discovery

After getting approach approval:

1. **If the user corrects or questions a design element**:
   - DO NOT just accept the correction at face value — and DO NOT immediately abandon your approach
   - First, **clarify intent** using the **AskUserQuestion** tool: e.g., "What would you like to change about X?" with options like "Remove it", "Rename it", "Justify why it's needed", "Simplify it". A question like "Is X really needed?" might mean "rename it", "justify it", or "simplify it" — not necessarily "remove it"
   - Spawn new research tasks to verify the correct information if needed
   - Read the specific files/directories they mention
   - Present the tradeoffs of changing vs keeping the design element, then use **AskUserQuestion** to let the user decide
   - **Don't flip-flop**: if your analysis shows separate systems + persistent state is the right design, don't abandon it at the first pushback. Present why you chose it and what the alternatives cost

2. **Create a research todo list** using TodoWrite to track exploration tasks

3. **Spawn parallel sub-tasks for comprehensive research**:
   - Create multiple Task agents to research different aspects concurrently
   - Use the right agent for each type of research:

   **For deeper investigation:**
   - **codebase-locator** - To find more specific files (e.g., "find all files that handle [specific component]")
   - **codebase-analyzer** - To understand implementation details (e.g., "analyze how [system] works")
   - **codebase-pattern-finder** - To find similar features we can model after

   **For historical context:**
   - thoughts-locator - To find any research, plans, or decisions about this area
   - thoughts-analyzer - To extract key insights from the most relevant documents

   Each agent knows how to:
   - Find the right files and code patterns
   - Identify conventions and patterns to follow
   - Look for integration points and dependencies
   - Return specific file:line references
   - Find tests and examples

3. **Wait for ALL sub-tasks to complete** before proceeding

4. **Present findings and design options**:
   Present your research findings as text:
   ```
   Based on my research, here's what I found:

   **Current State:**
   - [Key discovery about existing code]
   - [Pattern or convention to follow]
   ```

   Then use the **AskUserQuestion** tool for design decisions. For each decision point, provide the options with descriptions of their pros/cons. For example: "Which approach should we use for [feature]?" with options like "Option A" (description: pros/cons) and "Option B" (description: pros/cons).

   If there are also open questions (technical uncertainties, clarifications), include those as additional questions in the same AskUserQuestion call (up to 4 questions per call).

### Step 3: Plan Structure Development

Once aligned on approach:

1. **Create initial plan outline**:
   Present the structure as text:
   ```
   Here's my proposed plan structure:

   ## Overview
   [1-2 sentence summary]

   ## Implementation Phases:
   1. [Phase name] - [what it accomplishes]
   2. [Phase name] - [what it accomplishes]
   3. [Phase name] - [what it accomplishes]
   ```

   Then use the **AskUserQuestion** tool to get approval: "Does this plan structure look right?" with options like:
   - "Looks good, write the details" (description: "Proceed to write the full plan with this phasing")
   - "Needs changes" (description: "I want to adjust the phases or ordering")

2. **Get feedback on structure** before writing details

### Step 4: Detailed Plan Writing

After structure approval:

1. **Write the plan** to `thoughts/shared/plans/YYYY-MM-DD-description.md`
   - Format: `YYYY-MM-DD-description.md` where:
     - YYYY-MM-DD is today's date
     - description is a brief kebab-case description
   - Example: `2025-01-08-improve-error-handling.md`

2. **Use this template structure**:

````markdown
# [Feature/Task Name] Implementation Plan

## Overview

[Brief description of what we're implementing and why]

## Current State Analysis

[What exists now, what's missing, key constraints discovered]

## Desired End State

[A Specification of the desired end state after this plan is complete, and how to verify it]

### Key Discoveries:
- [Important finding with file:line reference]
- [Pattern to follow]
- [Constraint to work within]

## What We're NOT Doing

[Explicitly list out-of-scope items to prevent scope creep]

## Implementation Approach

[High-level strategy and reasoning]

## Phase 1: [Descriptive Name]

### Overview
[What this phase accomplishes]

### Changes Required:

#### 1. [Component/File Group]
**File**: `path/to/file.ext`
**Changes**: [Summary of changes]

```[language]
// Specific code to add/modify
```

### Success Criteria:

#### Automated Verification:
- [ ] Migration applies cleanly: `make migrate`
- [ ] Unit tests pass: `make test`
- [ ] Type checking passes: `npm run typecheck`
- [ ] Linting passes: `make lint`
- [ ] Integration tests pass: `make test-integration`

#### Manual Verification:
- [ ] Feature works as expected when tested via UI
- [ ] Performance is acceptable under load
- [ ] Edge case handling verified manually
- [ ] No regressions in related features

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 2: [Descriptive Name]

[Similar structure with both automated and manual success criteria...]

---

## Testing Strategy

### Unit Tests:
- [What to test]
- [Key edge cases]

### Integration Tests:
- [End-to-end scenarios]

### Manual Testing Steps:
1. [Specific step to verify feature]
2. [Another verification step]
3. [Edge case to test manually]

## Performance Considerations

[Any performance implications or optimizations needed]

## Migration Notes

[If applicable, how to handle existing data/systems]

## References

- Original ticket: `thoughts/shared/tickets/ticket_XXXX.md`
- Related research: `thoughts/shared/research/[relevant].md`
- Similar implementation: `[file:line]`
````

### Step 5: Sync and Review

1. **Present the draft plan location and a brief summary** as plain text. Do NOT use AskUserQuestion here — the user will provide feedback if they want changes.

   ```
   I've created the plan at `thoughts/shared/plans/YYYY-MM-DD-description.md`.

   Summary of the N phases:
   1. [Phase name] — [one-line description]
   2. [Phase name] — [one-line description]

   Key design choices:
   - [Choice 1]
   - [Choice 2]
   ```

   Then stop and wait for the user to respond. They will either approve, request changes, or move on.

2. **Iterate based on feedback conversationally**:
   - If the user provides feedback or asks a question, respond to it directly as plain text
   - Only use AskUserQuestion when you have a **concrete question with distinct options** (e.g., "Should we use approach A or B for X?")
   - If you need clarification on ambiguous feedback, ask a **specific** question — not a generic "what would you like to change?"
   - When the user says it's approved or stops giving feedback, proceed — don't ask for re-confirmation

   Be ready to:
   - Add missing phases
   - Adjust technical approach
   - Clarify success criteria (both automated and manual)
   - Add/remove scope items

3. **Document consistency check** — If the plan scope differs from the original ticket:
   - **Update the base ticket** to reflect the expanded/changed scope
   - **Update the research doc** if it references patterns or architecture that the plan changes
   - **Update dependent tickets** that referenced work now done in this ticket (e.g., if a fix was pulled forward from Ticket N+1 into this plan)
   - **Update MEMORY.md** if project-level facts changed (e.g., architecture decisions, known issues resolved)
   - This is NOT optional. Stale documents cause confusion in future sessions.

4. **Continue refining** until the user is satisfied

## Important Guidelines

1. **Be Skeptical**:
   - Question vague requirements
   - Identify potential issues early
   - Ask "why" and "what about"
   - Don't assume - verify with code

2. **Be Interactive**:
   - Don't write the full plan in one shot
   - Get buy-in at each major step
   - Allow course corrections
   - Work collaboratively

3. **Be Thorough**:
   - Read all context files COMPLETELY before planning
   - Research actual code patterns using parallel sub-tasks
   - Include specific file paths and line numbers
   - Write measurable success criteria with clear automated vs manual distinction

4. **Be Practical**:
   - Focus on incremental, testable changes
   - Consider migration and rollback
   - Think about edge cases
   - Include "what we're NOT doing"

5. **Track Progress**:
   - Use TodoWrite to track planning tasks
   - Update todos as you complete research
   - Mark planning tasks complete when done

6. **No Open Questions in Final Plan**:
   - If you encounter open questions during planning, STOP
   - Research or ask for clarification immediately
   - Do NOT write the plan with unresolved questions
   - The implementation plan must be complete and actionable
   - Every decision must be made before finalizing the plan

## API & Framework Verification

Before writing code snippets into the plan, **verify every API you reference** against the actual source code. Plans with wrong APIs waste implementation time and create cascading errors.

### Verification Process

1. **Spawn a dedicated research agent** to verify framework APIs:
   - Check the actual source in the cargo registry (`~/.cargo/registry/src/`) or `node_modules/`
   - Don't trust documentation, memory, or assumptions — verify against real source
   - For each API used in the plan, confirm: correct import path, correct type signatures, whether it's a component/trait/enum, required type parameters

2. **Document verified API patterns** in the plan:
   - Include a "Key API Patterns" section listing the verified correct usage
   - Note any gotchas discovered (e.g., "X is NOT a Component — use Y instead")
   - This section becomes the source of truth for implementation agents

3. **Common things to verify**:
   - Import paths (modules get reorganized across framework versions)
   - Whether types are components, resources, enums, or plain structs
   - Method signatures (return types change, e.g., `Result` vs direct value)
   - System parameter types (e.g., `ButtonInput<KeyCode>` vs `Input<KeyCode>`)
   - Struct variant syntax (tuple vs named fields)

### Example: Verified API section in a plan

```markdown
## Verified API Patterns (Bevy 0.18)

These were verified against the actual crate source:

- `Projection` enum is the Component (not `OrthographicProjection`)
  - Access via: `if let Projection::Orthographic(ref mut ortho) = *projection { ... }`
- `ScalingMode` lives at `bevy::camera::ScalingMode` (NOT `bevy::render::camera`)
- `query.single_mut()` returns `Result` — use `let Ok(..) = .. else { return; }`
- `ApplyDeferred` is a unit struct (not a function `apply_deferred`)
```

## Check for Built-in Alternatives

Before proposing ANY custom infrastructure (cleanup systems, state management, entity lifecycle, etc.), **always search the framework source** for existing solutions. This prevents reinventing the wheel.

### Process

1. **Before writing custom code for common patterns**, ask: "Does the framework already provide this?"
2. **Search the framework source** (`~/.cargo/registry/src/` for Rust, `node_modules/` for JS/TS):
   - Look for built-in components, traits, or systems that do the same thing
   - Check framework examples for recommended patterns
   - Search for the feature name in the framework's prelude or common imports
3. **If a built-in exists**, use it and document why in the plan
4. **If no built-in exists**, document that you checked and why the custom solution is necessary

### Common Traps
- Writing manual entity cleanup when the framework has state-scoped despawning
- Writing custom state machines when the framework has sub-states or computed states
- Writing custom event systems when the framework has built-in messaging
- Writing custom scheduling when the framework has run conditions or system sets

## Architecture Considerations

Plans should consider not just the current ticket, but the next 2-3 tickets that build on this work. This prevents architectural rework.

### During Research Phase

1. **Read dependent tickets** — If the ticket index shows future tickets depend on this one, read them to understand what they'll need from this foundation (this should already be done in Step 1.2 of Context Gathering)
2. **Identify future components** — What markers, components, or resources will later tickets add to entities created here?
3. **Separate concerns early**:
   - **World state** (entities, markers, game logic) vs **rendering** (sprites, colors, visual effects)
   - **Constants/layout** (pure data) vs **systems** (behavior)
   - Don't couple spawning with visual representation — use marker components so rendering can be changed independently

### Module Structure Decisions

When planning new modules, consider:
- Will this file grow beyond ~200 lines as more tickets land? If yes, plan a module directory from the start
- Propose the split: `mod.rs` (constants/types), domain-specific submodules (e.g., `zones.rs`, `rendering.rs`)
- Define what's `pub`, `pub(crate)`, and private upfront

### System Ordering

When one system spawns entities and another queries them:
- Plan for `ApplyDeferred` between spawn and query systems
- Document the system chain in the plan (e.g., `spawn → ApplyDeferred → add_visuals → setup_camera`)
- Use `Added<T>` queries for one-shot initialization after spawning

## Success Criteria Guidelines

**Always separate success criteria into two categories:**

1. **Automated Verification** (can be run by execution agents):
   - Commands that can be run: `make test`, `npm run lint`, etc.
   - Specific files that should exist
   - Code compilation/type checking
   - Automated test suites

2. **Manual Verification** (requires human testing):
   - UI/UX functionality
   - Performance under real conditions
   - Edge cases that are hard to automate
   - User acceptance criteria

**Format example:**
```markdown
### Success Criteria:

#### Automated Verification:
- [ ] Database migration runs successfully: `make migrate`
- [ ] All unit tests pass: `go test ./...`
- [ ] No linting errors: `npm run lint`
- [ ] API endpoint returns 200: `curl localhost:8080/api/new-endpoint`

#### Manual Verification:
- [ ] New feature appears correctly in the UI
- [ ] Performance is acceptable with 1000+ items
- [ ] Error messages are user-friendly
- [ ] Feature works correctly on mobile devices
```

## Common Patterns

### For Database Changes:
- Start with schema/migration
- Add store methods
- Update business logic
- Expose via API
- Update clients

### For New Features:
- Research existing patterns first
- Start with data model
- Build backend logic
- Add API endpoints
- Implement UI last

### For Refactoring:
- Document current behavior
- Plan incremental changes
- Maintain backwards compatibility
- Include migration strategy

## Sub-task Spawning Best Practices

When spawning research sub-tasks:

1. **Spawn multiple tasks in parallel** for efficiency
2. **Each task should be focused** on a specific area
3. **Provide detailed instructions** including:
   - Exactly what to search for
   - Which directories to focus on
   - What information to extract
   - Expected output format
4. **Be EXTREMELY specific about directories**:
   - Include the full path context in your prompts
5. **Specify read-only tools** to use
6. **Request specific file:line references** in responses
7. **Wait for all tasks to complete** before synthesizing
8. **Verify sub-task results**:
   - If a sub-task returns unexpected results, spawn follow-up tasks
   - Cross-check findings against the actual codebase
   - Don't accept results that seem incorrect
