# GAM-25: Clean Up avian2d Physics References Before State Transitions

## Overview

Eliminate noisy `WARN avian2d::dynamics::solver::islands::sleeping: Tried to wake body ... that does not exist` warnings that fire when `DespawnOnExit` batch-despawns physics entities during state transitions (e.g., returning to main menu from victory/defeat).

## Current State Analysis

When transitioning from `GameState::InGame` to `GameState::MainMenu`, `DespawnOnExit` despawns all game entities simultaneously. During despawn, avian2d's `On<Remove, RigidBody>` observer queues `WakeIslands` / `WakeBody` commands for other entities in the same physics island. By the time those commands execute, the target entities have been despawned, triggering warnings.

### Key Discoveries:
- **Units** have `RigidBody::Dynamic` + `Collider::circle` (`src/gameplay/units/mod.rs:157-164`)
- **Projectiles** have `RigidBody::Kinematic` + `Collider::circle` + `Sensor` (`src/gameplay/combat/attack.rs:107-112`)
- `WakeBody::apply()` at `sleeping.rs:445-453` checks `world.get::<BodyIslandNode>(entity)` — returns `None` for despawned entities → logs warning
- `remove_body_on` at `narrow_phase/mod.rs:463-500` checks `body_islands.get_mut(entity)` for `BodyIslandNode` — if absent, skips `WakeIslands`
- `BodyIslandNode` is `pub` in avian2d, has `#[component(on_remove = ...)]` hook that cleanly unlinks from the island
- `OnExit(GameState::InGame)` runs **before** `DespawnOnExit` in Bevy's state transition schedule, with `apply_deferred` between them

## Desired End State

No avian2d physics warnings appear in the console during state transitions. All physics cleanup happens cleanly before entities are despawned.

### How to verify:
1. `make check` and `make test` pass
2. Play the game, trigger victory or defeat, press Q to return to main menu — no warnings in console output

## What We're NOT Doing

- Not suppressing warnings via log filters (masks real issues)
- Not changing entity spawning patterns or physics architecture
- Not modifying avian2d's internal behavior

## Implementation Approach

Add a single `OnExit(GameState::InGame)` system in the avian wrapper that strips physics components in a specific order:

1. **Remove `BodyIslandNode` first** — the `on_remove` hook cleanly unlinks the entity from its physics island. After this, avian2d no longer knows about the entity.
2. **Remove `RigidBody`** — the `On<Remove, RigidBody>` observer fires but `body_islands.get_mut(entity)` fails (BodyIslandNode gone) → no `WakeIslands` queued.
3. **Remove `Collider`** — same pattern, no island wake.

When `DespawnOnExit` then despawns these entities, they have no physics components → no avian2d observers fire → no warnings.

## Verified API Patterns (avian2d 0.5.0)

- `BodyIslandNode` is `pub` at `avian2d::dynamics::solver::islands::BodyIslandNode` — may or may not be in prelude, verify import path
- `WakeBody::apply()` checks `world.get::<BodyIslandNode>(entity)` — `None` → warning
- `remove_body_on` checks `body_islands.get_mut(entity)` — `Err` → skips `WakeIslands` → no `WakeBody`
- Commands within `apply_deferred` are processed FIFO — sequential `remove` calls on the same entity execute in order

## Phase 1: Add Physics Cleanup System

### Overview
Add a system to `src/third_party/avian.rs` that strips physics components before state-scoped entity despawn.

### Changes Required:

#### 1. `src/third_party/avian.rs`

**Add import** for `GameState`:

```rust
use crate::screens::GameState;
```

**Add import** for `BodyIslandNode` (verify exact path — try prelude first, fall back to full path):

```rust
// Try: already in avian2d::prelude via wildcard import
// Fallback: use avian2d::dynamics::solver::islands::BodyIslandNode;
```

**Add system**:

```rust
/// Strips avian2d physics components from all rigid bodies before
/// `DespawnOnExit` runs. This prevents "Tried to wake body" warnings
/// caused by batch-despawning entities that share a physics island.
///
/// The removal order matters:
/// 1. `BodyIslandNode` — unlinks from island (via on_remove hook)
/// 2. `RigidBody` — observer can't find island node → no wake commands
/// 3. `Collider` — same, no island wake
fn strip_physics_before_despawn(
    mut commands: Commands,
    physics_entities: Query<Entity, With<RigidBody>>,
) {
    for entity in &physics_entities {
        commands.entity(entity).remove::<BodyIslandNode>();
        commands.entity(entity).remove::<(RigidBody, Collider)>();
    }
}
```

**Register in plugin function** (add to existing `plugin` fn):

```rust
pub(super) fn plugin(app: &mut App) {
    app.add_plugins(PhysicsPlugins::default().with_length_unit(CELL_SIZE));
    app.insert_resource(Gravity::ZERO);

    // Strip physics components before DespawnOnExit batch-despawns entities.
    // Prevents avian2d "Tried to wake body" warnings during state transitions.
    app.add_systems(OnExit(GameState::InGame), strip_physics_before_despawn);
}
```

#### 2. Tests in `src/third_party/avian.rs`

Add integration test verifying the cleanup system removes physics components:

```rust
#[cfg(test)]
mod cleanup_tests {
    use super::*;
    use bevy::state::app::StatesPlugin;
    use crate::testing::assert_entity_count;

    #[test]
    fn strip_physics_removes_rigid_bodies_on_exit_ingame() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.add_systems(OnExit(GameState::InGame), strip_physics_before_despawn);

        // Transition to InGame
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();

        // Spawn physics entities (simulating units/projectiles)
        app.world_mut().spawn((
            RigidBody::Dynamic,
            Collider::circle(12.0),
            DespawnOnExit(GameState::InGame),
        ));
        app.world_mut().spawn((
            RigidBody::Kinematic,
            Collider::circle(3.0),
            DespawnOnExit(GameState::InGame),
        ));

        assert_entity_count::<With<RigidBody>>(&mut app, 2);

        // Transition to MainMenu — triggers OnExit(InGame)
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::MainMenu);
        app.update();
        app.update(); // Apply deferred

        // Physics components should be gone (entities despawned by DespawnOnExit)
        assert_entity_count::<With<RigidBody>>(&mut app, 0);
        assert_entity_count::<With<Collider>>(&mut app, 0);
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes (no clippy/compile errors)
- [ ] `make test` passes (all existing + new tests)

#### Manual Verification:
- [ ] Run the game, spawn units via barracks, let combat play out
- [ ] Trigger victory (destroy enemy fortress) or defeat (player fortress destroyed)
- [ ] Press Q to return to main menu
- [ ] **No** `WARN avian2d::dynamics::solver::islands::sleeping: Tried to wake body` messages in console
- [ ] **No** `WARN bevy_ecs::error::handler: Entity despawned` messages in console
- [ ] Start a new game from main menu — physics works normally (units move, projectiles fire)

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation that the warnings are eliminated.

---

## Testing Strategy

### Unit Tests:
- Verify `strip_physics_before_despawn` removes `RigidBody` and `Collider` from entities during state transition

### Manual Testing Steps:
1. `cargo run` — start the game
2. Place barracks, let units spawn and fight
3. Wait for endgame (or use dev tools to trigger it)
4. Press Q at victory/defeat screen
5. Check terminal output for avian2d warnings
6. Re-enter game from main menu and verify physics still works

## Performance Considerations

Negligible — the cleanup system runs once per state transition, iterating only entities with `RigidBody` (typically < 100 in a game session).

## References

- Linear ticket: [GAM-25](https://linear.app/tayhu-games/issue/GAM-25/clean-up-avian2d-physics-references-before-state-transitions)
- Introduced in: [GAM-10](https://linear.app/tayhu-games/issue/GAM-10/add-unit-physics) (unit physics)
- avian2d source: `~/.cargo/registry/src/.../avian2d-0.5.0/src/dynamics/solver/islands/sleeping.rs:445-453` (WakeBody warning)
- avian2d source: `~/.cargo/registry/src/.../avian2d-0.5.0/src/collision/narrow_phase/mod.rs:463-500` (remove_body_on)
