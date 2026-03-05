---
description: Create git commits and push to remote
---

# Commit and Push

You are tasked with creating git commits and pushing them to the remote for the changes made during this session.

## Process:

1. **Think about what changed:**
   - Review the conversation history and understand what was accomplished
   - Run `git status` to see current changes
   - Run `git diff` to understand the modifications
   - Consider whether changes should be one commit or multiple logical commits

2. **Plan your commit(s):**
   - Identify which files belong together
   - Draft clear, descriptive commit messages
   - Use imperative mood in commit messages
   - Focus on why the changes were made, not just what

3. **Present your plan to the user:**
   - List the files you plan to add for each commit
   - Show the commit message(s) you'll use
   - Ask: "I plan to create [N] commit(s) with these changes. Shall I proceed?"

4. **Execute upon confirmation:**
   - Use `git add` with specific files (never use `-A` or `.`)
   - Create commits with your planned messages
   - Push to the remote branch: `git push`
   - Show the result with `git log --oneline -n [number]`

5. **Verify push succeeded:**
   - Confirm the push completed without errors
   - If the push fails (e.g., no upstream branch), set it up: `git push -u origin <branch>`

## Important:
- **NEVER use `$(...)` command substitution in git commands** — it triggers a security prompt. Use plain `git commit -m "message"` with simple strings. For multi-line messages, use multiple `-m` flags.
- **NEVER add co-author information or Claude attribution**
- Commits should be authored solely by the user
- Do not include any "Generated with Claude" messages
- Do not add "Co-Authored-By" lines
- Write commit messages as if the user wrote them
- **Always push after committing** — this is the key difference from `/commit`

## Remember:
- You have the full context of what was done in this session
- Group related changes together
- Keep commits focused and atomic when possible
- The user trusts your judgment - they asked you to commit
