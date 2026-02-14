# CLAUDE.md

This file provides guidance to Claude Code when working with code in this repository.

## Repository Overview

Auto-battle-1 is a 2D auto-battler game built with **Bevy 0.18** in Rust.

### Architecture

See `ARCHITECTURE.md` for the full conventions reference.

This project uses Bevy's **Entity Component System (ECS)**. All game state lives in components on entities, and all logic lives in systems. Do not use OOP patterns (inheritance hierarchies, manager objects, singletons). Key conventions:

- **Function plugins** — all plugins use `pub(super) fn plugin(app: &mut App)`, not struct-based `impl Plugin`.
- **Visibility** — `pub(crate)` for cross-module types, `pub(super)` for plugin functions, private for systems.
- **States** live in `screens/` — `GameState` in `screens/mod.rs`, `InGameState` in `screens/in_game.rs`. Both use `#[states(scoped_entities)]`.
- **Components** are co-located with their systems in domain plugins, not in a shared `components/` module.
- **Theme** — shared colors in `theme/palette.rs`, widget constructors in `theme/widget.rs`.
- **GameSet** — global `Update` schedule ordering (Input → Production → Ai → Movement → Combat → Death → Ui). Domain plugins use `.in_set(GameSet::Xxx)`.
- **Dev tools** — `src/dev_tools/` is feature-gated on `dev`. Debug-only tools go here.
- **Resources** are global singletons for cross-cutting state (economy balance, wave counter). Prefer components on entities over resources when the data belongs to a specific entity.

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

### Testing
- Target 90% test coverage
- Every ticket should include tests that maintain or increase coverage toward this goal
- Use `src/testing.rs` helpers (`create_base_test_app`, `create_base_test_app_no_input`, `transition_to_ingame`, `assert_entity_count`) for system-level tests

### Commits
- Use `/commit` command for creating commits
- Write clear, imperative commit messages
- Group related changes together

### Planning
- Use `/create_plan` before implementing significant features
- Break work into phases with clear success criteria
- Distinguish between automated and manual verification
