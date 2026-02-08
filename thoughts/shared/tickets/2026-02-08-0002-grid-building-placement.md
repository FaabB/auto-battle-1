# Ticket 2: Grid & Building Placement

**Delivers:** Visible grid in the building zone, click-to-place buildings (Barracks and Farm)

**Depends on:** 1

## What to Implement

| What to implement | How to test |
|-------------------|-------------|
| Grid data structure (6 columns x 8 rows) | Grid tracks occupied/empty cells |
| Grid rendering (outlined squares in building zone) | Grid lines visible, cells clearly defined |
| Mouse-to-grid coordinate conversion | Hover highlights the correct cell |
| Building component and BuildingType enum (Barracks, Farm) | Types defined with distinct properties |
| Click to place building (hardcoded to Barracks for now) | Click empty cell → blue square appears |
| Placement validation (can't place on occupied cell) | Clicking occupied cell does nothing |
| Building rendering (colored squares per type) | Barracks = dark blue, Farm = green/gold |
| Fix Paused state architecture | Pausing no longer destroys InGame entities |

## Context

This ticket introduces the first player interaction. The grid lives inside the building zone defined in Ticket 1. Players click cells to place buildings.

For now, buildings are visual only — they don't produce units or cost gold yet (those come in Tickets 3 and 6). The goal is to get the placement loop feeling right.

**Critical fix needed:** The Paused state must be reworked before this ticket, because placing buildings creates InGame entities that would be destroyed when pausing. Either make Paused a sub-state of InGame, or use a parallel `IsPaused` state with run conditions. See research doc Section 3.1.

Relevant files:
- `src/screens/in_game.rs` — InGame state (will need game entity spawning)
- `src/screens/paused.rs` — Paused state (needs architectural fix)
- `src/lib.rs` — GameState enum (may need sub-states)
- `src/components/mod.rs` — new Building components go here

## Done When

- Grid is visible in the building zone with clearly outlined cells
- Hovering over the grid highlights the targeted cell
- Clicking an empty cell places a building (colored square)
- Clicking an occupied cell does nothing (no duplicate placement)
- Pausing and resuming preserves all placed buildings

## References

- Source: `thoughts/shared/research/2026-02-04-tano-style-game-research.md` (Section 2.2 Building System, Section 3.1 State Machine note on Pause)
