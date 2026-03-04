---
description: Mark draft PR as ready for review and update Linear ticket
---

# Ready PR

Marks the current branch's draft PR as ready for review and moves the associated Linear ticket to "In Review".

**CRITICAL RULE**: Whenever you need to ask the user a question, present options, or request guidance, you MUST use the **AskUserQuestion** tool.

## Process

1. **Detect the current PR**:
   - Run `gh pr view --json number,title,url,isDraft,body` to find the PR for the current branch
   - If no PR exists, inform the user and stop
   - If the PR is not a draft, inform the user it's already marked as ready

2. **Update the PR description**:
   - Read the plan file from `thoughts/shared/plans/` (find it via the PR body or git log)
   - Review all commits on the branch: `git log main..HEAD --oneline`
   - Update the PR body with a proper summary using `gh pr edit`:

   ```
   gh pr edit --body "$(cat <<'EOF'
   ## Summary
   <1-3 bullet points summarizing what was done>

   ## Changes
   <list of key changes with context>

   ## Test plan
   - [x] `make check` passes
   - [x] `make test` passes
   - [ ] <manual verification items from the plan>

   ## Linear ticket
   [TICKET-ID](linear-url)
   EOF
   )"
   ```

3. **Mark PR as ready**:
   ```
   gh pr ready
   ```

4. **Update Linear ticket**:
   - Extract the ticket identifier from the PR title or branch name (e.g., `GAM-55` from `gam-55-description`)
   - Move the ticket to **In Review** using `mcp__plugin_linear_linear__save_issue` (set `state` to `"In Review"`)

5. **Report to the user**:
   ```
   PR marked as ready for review:
   - PR: <pr-url>
   - Linear ticket <TICKET-ID>: moved to In Review
   ```

## Important:
- If `make check` or `make test` hasn't been run recently, run them first before marking ready
- If tests fail, inform the user and do NOT mark the PR as ready
- Always verify the branch has been pushed before marking ready
