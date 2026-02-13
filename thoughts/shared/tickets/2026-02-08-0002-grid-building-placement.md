# Ticket 2: Grid & Building Placement — DONE

**Status:** Completed 2026-02-09

**Delivers:** Visible grid in the building zone, click-to-place buildings (Barracks and Farm)

**Depends on:** 1

## What to Implement

| What to implement | How to test |
|-------------------|-------------|
| Grid data structure (6 columns x 10 rows) | Grid tracks occupied/empty cells |
| Grid rendering (outlined squares in building zone) | Grid lines visible, cells clearly defined |
| Mouse-to-grid coordinate conversion | Hover highlights the correct cell |
| Building component and BuildingType enum (Barracks, Farm) | Types defined with distinct properties |
| Click to place building (hardcoded to Barracks for now) | Click empty cell → blue square appears |
| Placement validation (can't place on occupied cell) | Clicking occupied cell does nothing |
| Building rendering (colored squares per type) | Barracks = dark blue, Farm = green/gold |
| Z-layer ordering constants (background, zone, grid, building, unit, health bar, etc.) | Sprites render in correct visual order, no z-fighting |

## Context

This ticket introduces the first player interaction. The grid lives inside the building zone defined in Ticket 1. Players click cells to place buildings.

For now, buildings are visual only — they don't produce units or cost gold yet (those come in Tickets 3 and 6). The goal is to get the placement loop feeling right.

**Prerequisites from Ticket 1:**
- Battlefield constants (`CELL_SIZE`, zone column ranges, `col_to_world_x`/`row_to_world_y` helpers)
- State architecture with `InGameState` SubState (Paused fix already done in Ticket 1)
- `DespawnOnExit(GameState::InGame)` pattern for entity cleanup
- Game systems use `run_if(in_state(InGameState::Playing))` to auto-pause

Relevant files:
- `src/battlefield.rs` — battlefield constants and helpers
- `src/lib.rs` — `GameState`, `InGameState` enums
- `src/components/mod.rs` — new Building components go here

## Done When

- Grid is visible in the building zone with clearly outlined cells
- Hovering over the grid highlights the targeted cell
- Clicking an empty cell places a building (colored square)
- Clicking an occupied cell does nothing (no duplicate placement)
- Pausing and resuming preserves all placed buildings

## Architecture Notes (from Ticket 1 review)

- **`screens/` directory naming**: `screens/in_game.rs` is becoming an input dispatcher, not a visual screen. If building placement input is added, put it in the building/grid domain plugin — not in `handle_game_input`. Monitor whether the `screens/` name still fits.
- **`handle_game_input` scope**: Keep it to state-transition keys (ESC/Q) only. Mouse/building input belongs in domain plugins.
- **Prelude convention**: Use explicit imports from domain modules (e.g., `use crate::battlefield::PlayerFortress`) rather than glob re-exports through the prelude.
- **Re-create `src/components/mod.rs`** when adding Building components. Ticket 1 deleted the empty module.

## References

- Source: `thoughts/shared/research/2026-02-04-tano-style-game-research.md` (Section 2.2 Building System)
