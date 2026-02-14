---
description: Implement technical plans from thoughts/shared/plans with verification
---

# Implement Plan

You are tasked with implementing an approved technical plan from `thoughts/shared/plans/`. These plans contain phases with specific changes and success criteria.

**CRITICAL RULE**: Whenever you need to ask the user a question, present options, or request guidance, you MUST use the **AskUserQuestion** tool. Do NOT just write questions as plain text output — the user expects structured, interactive prompts they can respond to via the tool's UI.

## Linear Ticket Integration

Plans may reference a Linear ticket (e.g., `GAM-5`, `GAM-9`). Look for a ticket identifier in:
- The plan's metadata or title
- The original ticket file referenced by the plan

When a Linear ticket is found:
- **On start**: Move the ticket to **In Progress** using the `update_issue` Linear tool (set `state` to `"In Progress"`)
- **On completion** (all phases done and verified): Move the ticket to **Done** (set `state` to `"Done"`)

If no Linear ticket is associated with the plan, skip this step silently.

## Getting Started

When given a plan path:
- Read the plan completely and check for any existing checkmarks (- [x])
- Read the original ticket and all files mentioned in the plan
- **Read files fully** - never use limit/offset parameters, you need complete context
- Think deeply about how the pieces fit together
- Check if the plan references a Linear ticket — if so, move it to **In Progress**
- Create a todo list to track your progress
- Start implementing if you understand what needs to be done

If no plan path provided, use the **AskUserQuestion** tool to ask: "Which plan should I implement?" with options listing recent plan files from `thoughts/shared/plans/`, or an option to provide a custom path.

## Implementation Philosophy

Plans are carefully designed, but reality can be messy. Your job is to:
- Follow the plan's intent while adapting to what you find
- Implement each phase fully before moving to the next
- Verify your work makes sense in the broader codebase context
- Update checkboxes in the plan as you complete sections

When things don't match the plan exactly, think about why and communicate clearly. The plan is your guide, but your judgment matters too.

If you encounter a mismatch:
- STOP and think deeply about why the plan can't be followed
- Present the issue context as text:
  ```
  Issue in Phase [N]:
  Expected: [what the plan says]
  Found: [actual situation]
  Why this matters: [explanation]
  ```
- Then use the **AskUserQuestion** tool to ask how to proceed, with options describing the viable paths forward (e.g., "Adapt the plan to match reality", "Skip this step", "Investigate further before deciding").

## Verification Approach

After implementing a phase:
- Run the success criteria checks (usually `make check test` covers everything)
- Fix any issues before proceeding
- Update your progress in both the plan and your todos
- Check off completed items in the plan file itself using Edit
- **Pause for human verification**: After completing all automated verification for a phase, pause and inform the human that the phase is ready for manual testing. Use this format:
  ```
  Phase [N] Complete - Ready for Manual Verification

  Automated verification passed:
  - [List automated checks that passed]

  Please perform the manual verification steps listed in the plan:
  - [List manual verification items from the plan]

  Let me know when manual testing is complete so I can proceed to Phase [N+1].
  ```

If instructed to execute multiple phases consecutively, skip the pause until the last phase. Otherwise, assume you are just doing one phase.

Do not check off items in the manual testing steps until confirmed by the user.

When all phases are complete and the user confirms manual verification has passed, move the associated Linear ticket (if any) to **Done**.

## If You Get Stuck

When something isn't working as expected:
- First, make sure you've read and understood all the relevant code
- Consider if the codebase has evolved since the plan was written
- Present the mismatch clearly as text, then use the **AskUserQuestion** tool to ask for guidance with concrete options

Use sub-tasks sparingly - mainly for targeted debugging or exploring unfamiliar territory.

## Resuming Work

If the plan has existing checkmarks:
- Trust that completed work is done
- Pick up from the first unchecked item
- Verify previous work only if something seems off

Remember: You're implementing a solution, not just checking boxes. Keep the end goal in mind and maintain forward momentum.
