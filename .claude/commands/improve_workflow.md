---
description: Apply agent feedback to improve commands, settings, and memory
---

# Improve Workflow

You are helping the user improve the agent workflow based on feedback from implementation sessions, retros, or observations.

## Process

1. **Listen to the feedback** — the user will describe issues they observed with agent behavior (skipped steps, wrong approaches, permission prompts, etc.)

2. **Identify what needs updating** — for each issue, determine which files are affected:
   - **Commands** (`.claude/commands/*.md`) — agent instructions that get followed during workflows
   - **Settings** (`.claude/settings.json`) — permissions, allowedTools, env vars
   - **User settings** (`~/.claude/settings.json`) — machine-specific permissions (not committed)
   - **Memory** (`MEMORY.md`) — persistent lessons and preferences
   - **CLAUDE.md** — project-level instructions loaded into every conversation

3. **Present the changes** — for each issue, explain:
   - What file(s) to update
   - What the change is
   - Why it fixes the observed behavior

4. **Apply changes one at a time** — make each edit, let the user review

5. **Check for duplication** — if the same rule applies to multiple commands (e.g., `create_plan.md` and `create_plan_with_pr.md` share structure), update all affected files

## Key Files

| File | Purpose | Shared? |
|------|---------|---------|
| `.claude/commands/*.md` | Agent workflow instructions | Yes (committed) |
| `.claude/settings.json` | Project permissions & tools | Yes (committed) |
| `~/.claude/settings.json` | User-specific permissions | No (per-machine) |
| `MEMORY.md` | Persistent agent memory | No (per-user) |
| `CLAUDE.md` | Project instructions for all agents | Yes (committed) |

## Guidelines

- **Settings that contain absolute paths** (e.g., `/Users/faabb/...`) go in user settings, not project settings
- **Rules that affect agent behavior** go in the relevant command file AND memory (commands are authoritative, memory reinforces)
- **Don't over-engineer** — add the minimum rule that prevents the observed bad behavior
- **Check both planning commands** — `create_plan.md` and `create_plan_with_pr.md` share most of their structure. If you update one, check if the other needs the same change.
