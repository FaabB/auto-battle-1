---
description: Create implementation plans with branch and draft PR setup
model: opus
---

# Implementation Plan with Draft PR

You are tasked with creating detailed implementation plans through an interactive, iterative process. This command extends `/create_plan` by creating a worktree upfront so the entire workflow (planning, implementation, PR) happens in one isolated branch from the start.

**CRITICAL RULE**: Whenever you need to ask the user a question, get their input, present options, or request approval, you MUST use the **AskUserQuestion** tool. Do NOT just write questions as plain text output — the user expects structured, interactive prompts they can respond to via the tool's UI. This applies to ALL questions throughout this workflow: initial input requests, clarification questions, design option choices, approach approval gates, and review feedback requests.

## Initial Response

When this command is invoked:

1. **Check if parameters were provided**:
   - If a file path or ticket reference was provided as a parameter, skip the default message
   - If a **Linear ticket identifier** was provided (e.g., `GAM-8`, `LIN-123`), use the `mcp__plugin_linear_linear__get_issue` tool to fetch the ticket details. Store the identifier and URL for use in the plan's References section.
   - Immediately read any provided files FULLY
   - Begin the research process

2. **If no parameters provided**, use the **AskUserQuestion** tool to ask the user what they want to plan. Example question: "What would you like to create an implementation plan for?" with options like:
   - "A Linear ticket" (description: "I'll provide a Linear issue identifier like GAM-8")
   - "A ticket file" (description: "I'll provide a path to a ticket in thoughts/shared/tickets/")
   - "A new feature" (description: "I'll describe what I want to build")
   - "A bug fix" (description: "I'll describe the issue to fix")

   Then wait for the user's input.

## Worktree Setup (before planning begins)

Once you know what you're working on (ticket identifier or task description):

1. **Determine branch name**:
   - If a Linear ticket is associated, use: `{identifier-lowercase}-short-description` (e.g., `gam-54-snap-to-mesh`)
   - If no ticket, use: `plan-short-description`

2. **Create a worktree for isolated work**:
   Use the `EnterWorktree` tool with the branch name:
   ```
   EnterWorktree(name: "<branch-name>")
   ```
   This creates a worktree at `.claude/worktrees/<branch-name>/` with a new branch based on HEAD. The session's working directory switches to the worktree automatically.

   **Why upfront?** The plan file, implementation, and PR all live in one worktree from the start. No need to copy files later. The main working tree stays clean, and multiple tickets can be in progress simultaneously.

3. **Continue with planning** — all file reads, research, and plan writing happen inside the worktree (which has the full repo contents).

## Planning Process

Follow the exact same planning process as `/create_plan`:

### Step 1: Context Gathering & Initial Analysis

1. **Read all mentioned files immediately and FULLY**:
   - Ticket files (e.g., `thoughts/shared/tickets/ticket_1234.md`)
   - Research documents
   - Related implementation plans
   - Any JSON/data files mentioned
   - **Linear tickets**: If a Linear issue identifier was provided, fetch it with `mcp__plugin_linear_linear__get_issue` (with `includeRelations: true`) and read any linked local ticket files. Also fetch comments with `mcp__plugin_linear_linear__list_comments` for additional context.
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
   - **If based on a Linear ticket**, include the ticket identifier in the filename:
     - Format: `YYYY-MM-DD-{identifier}-description.md`
     - Example: `2025-01-08-GAM-8-fortresses-damageable.md`
   - Example (no Linear ticket): `2025-01-08-improve-error-handling.md`

2. **Use the standard plan template structure** (same as `/create_plan`):
   - Overview, Current State Analysis, Desired End State, What We're NOT Doing
   - Implementation Approach, Phases with Changes Required and Success Criteria
   - Testing Strategy, Performance Considerations, References

### Step 5: Sync and Review

1. **Present the draft plan location and a brief summary** as plain text. Do NOT use AskUserQuestion here — the user will provide feedback if they want changes.

2. **Iterate based on feedback conversationally**

3. **Document consistency check** — If the plan scope differs from the original ticket, update related documents

4. **Continue refining** until the user is satisfied

---

## Step 6: Commit, Draft PR, and Linear Status (unique to this command)

After the plan is approved (worktree and branch already exist from the setup step):

1. **Commit the plan file and push**:
   ```
   git add thoughts/shared/plans/<plan-file>
   git commit -m "Add implementation plan for <description> (<TICKET-ID>)"
   git push -u origin <branch-name>
   ```

2. **Create a draft PR**:
   Use `gh pr create --draft` with:
   - Title: the plan title or ticket title
   - Body: a summary of the plan phases + link to the plan file

   Format:
   ```
   gh pr create --draft --title "<title> (<TICKET-ID>)" --body "$(cat <<'EOF'
   ## Implementation Plan

   Plan file: `thoughts/shared/plans/<plan-file>`

   ### Phases:
   1. [Phase name] — [one-line description]
   2. [Phase name] — [one-line description]

   ### Linear ticket
   [TICKET-ID](linear-url)
   EOF
   )"
   ```

3. **Update Linear ticket**:
   - Move the ticket to **In Progress** using `mcp__plugin_linear_linear__save_issue` (set `state` to `"In Progress"`)

4. **Report to the user**:
   ```
   Setup complete:
   - Worktree: `.claude/worktrees/<branch-name>/`
   - Branch: `<branch-name>`
   - Draft PR: <pr-url>
   - Linear ticket: moved to In Progress

   Ready for `/implement_plan` (already in the worktree).
   ```

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

## Check for Built-in Alternatives

Before proposing ANY custom infrastructure, **always search the framework source** for existing solutions.

## Architecture Considerations

Plans should consider not just the current ticket, but the next 2-3 tickets that build on this work.

## Sub-task Spawning Best Practices

When spawning research sub-tasks:

1. **Spawn multiple tasks in parallel** for efficiency
2. **Each task should be focused** on a specific area
3. **Provide detailed instructions** including exactly what to search for
4. **Be EXTREMELY specific about directories**
5. **Specify read-only tools** to use
6. **Request specific file:line references** in responses
7. **Wait for all tasks to complete** before synthesizing
8. **Verify sub-task results** against the actual codebase
