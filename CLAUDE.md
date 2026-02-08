# CLAUDE.md

This file provides guidance to Claude Code when working with code in this repository.

## Repository Overview

This is the auto-battle-1 project.

## Thoughts Workflow

This project uses a **thoughts workflow** system for managing development notes, research, plans, and handoffs separately from the codebase while maintaining tight integration.

### Directory Structure

```
thoughts/
├── shared/              # Team-shared documents
│   ├── research/        # Codebase research documents
│   ├── plans/           # Implementation plans
│   ├── handoffs/        # Session handoff documents
│   └── tickets/         # Ticket documentation
├── [username]/          # Personal notes (user-specific)
└── global/              # Cross-repository thoughts
```

### Available Commands

| Command | Description |
|---------|-------------|
| `/research_codebase` | Document codebase understanding without critique |
| `/create_tickets` | Generate implementation tickets from research, files, or text |
| `/create_plan` | Create detailed implementation plans interactively |
| `/implement_plan` | Execute plans with verification gates |
| `/validate_plan` | Verify implementation matches plan |
| `/iterate_plan` | Update existing plans based on feedback |
| `/create_handoff` | Create session handoff document |
| `/resume_handoff` | Resume work from handoff document |
| `/commit` | Create git commits with user approval |

### Document Naming Conventions

| Type | Pattern | Example |
|------|---------|---------|
| Research | `YYYY-MM-DD-description.md` | `2025-02-04-auth-flow.md` |
| Tickets | `YYYY-MM-DD-NNNN-description.md` | `2025-02-04-0001-camera-layout.md` |
| Plans | `YYYY-MM-DD-description.md` | `2025-02-04-new-feature.md` |
| Handoffs | `YYYY-MM-DD_HH-MM-SS_description.md` | `2025-02-04_14-30-00_feature-work.md` |

### Workflow Phases

1. **Research** (`/research_codebase`) - Document codebase as-is
2. **Ticketing** (`/create_tickets`) - Break work into implementation tickets
3. **Planning** (`/create_plan`) - Create detailed implementation plans
4. **Implementation** (`/implement_plan`) - Execute with verification pauses
5. **Validation** (`/validate_plan`) - Verify correctness
6. **Handoff** (`/create_handoff`) - Transfer context to new sessions

### Specialized Agents

The workflow uses specialized sub-agents:

- **codebase-locator** - Find WHERE files live
- **codebase-analyzer** - Understand HOW code works
- **codebase-pattern-finder** - Find existing patterns to model after
- **thoughts-locator** - Find relevant thoughts documents
- **thoughts-analyzer** - Extract insights from thoughts

**Key principle**: All agents are documentarians, not critics. They describe what exists without suggesting improvements.

## Development Commands

### Quick Actions
- `make check` - Run linting and type checking
- `make test` - Run all tests
- `make build` - Build the project

## Guidelines

### Code Style
- Follow existing patterns in the codebase
- Use consistent naming conventions
- Write clear, self-documenting code

### Commits
- Use `/commit` command for creating commits
- Write clear, imperative commit messages
- Group related changes together

### Planning
- Use `/create_plan` before implementing significant features
- Break work into phases with clear success criteria
- Distinguish between automated and manual verification
