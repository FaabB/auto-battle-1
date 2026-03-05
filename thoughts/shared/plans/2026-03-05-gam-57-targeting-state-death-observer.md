# GAM-57: Add TargetingState + Death Observer

**Date**: 2026-03-05
**Ticket**: [GAM-57](https://linear.app/tayhu-games/issue/GAM-57/add-targetingstate-death-observer)
**Status**: Draft
**Blocked by**: Nothing
**Blocks**: GAM-58 (Migrate consumers to TargetingState, remove CurrentTarget)

---

## Overview

Add `TargetingState` enum and `EngagementLeash` component alongside the existing `CurrentTarget`. The AI system writes both in parallel. Add an `On<Remove, Target>` death observer that transitions orphaned units from `Engaging`/`Attacking` to `Seeking`. This is purely additive — existing consumers (`movement.rs`, `attack.rs`, `pathfinding.rs`) continue reading `CurrentTarget` unchanged.

## Current State Analysis

- `CurrentTarget(Option<Entity>)` — `gameplay/mod.rs:77`, written by `find_target` in `ai.rs:96`
- `Target` marker — `gameplay/mod.rs:71`, on all targetable entities
- Death handling — `combat/death.rs:14`, polls health, despawns at 0hp, no observer notification
- Unit spawns — `units/mod.rs:100` with `CurrentTarget(None)`
- Fortress spawns — `battlefield/renderer.rs:67,149` with `CurrentTarget(None)`
- Building spawns — `building/placement.rs:133` with `Target` only (no `CurrentTarget`, buildings don't seek)
- Test helpers — `spawn_test_unit` (`testing.rs:154`), `spawn_test_target` (`testing.rs:194`)

## Desired End State

- `TargetingState` enum component exists on all entities that have `CurrentTarget`
- `EngagementLeash` component defined but not yet read by any system
- AI system writes both `CurrentTarget` and `TargetingState` each evaluation
- When a `Target` entity dies, orphaned `Engaging`/`Attacking` units transition to `Seeking`
- All existing tests pass with minimal changes (adding `TargetingState` to spawned entities)
- New tests cover the death observer and TargetingState transitions

## What We're NOT Doing

- **Not using `Moving` or `Attacking` states** — `Moving` requires flow fields (Ticket 3), `Attacking` requires EntityExtent range checks (Ticket 2). For now AI only writes `Seeking` (no target) or `Engaging(entity)` (has target).
- **Not reading `EngagementLeash`** — the leash check system comes in Ticket 2/3. We define it now so spawn sites set the origin.
- **Not changing existing consumers** — `movement.rs`, `attack.rs`, `pathfinding.rs` still read `CurrentTarget`. Migration is GAM-58.
- **Not adding reverse-lookup index** — O(n) orphan scan is fine at current scale. Deferred to Ticket 6 profiling if needed.

---

## Implementation Approach

### Phase 1: New Components

**Changes Required:**

1. **`gameplay/mod.rs`** — Add `TargetingState` enum and `EngagementLeash` struct:

```rust
/// State machine for targeting behavior.
/// Coexists with `CurrentTarget` during migration (GAM-57/58).
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component)]
pub enum TargetingState {
    /// Following flow field toward assigned goal. No spatial queries.
    Moving,
    /// Looking for targets. Default state for static entities (fortresses).
    Seeking,
    /// Locked onto a target. Movement system steers directly toward it.
    Engaging(Entity),
    /// In attack range, firing. Velocity = 0.
    Attacking(Entity),
}

/// Leash that pulls a unit back to Seeking if it moves too far from origin.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct EngagementLeash {
    pub origin: Vec2,
    pub max_distance: f32,
}

/// Default leash distance in pixels (3 cells).
pub const LEASH_DISTANCE: f32 = 192.0;
```

2. Register types in `gameplay/mod.rs::plugin`:
```rust
.register_type::<TargetingState>()
.register_type::<EngagementLeash>()
```

**Success Criteria (automated):**
- `cargo build` compiles
- Existing tests pass (no changes needed yet)

### Phase 2: AI System — Write Both Components

**Changes Required:**

1. **`gameplay/ai.rs`** — Add `&mut TargetingState` to the `find_target` query. After writing `current_target.0 = nearest`, also write the targeting state:

```rust
// In find_target, after: current_target.0 = nearest;
match nearest {
    Some(entity) => *targeting_state = TargetingState::Engaging(entity),
    None => {
        // Only transition to Seeking if not already Engaging/Attacking
        // (preserves state for units whose target is still valid but not re-evaluated this slot)
        if !matches!(*targeting_state, TargetingState::Engaging(_) | TargetingState::Attacking(_)) {
            *targeting_state = TargetingState::Seeking;
        }
    }
}
```

Wait — actually this is simpler. The `find_target` system already handles the "has valid target, skip" logic. When it does evaluate, it writes the result. So the TargetingState write mirrors CurrentTarget exactly:

```rust
// After: current_target.0 = nearest;
*targeting_state = match nearest {
    Some(entity) => TargetingState::Engaging(entity),
    None => TargetingState::Seeking,
};
```

2. Add `&mut TargetingState` to the seekers query in `find_target`:
```rust
mut seekers: Query<(
    Entity,
    &Team,
    &GlobalTransform,
    &Collider,
    &mut CurrentTarget,
    &mut TargetingState,  // NEW
    Option<&Movement>,
)>,
```

**Success Criteria (automated):**
- `cargo build` compiles
- AI tests updated to spawn entities with `TargetingState::Seeking`
- AI tests verify `TargetingState` is set correctly alongside `CurrentTarget`

### Phase 3: Death Observer

**Changes Required:**

1. **`gameplay/combat/death.rs`** — Add `On<Remove, Target>` observer:

```rust
use crate::gameplay::TargetingState;

/// When a targetable entity dies (Target removed during despawn),
/// transition all orphaned Engaging/Attacking units to Seeking.
fn handle_target_death(
    trigger: On<Remove, Target>,
    mut seekers: Query<&mut TargetingState>,
) {
    let dead_entity = trigger.entity;
    for mut state in &mut seekers {
        match *state {
            TargetingState::Engaging(e) | TargetingState::Attacking(e) if e == dead_entity => {
                *state = TargetingState::Seeking;
            }
            _ => {}
        }
    }
}
```

2. Register observer in `combat/death.rs::plugin`:
```rust
pub(super) fn plugin(app: &mut App) {
    app.add_observer(handle_target_death);
    app.add_systems(
        Update,
        check_death
            .in_set(DeathCheck)
            .in_set(GameSet::Death)
            .run_if(gameplay_running),
    );
}
```

**Success Criteria (automated):**
- Observer fires when entity with `Target` is despawned
- Orphaned `Engaging(dead)` units transition to `Seeking`
- Units targeting a different entity are unaffected
- Units in `Seeking`/`Moving` state are unaffected

### Phase 4: Spawn Site Updates

**Changes Required:**

1. **`gameplay/units/mod.rs:spawn_unit()`** — Add `TargetingState::Seeking` to the spawn bundle (line ~106, alongside `CurrentTarget(None)`)

2. **`gameplay/battlefield/renderer.rs`** — Add `TargetingState::Seeking` to both fortress spawns (player ~87, enemy ~169), alongside their existing `CurrentTarget(None)`

3. **`gameplay/building/placement.rs`** — Buildings have `Target` but no `CurrentTarget`/`TargetingState` (they are targets, not seekers). **No change needed.**

**Success Criteria (automated):**
- All spawned seekers (units + fortresses) have `TargetingState::Seeking` on spawn
- Buildings remain unchanged (Target only)

### Phase 5: Test Helper Updates + New Tests

**Changes Required:**

1. **`src/testing.rs`** — Add `TargetingState::Seeking` to `spawn_test_unit()` (alongside `CurrentTarget(None)`)

2. **`src/testing.rs`** — `spawn_test_target()` does NOT get `TargetingState` (it's a minimal target entity, not a seeker)

3. **`gameplay/ai.rs` tests** — Update `create_ai_test_app()` if needed. All existing AI tests spawn via `spawn_test_unit` and `spawn_test_target`, so they'll get `TargetingState` automatically from the helper update. Add assertions for `TargetingState` in key tests.

4. **`gameplay/combat/death.rs` tests** — Add new test module:

```rust
#[cfg(test)]
mod observer_tests {
    use super::*;
    use crate::gameplay::{CurrentTarget, TargetingState, Team};
    use crate::testing::{assert_entity_count, spawn_test_unit, spawn_test_target};

    fn create_death_observer_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_observer(handle_target_death);
        app.add_systems(Update, check_death);
        app
    }

    #[test]
    fn orphaned_engaging_unit_transitions_to_seeking() { ... }

    #[test]
    fn orphaned_attacking_unit_transitions_to_seeking() { ... }

    #[test]
    fn unit_targeting_different_entity_unaffected() { ... }

    #[test]
    fn seeking_unit_unaffected_by_death() { ... }

    #[test]
    fn multiple_orphans_all_transition() { ... }
}
```

5. **`gameplay/battlefield/` integration tests** — If any exist that check fortress components, add `TargetingState::Seeking` assertion.

**Success Criteria (automated):**
- All existing tests pass
- New death observer tests pass
- `cargo test` green
- `make check` passes (clippy + build)

---

## Testing Strategy

| Test | Type | Location | What it verifies |
|------|------|----------|-----------------|
| Existing AI tests | Updated | `ai.rs` | TargetingState set alongside CurrentTarget |
| `orphaned_engaging_unit_transitions_to_seeking` | New | `death.rs` | Observer transitions Engaging→Seeking on target death |
| `orphaned_attacking_unit_transitions_to_seeking` | New | `death.rs` | Observer transitions Attacking→Seeking on target death |
| `unit_targeting_different_entity_unaffected` | New | `death.rs` | Observer only affects units targeting the dead entity |
| `seeking_unit_unaffected_by_death` | New | `death.rs` | Seeking units stay Seeking |
| `multiple_orphans_all_transition` | New | `death.rs` | All orphaned units (not just first) transition |
| Existing death tests | Unchanged | `death.rs` | Health-based despawn still works |
| Existing movement/attack/pathfinding tests | Unchanged | Various | No regressions (don't read TargetingState) |

## Performance Considerations

- **Death observer O(n) scan**: Iterates all `TargetingState` entities per death event. At current scale (hundreds of units) this is negligible. At 40k+ with mass death, consider reverse-lookup index (deferred to Ticket 6).
- **Component memory**: `TargetingState` is 12 bytes (enum discriminant + Entity). `EngagementLeash` is 12 bytes (Vec2 + f32). Temporary ~24 bytes/entity overhead during coexistence with `CurrentTarget`.

## References

- Research doc: `thoughts/shared/research/2026-03-05-targeting-movement-combat-scalability.md` (Section 9, Ticket 1a)
- Linear ticket: [GAM-57](https://linear.app/tayhu-games/issue/GAM-57/add-targetingstate-death-observer)
- Blocked ticket: [GAM-58](https://linear.app/tayhu-games/issue/GAM-58/migrate-consumers-to-targetingstate-remove-currenttarget)
