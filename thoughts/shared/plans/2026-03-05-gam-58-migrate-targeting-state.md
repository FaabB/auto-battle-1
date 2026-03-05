# Migrate Consumers to TargetingState, Remove CurrentTarget (GAM-58)

## Overview

Mechanical migration: make all consumers read `TargetingState` instead of `CurrentTarget`, then delete `CurrentTarget`. No logic changes — just swapping which component is read. Part 1b of the targeting/movement/combat architecture rework.

## Current State Analysis

`TargetingState` (added in GAM-57) coexists with `CurrentTarget`. The AI system (`find_target`) writes both in parallel. Three consumer systems (`unit_movement`, `compute_paths`, `attack`) read `CurrentTarget.0` for the target entity. Five spawn sites include `CurrentTarget(None)`.

### Key Discoveries:
- `find_target` (`ai.rs:96`) queries both `&mut CurrentTarget` and `&mut TargetingState`, writes both at line 154-155
- `unit_movement` (`movement.rs:29`) reads `&CurrentTarget` — extracts `current_target.0` at line 54
- `compute_paths` (`pathfinding.rs:125`) reads `&CurrentTarget` — extracts `.0` at lines 148, 156, 176
- `attack` (`attack.rs:50`) reads `&CurrentTarget` — extracts `.0` at line 69
- `handle_target_death` observer (`death.rs:24`) already reads only `&mut TargetingState` — no change needed
- Spawn sites: `units/mod.rs:106`, `renderer.rs:87,170`, `testing.rs:162`; movement test helper `movement.rs:124`
- ~25 test assertions check `CurrentTarget` across `ai.rs`, `attack.rs`, `spawn.rs`, `battlefield/mod.rs`

## Desired End State

- `CurrentTarget` struct is deleted from `gameplay/mod.rs`
- All systems read target entity from `TargetingState` via a `target_entity()` helper method
- All spawn sites no longer include `CurrentTarget`
- All tests assert on `TargetingState` variants
- `make check && make test` pass cleanly

## What We're NOT Doing

- No logic changes to targeting, movement, or combat behavior
- No changes to `TargetingState` variants or the death observer
- No changes to `EngagementLeash` or `LEASH_DISTANCE` (future tickets)
- No removal of `Collider`-based distance checks (that's GAM-59)

## Implementation Approach

Single phase — purely mechanical find-and-replace with a helper method to keep it clean.

## Phase 1: Migrate and Remove

### Changes Required:

#### 1. Add `target_entity()` helper to `TargetingState`
**File**: `src/gameplay/mod.rs`
**Changes**: Add method to extract the target entity from `Engaging` or `Attacking` variants.

```rust
impl TargetingState {
    /// Returns the target entity if in `Engaging` or `Attacking` state.
    #[must_use]
    pub const fn target_entity(self) -> Option<Entity> {
        match self {
            Self::Engaging(e) | Self::Attacking(e) => Some(e),
            Self::Moving | Self::Seeking => None,
        }
    }
}
```

#### 2. Migrate `find_target`
**File**: `src/gameplay/ai.rs`
**Changes**:
- Remove `&mut CurrentTarget` from query (line 105)
- Remove `mut current_target` from destructure (line 122)
- Change `has_valid_target` check (line 127): `current_target.0.is_some_and(...)` → `targeting_state.target_entity().is_some_and(...)`
- Remove `current_target.0 = nearest;` (line 154)
- Keep the `*targeting_state = ...` write (line 155) — this is already correct
- Remove `CurrentTarget` from import (line 8)
- Update system doc (line 89): remove "with `CurrentTarget`"

#### 3. Migrate `unit_movement`
**File**: `src/gameplay/units/movement.rs`
**Changes**:
- Replace `&CurrentTarget` with `&TargetingState` in query (line 32)
- Replace `current_target` with `targeting_state` in destructure (line 45)
- Replace `let Some(target_entity) = current_target.0` with `let Some(target_entity) = targeting_state.target_entity()` (line 54)
- Update import: `CurrentTarget` → `TargetingState` (line 8)

#### 4. Migrate `compute_paths`
**File**: `src/gameplay/units/pathfinding.rs`
**Changes**:
- Replace `&CurrentTarget` with `&TargetingState` in query (line 125)
- Replace `current_target` with `targeting_state` in loop destructure (line 147)
- Replace `nav_path.needs_recompute(current_target.0)` → `nav_path.needs_recompute(targeting_state.target_entity())` (line 148)
- Replace `let Some(target_entity) = current_target.0` → `let Some(target_entity) = targeting_state.target_entity()` (line 156)
- Replace `nav_path.set(path.path, current_target.0)` → `nav_path.set(path.path, targeting_state.target_entity())` (line 176)
- Update import: `CurrentTarget` → `TargetingState` (line 7)

#### 5. Migrate `attack`
**File**: `src/gameplay/combat/attack.rs`
**Changes**:
- Replace `&CurrentTarget` with `&TargetingState` in query (line 53)
- Rename `target` to `targeting_state` in destructure (line 63)
- Replace `let Some(target_entity) = target.0` → `let Some(target_entity) = targeting_state.target_entity()` (line 69)
- Update import: `CurrentTarget` → `TargetingState` (line 6)

#### 6. Remove `CurrentTarget` from spawn sites
**File**: `src/gameplay/units/mod.rs`
- Remove `CurrentTarget(None),` from `spawn_unit` (line 106)
- Remove `CurrentTarget` from import (line 18)

**File**: `src/gameplay/battlefield/renderer.rs`
- Remove `CurrentTarget(None),` from player fortress spawn (line 87)
- Remove `CurrentTarget(None),` from enemy fortress spawn (line 170)
- Remove `CurrentTarget` from import (line 17)

**File**: `src/testing.rs`
- Remove `CurrentTarget(None),` from `spawn_test_unit` (line 162)
- Remove `CurrentTarget` from import (line 16)
- Update `spawn_test_unit` doc comment (line 148): remove "CurrentTarget(None),"

#### 7. Delete `CurrentTarget` definition and registration
**File**: `src/gameplay/mod.rs`
- Delete `CurrentTarget` struct and doc comment (lines 73-77)
- Delete migration comment (line 80)
- Remove `.register_type::<CurrentTarget>()` (line 132)
- Update entity archetype doc comments (lines 5, 12): remove `CurrentTarget` from listings

#### 8. Update tests

**`src/gameplay/ai.rs` tests** (~15 assertions):
- All `.get::<CurrentTarget>(entity).unwrap().0` → `.get::<TargetingState>(entity).unwrap().target_entity()`
- Assertions: `assert_eq!(ct.0, Some(enemy))` → `assert_eq!(ts.target_entity(), Some(enemy))`
- Assertions: `assert_eq!(ct.0, None)` → `assert_eq!(ts.target_entity(), None)`
- Fortress test spawns (lines 460-501): remove `CurrentTarget(None),` (already have `TargetingState::Seeking`)

**`src/gameplay/combat/attack.rs` tests**:
- `spawn_attacker` helper (line 249): `world.entity_mut(id).insert(CurrentTarget(Some(t)))` → `world.entity_mut(id).insert(TargetingState::Engaging(t))`
- `attack_respects_cooldown` (line 335): `.insert(CurrentTarget(Some(target)))` → `.insert(TargetingState::Engaging(target))`
- `fortress_can_attack_in_range` (line 511): `CurrentTarget(None)` → keep `TargetingState::Seeking` (if already present), otherwise add it
- Fortress target set (line 528): `.get_mut::<CurrentTarget>(fortress).unwrap().0 = Some(target)` → `*app.world_mut().get_mut::<TargetingState>(fortress).unwrap() = TargetingState::Engaging(target)`
- Update import: `CurrentTarget` → `TargetingState` (line 213)

**`src/gameplay/units/movement.rs` tests**:
- `spawn_unit_at` helper (line 124): `.insert((Movement { speed }, CurrentTarget(target)))` → `.insert((Movement { speed }, TargetingState::from_target(target)))` — but we don't have `from_target`. Instead: `target.map_or(TargetingState::Seeking, TargetingState::Engaging)`
- Update import in test: remove `CurrentTarget`

**`src/gameplay/units/spawn.rs` tests** (line 267):
- `assert_entity_count::<(With<Unit>, With<CurrentTarget>)>` → `assert_entity_count::<(With<Unit>, With<TargetingState>)>`
- Update import (line 190): `CurrentTarget` → `TargetingState`

**`src/gameplay/battlefield/mod.rs` tests** (lines 490-495):
- `assert_entity_count::<(With<PlayerFortress>, With<CurrentTarget>)>` → `assert_entity_count::<(With<PlayerFortress>, With<TargetingState>)>`
- Same for `EnemyFortress`
- Rename test: `fortress_has_current_target` → `fortress_has_targeting_state`
- Update import: `CurrentTarget` → `TargetingState`

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes (no compiler errors, clippy clean)
- [x] `make test` passes (all ~40 affected tests)
- [x] `grep -r "CurrentTarget" src/` returns zero results

#### Manual Verification:
- [x] Game runs normally — units target, move, and attack as before
- [x] Fortresses acquire targets and fire projectiles
- [x] Units retarget when their target dies

---

## Testing Strategy

No new tests needed — this is a mechanical migration. All existing tests are updated to assert on `TargetingState` instead of `CurrentTarget`. The test count stays the same.

### Key test patterns to update:
- **AI tests**: `get::<CurrentTarget>` → `get::<TargetingState>`, check `.target_entity()`
- **Attack tests**: Insert `TargetingState::Engaging(target)` instead of `CurrentTarget(Some(target))`
- **Movement tests**: Same pattern as attack
- **Spawn tests**: `With<CurrentTarget>` → `With<TargetingState>`

## References

- Linear ticket: [GAM-58](https://linear.app/tayhu-games/issue/GAM-58/migrate-consumers-to-targetingstate-remove-currenttarget)
- Research: `thoughts/shared/research/2026-03-05-targeting-movement-combat-scalability.md` (Section 9, Ticket 1b)
- GAM-57 plan: `thoughts/shared/plans/2026-03-05-gam-57-targeting-state-death-observer.md`
- Blocks: [GAM-59](https://linear.app/tayhu-games/issue/GAM-59/combat-state-and-range-simplification-entityextent) (EntityExtent)
