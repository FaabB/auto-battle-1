# Ticket 1: Camera & Battlefield Layout — DONE

**Status:** Done (2026-02-08, reviewed 2026-02-09)

**Delivers:** Battlefield coordinate system with zones, fortress placeholders, horizontal camera panning, and state architecture refactor

**Depends on:** None

## What Was Implemented

### Phase 0: State Architecture Refactor
- Removed `GameState::Paused` — replaced with `InGameState::Paused` SubState
- Replaced all manual `CleanupXxx` markers with Bevy's built-in `DespawnOnExit`
- Deleted `src/components/`, `src/systems/`, `src/resources/`, `src/ui/` directories
- Moved camera setup from `main.rs` to `GamePlugin`
- Merged `PausedPlugin` into `InGamePlugin` (owns both pause UI and input)
- Added `#[derive(Debug)]` to all plugins
- Cleaned prelude (no wildcard component re-export, explicit `InGameState` export)

### Phase 1: Battlefield Module
- Created `src/battlefield/` with `mod.rs`, `renderer.rs`, `camera.rs`
- All coordinate constants defined (82 cols, 10 rows, 64px cells)
- Zone column ranges: player fort (0-1), build zone (2-7), combat zone (8-79), enemy fort (80-81)
- Marker components: `PlayerFortress`, `EnemyFortress`, `BuildZone`, `CombatZone`, `BattlefieldBackground`, `BuildSlot`
- Helper functions: `col_to_world_x`, `row_to_world_y`, `zone_center_x`, `battlefield_center_y`

### Phase 2: Spawn Battlefield
- `spawn_battlefield` creates 5 colored sprites (background + 4 zones)
- 60 `BuildSlot` data-only entities (10 rows x 6 cols) for Ticket 2
- All entities use `DespawnOnExit(GameState::InGame)`
- Fortress entities have marker components for Ticket 8

### Phase 3: Camera Positioning & Panning
- Camera starts centered on build zone with `FixedVertical` scaling
- A/D and arrow keys pan horizontally at 500px/s
- Camera clamps at battlefield boundaries (aspect-ratio-aware)
- Panning stops during `InGameState::Paused`

### Bevy 0.18 Idiom Compliance (post-implementation review)
- All components derive `Debug, Reflect` with `#[reflect(Component)]`
- Marker components have `Clone, Copy`; data components have `Clone`
- All 6 types registered with `app.register_type::<T>()`
- `Single<D, F>` used everywhere (no `.single_mut()`)
- `#[must_use]` on all helper functions

### Tests
- 23 unit + integration tests (7 unit, 16 integration)
- 2 additional integration tests in `tests/integration.rs`
- Test helpers: `create_test_app`, `create_test_app_with_state`, `create_ingame_test_app`

### Divergences from Plan (improvements)
- Module split into `battlefield/{mod,renderer,camera}.rs` instead of single file
- Extra marker components (`BuildZone`, `CombatZone`, `BattlefieldBackground`) for better queryability
- `BuildSlot` grid entities spawned early (data-only, ready for Ticket 2)
- `PausedPlugin` merged into `InGamePlugin` (cleaner ownership)

## Done When

You can see the full battlefield layout with:
- Blue fortress rectangle on the far left
- Marked building zone area (6x8 grid area)
- Wide combat zone in the middle
- Red fortress rectangle on the far right
- Horizontal camera panning with A/D or arrow keys
- Camera stops at battlefield boundaries
- ESC pauses (overlay appears), ESC resumes — **battlefield persists through pause**
- Q from pause returns to MainMenu — all InGame entities cleaned up

## References

- Plan: `thoughts/shared/plans/2026-02-08-camera-battlefield-layout.md`
- Source: `thoughts/shared/research/2026-02-04-tano-style-game-research.md` (Section 2.1, Section 7)
