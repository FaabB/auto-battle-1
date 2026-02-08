---
date: 2026-02-04T21:15:50+0100
researcher: FaabB
git_commit: HEAD (uncommitted changes)
branch: main
repository: auto-battle-1
topic: "Bevy 0.18 Autobattler Project Setup"
tags: [implementation, bevy, rust, game-dev, project-setup]
status: in_progress
last_updated: 2026-02-04
last_updated_by: FaabB
type: implementation_strategy
---

# Handoff: Bevy 0.18 Project Setup - Phase 1 Complete

## Task(s)

Implementing the Bevy 0.18 project setup plan for an autobattler game.

| Phase | Status | Description |
|-------|--------|-------------|
| Phase 1 | **Completed** | Project Initialization (Cargo, Bevy deps, config files) |
| Phase 2 | Pending | Project Structure & Plugin Architecture |
| Phase 3 | Pending | Game States & Screen Plugins |
| Phase 4 | Pending | Main Entry Point & Camera Setup |
| Phase 5 | Pending | Testing Infrastructure |
| Phase 6 | Pending | Development Tooling & Makefile |

**Currently at**: Awaiting manual verification of Phase 1 before proceeding to Phase 2.

## Critical References

- Implementation plan: `thoughts/shared/plans/2026-02-04-bevy-project-setup.md`

## Recent changes

Files created/modified:
- `Cargo.toml` - Bevy 0.18 dependency, clippy/rust lints, build profiles
- `.cargo/config.toml` - LLD linker configuration for fast builds
- `rustfmt.toml` - Formatter settings (removed unstable options)
- `clippy.toml` - Linter configuration
- `src/main.rs` - Default cargo init placeholder (will be replaced in Phase 4)

## Learnings

1. **LLD not installed by default on macOS**: The plan specifies LLD for faster link times. User installed it via `brew install llvm`. Config is at `.cargo/config.toml:12-16`.

2. **Rustfmt unstable features**: `imports_granularity` and `group_imports` require nightly Rust. Removed these from `rustfmt.toml`.

3. **Clippy lint priorities**: Lint groups (`all`, `pedantic`, `nursery`) need explicit `priority = -1` to allow individual lint overrides. Fixed in `Cargo.toml:17-19`.

4. **Build time**: Full Bevy build takes ~4.5 minutes. Avoid `cargo clean` unless necessary - incremental builds are much faster (~1 min).

## Artifacts

- `thoughts/shared/plans/2026-02-04-bevy-project-setup.md` - Implementation plan (Phase 1 checkboxes updated)
- `Cargo.toml` - Project manifest
- `.cargo/config.toml` - Build configuration
- `rustfmt.toml` - Formatter config
- `clippy.toml` - Linter config

## Action Items & Next Steps

1. **Complete manual verification of Phase 1**:
   - [ ] Verify `Cargo.toml` contains all specified sections
   - [ ] Verify `.cargo/config.toml` exists with linker config

2. **Proceed to Phase 2**: Create project directory structure and plugin architecture:
   - Create `src/{game,screens,ui,components,systems,resources}` directories
   - Create `assets/{sprites,audio,fonts}` with `.gitkeep`
   - Implement prelude, components, resources, systems, game, and screens modules

3. **Phases 3-6**: Continue through remaining phases per the plan

## Other Notes

- Bevy 0.18 requires Rust 1.89+ according to cargo fetch output, but we specified `rust-version = "1.85"` - this hasn't caused issues but may need attention
- The plan references "There Are No Orcs" as the inspiration game - a 2D pixel-art autobattler
- All verification should use `make check` once the Makefile is created in Phase 6; until then use `cargo fmt --check && cargo clippy`
