---
description: Generate implementation tickets/tasks from research files, documents, or text descriptions
model: opus
---

# Create Tickets

You are tasked with generating well-structured implementation tickets from input sources (research documents, design docs, freeform text, or any combination). You work interactively with the user to produce clear, actionable tickets that can later be used as input for `/create_plan`.

## Initial Response

When this command is invoked:

1. **Check if parameters were provided**:
   - If file paths or text were provided as parameters, skip the default message
   - Immediately read any provided files FULLY
   - Begin the research and analysis process

2. **If no parameters provided**, respond with:
```
I'll help you break down work into implementation tickets. Let me understand what we're working with.

Please provide one or more of:
1. A research document (e.g., `thoughts/shared/research/2026-02-04-feature.md`)
2. Any other file with requirements, specs, or design notes
3. A text description of what needs to be built

I'll analyze the input, research the codebase, and work with you to create well-scoped tickets.

Tip: You can pass files directly: `/create_tickets thoughts/shared/research/my-research.md`
```

Then wait for the user's input.

## Process Steps

### Step 1: Input Ingestion & Initial Analysis

1. **Read all provided files immediately and FULLY**:
   - Research documents from `thoughts/shared/research/`
   - Design documents, specs, or any referenced files
   - Existing ticket files in `thoughts/shared/tickets/` (to avoid duplicates and continue numbering)
   - **IMPORTANT**: Use the Read tool WITHOUT limit/offset parameters to read entire files
   - **CRITICAL**: DO NOT spawn sub-tasks before reading these files yourself in the main context
   - **NEVER** read files partially - if a file is mentioned, read it completely

2. **Check existing tickets**:
   - Read the contents of `thoughts/shared/tickets/` to understand what tickets already exist
   - Determine the next available ticket number to avoid conflicts
   - Note any overlap with existing tickets

3. **Analyze the input** to identify:
   - Major features or systems described
   - Dependencies between components
   - Implicit requirements (things needed but not explicitly stated)
   - Technical complexity and risk areas
   - Natural breakpoints for incremental delivery

4. **Present initial understanding**:
   ```
   I've analyzed your input. Here's what I see:

   **Scope:** [1-2 sentence summary of what needs to be built]

   **Major areas I identified:**
   1. [Area/system 1]
   2. [Area/system 2]
   3. [Area/system 3]

   **Key dependencies:** [How areas relate to each other]

   **Questions before I draft tickets:**
   - [Clarifying question about scope]
   - [Question about priorities]
   - [Question about technical constraints]
   ```

   Only ask questions that genuinely affect ticket breakdown.

### Step 2: Research & Discovery

After getting initial clarifications:

1. **If the input references existing code or systems**:
   - Spawn parallel research agents to understand the current codebase state
   - Use **codebase-locator** to find relevant files
   - Use **codebase-analyzer** to understand existing implementation
   - Use **codebase-pattern-finder** to find patterns to follow

2. **If related thoughts documents might exist**:
   - Use **thoughts-locator** to find relevant research, plans, or decisions
   - Use **thoughts-analyzer** to extract key insights

3. **If the user corrects any misunderstanding**:
   - DO NOT just accept the correction
   - Spawn new research tasks to verify the correct information
   - Only proceed once you've verified the facts yourself

4. **Present refined understanding**:
   ```
   Based on my research, here's what I found:

   **Current state:** [What exists now]
   **What needs to be built:** [Refined scope]
   **Suggested ticket breakdown:** [High-level list of N tickets]

   Does this grouping make sense? Should any tickets be split or merged?
   ```

### Step 3: Ticket Breakdown Proposal

Once aligned on scope:

1. **Propose the ticket list**:
   ```
   Here's my proposed ticket breakdown:

   1. [Ticket title] - [1-line description of what it delivers]
   2. [Ticket title] - [1-line description]
   3. [Ticket title] - [1-line description]
   ...

   **Dependency order:** [Describe which tickets depend on others]

   Does this look right? Should I adjust scope, ordering, or granularity?
   ```

2. **Get feedback on the breakdown** before writing detailed tickets

3. **Apply the following principles** when breaking down tickets:
   - Each ticket should deliver a **testable milestone** - something you can see or verify
   - Tickets should be **incrementally buildable** - each builds on the previous
   - Keep tickets **small enough to implement in a single session** (roughly 1-3 hours of work)
   - Each ticket should have **clear "done when" criteria**
   - Minimize cross-ticket dependencies where possible
   - Group related changes together (don't split a feature across tickets unnecessarily)

### Step 4: Detailed Ticket Writing

After breakdown approval:

1. **Determine ticket numbering**:
   - Check existing tickets in `thoughts/shared/tickets/`
   - Start numbering from the next available number
   - Use today's date for the file prefix

2. **Write each ticket** to `thoughts/shared/tickets/YYYY-MM-DD-NNNN-description.md`
   - Format: `YYYY-MM-DD-NNNN-description.md` where:
     - YYYY-MM-DD is today's date
     - NNNN is the 4-digit zero-padded sequential ticket number (0001, 0002, ...)
     - description is a brief kebab-case description
   - Example: `2026-02-08-0001-camera-battlefield-layout.md`

3. **Use this ticket template**:

````markdown
# Ticket N: [Title]

**Delivers:** [One-line summary of what this ticket produces]

**Depends on:** [List ticket numbers this depends on, or "None"]

## What to Implement

| What to implement | How to test |
|-------------------|-------------|
| [Specific item to build] | [How to verify it works] |
| [Another item] | [Verification method] |
| [Another item] | [Verification method] |

## Context

[Brief context about why this ticket exists, what it connects to, and any important technical notes. Include file:line references to relevant existing code if applicable.]

## Done When

[Clear, observable criteria for when this ticket is complete. Should be specific enough that someone unfamiliar with the project could verify it.]

## References

- Source: [Link to research doc, design doc, or other input that generated this ticket]
````

4. **Write tickets one at a time**, presenting each to the user as you go:
   ```
   Written ticket NNNN to: `thoughts/shared/tickets/YYYY-MM-DD-NNNN-description.md`

   [Brief summary of what the ticket covers]

   Continuing to next ticket...
   ```

### Step 5: Summary & Cross-References

After all tickets are written:

1. **Update MEMORY.md** with a ticket index:
   - Add a section listing all generated tickets with their numbers and titles
   - Note the source input that generated them
   - Include the ticket number range

2. **Present the final summary**:
   ```
   Created N tickets in `thoughts/shared/tickets/`:

   | # | File | Title | Depends on |
   |---|------|-------|------------|
   | 0001 | YYYY-MM-DD-0001-description.md | [Title] | None |
   | 0002 | YYYY-MM-DD-0002-description.md | [Title] | 0001 |
   | ... | ... | ... | ... |

   **Source:** [Input files/text used]

   **Suggested implementation order:** [Order considering dependencies]

   You can now use these tickets with `/create_plan` to generate detailed implementation plans:
   `/create_plan thoughts/shared/tickets/YYYY-MM-DD-0001-description.md`
   ```

3. **Ask if any adjustments are needed**:
   ```
   Would you like to:
   - Adjust any ticket scope or details?
   - Split or merge any tickets?
   - Change the ordering or dependencies?
   ```

4. **Iterate** until the user is satisfied

## Important Guidelines

1. **Be Interactive**:
   - Don't write all tickets in one shot
   - Get buy-in on the breakdown before writing details
   - Allow course corrections at each step
   - Present ticket proposals incrementally

2. **Be Thorough**:
   - Read all input files COMPLETELY before analyzing
   - Research actual codebase state using parallel sub-tasks
   - Include specific file paths and line numbers where relevant
   - Write testable "done when" criteria

3. **Be Practical**:
   - Focus on incremental, testable milestones
   - Keep tickets reasonably sized (not too big, not too granular)
   - Consider implementation order and dependencies
   - Think about what makes a satisfying "checkpoint"

4. **Avoid Scope Creep**:
   - Stick to what the input describes
   - If you identify additional work, note it separately as "future tickets"
   - Don't add nice-to-have features unless the user asks

5. **No Open Questions in Final Tickets**:
   - If you encounter ambiguity during ticket writing, STOP
   - Ask for clarification immediately
   - Do NOT write tickets with unresolved questions
   - Every ticket must be clear and actionable

6. **Track Progress**:
   - Use TodoWrite to track which tickets you've written
   - Update todos as you complete each ticket
   - Mark all complete when done

## Ticket Scoping Guidelines

**Good ticket scope:**
- Delivers something visible/testable
- Can be implemented in 1-3 hours
- Has clear boundaries
- Builds incrementally on prior work

**Too big:**
- "Implement the entire combat system" (split into targeting, damage, death, health bars)
- "Add all UI elements" (split by functional area)

**Too small:**
- "Add a single component struct" (combine with related logic)
- "Write one test" (include testing in the implementation ticket)

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
