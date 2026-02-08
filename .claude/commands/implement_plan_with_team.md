---
description: Implement plans using a dev/review/test agent team with per-phase gates
---

# Implement Plan with Team

You are the **team lead** implementing an approved technical plan from `thoughts/shared/plans/` using an agent team.

## Getting Started

When given a plan path:
- Read the plan completely and check for any existing checkmarks (- [x])
- Read the original ticket and all files mentioned in the plan
- **Read files fully** - never use limit/offset parameters, you need complete context
- Think deeply about how the pieces fit together

If no plan path provided, ask for one.

## Team Structure

Create a team with 3 agents:
- **developer** — implements code changes (general-purpose, bypassPermissions)
- **reviewer** — reviews code AND runs `cargo build` to verify compilation (general-purpose, default mode)
- **tester** — runs `make check`, writes unit tests (general-purpose, bypassPermissions)

## Task Pipeline (Per-Phase Gates)

For EACH phase in the plan, create 3 tasks in a strict pipeline:

```
Phase N (developer) → Review N (reviewer) → Test N (tester) → Phase N+1 (developer) → ...
```

Set up blocking dependencies so each step waits for the previous:
- Review N is blocked by Phase N
- Test N is blocked by Review N
- Phase N+1 is blocked by Test N

Assign ownership upfront: developer owns all Phase tasks, reviewer owns all Review tasks, tester owns all Test tasks.

### Developer Instructions
- Implement the phase according to the plan
- Run `cargo build` to verify compilation before marking complete
- Message team lead when done

### Reviewer Instructions
- Read all changed files and verify correctness
- **Must run `cargo build`** (not just read code) before marking review complete
- Check for API correctness, proper patterns, missing components
- If issues found, message the developer with specifics
- Mark complete only when code is correct AND compiles

### Tester Instructions
- Run `make check` (clippy + tests)
- On the final phase: also write unit tests if the plan calls for them
- Report any failures to the developer
- Mark complete only when all checks pass

## Your Role as Team Lead

1. **Set up**: Read the plan, create team, create all tasks with dependencies
2. **Coordinate**: When an agent completes work, notify the next agent in the pipeline
3. **Unblock**: If an agent is stuck, investigate and help resolve
4. **Monitor**: Check TaskList periodically to track progress
5. **Verify**: After all phases complete, run `make check` yourself to confirm

## Handling API Mismatches

Plans may contain incorrect API details. When compilation fails:
- Investigate the correct API (use research agents, LSP, cargo docs)
- Send the developer precise fix instructions
- This is expected — plans are guides, not gospel

## Shutdown Protocol

When all phases are complete:
1. **Ask for learnings FIRST** — message each agent asking for bullet-point learnings
2. Wait for learnings to come back
3. Present learnings to the user via AskUserQuestion for memory decisions
4. **Then** send shutdown requests
5. Clean up team with TeamDelete
6. Update plan file checkboxes

## Verification Approach

After ALL phases complete:
- Update checkboxes in the plan file (automated verification items)
- **Pause for human verification**: Inform the user the implementation is ready for manual testing
- Do not check off manual verification items until confirmed by the user

## If You Get Stuck

- If an agent is idle too long, send them a message
- If compilation fails, research the correct API before instructing fixes
- If the plan doesn't match reality, present the mismatch and ask the user

## Resuming Work

If the plan has existing checkmarks:
- Trust that completed work is done
- Pick up from the first unchecked phase
- Only create tasks for remaining phases
