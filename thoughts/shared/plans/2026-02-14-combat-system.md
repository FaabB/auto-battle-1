# Combat System Implementation Plan (GAM-5)

## Overview

Add the core combat loop: units attack their targets, take damage, show health bars, and die. This delivers the "payoff" ticket where units actually fight, completing the spawn → move → engage → fight → die cycle.

## Current State Analysis

**What exists:**
- `Health { current, max }` component (`units/mod.rs:51-63`) — already on units
- `CombatStats { damage, attack_speed, range }` (`units/mod.rs:66-72`) — already on units
- `CurrentTarget(Option<Entity>)` (`units/mod.rs:88-91`) — set by AI system
- Movement system stops units at attack range (`movement.rs:26-28`)
- `GameSet::Combat`, `GameSet::Death`, `GameSet::Ui` — defined (`lib.rs:50-55`) but no systems registered
- `Z_HEALTH_BAR = 5.0` constant (`lib.rs:32`) — defined but unused (`#[allow(dead_code)]`)
- `ChildOf`/`Children` hierarchy is built into `bevy_ecs` core (no extra feature needed)
- `despawn()` is recursive in Bevy 0.18 (children auto-despawn with parent)

### Key Discoveries:
- Units are spawned with `SOLDIER_ATTACK_SPEED = 1.0` and `SOLDIER_ATTACK_RANGE = 30.0` (`units/mod.rs:15,17`)
- `Z_HEALTH_BAR` has `#[allow(dead_code)]` that needs removal (`lib.rs:31`)
- Existing unit spawning: `building/production.rs:29-47` (player) and `dev_tools/mod.rs:41-59` (debug enemies)

## Desired End State

After implementation:
1. Units in range of their target fire a projectile on their attack timer
2. Projectiles travel from attacker to target; on arrival, damage is applied
3. If a target dies before the projectile arrives, the projectile despawns harmlessly
4. Units at 0 HP are despawned (with children)
5. Every entity with `Health` gets a health bar (two child sprites: red background + green fill)
6. Health bars visually shrink as damage is taken
7. No orphan entities after combat (no ghost health bars, no stale projectiles)
8. Surviving units resume moving toward the opposing fortress

### Verification:
- `make check` passes (clippy + format)
- `make test` passes (all existing + ~15 new tests)
- Manual: spawn player units (barracks) + enemies (E key) → they engage, small yellow projectiles fly between units, health bars shrink, dead units vanish, survivors resume moving

## What We're NOT Doing

- Kill rewards / gold (GAM-6: Economy)
- Fortress health / fortress targeting with damage (GAM-8: Fortresses as Damageable Entities)
- Wave-based enemy spawning (GAM-7: Wave System)
- Damage types, armor, or stat scaling
- Death animations or visual effects
- Damage numbers or combat log

## Verified API Patterns (Bevy 0.18)

These were verified against the actual `bevy_ecs-0.18.0` source:

- **`ChildOf` / `Children`** — built into `bevy_ecs` core, NOT a separate crate. `ChildOf(parent_entity)` is a component; `Children` is auto-populated on the parent. Both in prelude.
- **`EntityCommands::with_children()`** — `commands.entity(e).with_children(|p| { p.spawn(bundle); })` (`hierarchy.rs:386-392`)
- **`despawn()` IS recursive** — despawns entity + all descendants via `ChildOf` hierarchy (`hierarchy.rs:646-650` test confirms)
- **`Added<T>` query filter** — matches entities where T was added since last system run. In prelude.
- **`Sprite::from_color(color, size)`** — creates colored rectangle sprite. Actively used in codebase (`battlefield/renderer.rs`, `building/placement.rs`).
- **`Children` derefs to `[Entity]`** — iterate with `for &child in children.iter()` (`hierarchy.rs:258-264`).

## Implementation Approach

New module `src/gameplay/combat/mod.rs` with five systems:
1. **`unit_attack`** (`GameSet::Combat`) — tick attack timers, spawn projectile toward target when timer fires and in range
2. **`move_projectiles`** (`GameSet::Combat`) — move projectiles toward target, apply damage on arrival, despawn stale projectiles
3. **`check_death`** (`GameSet::Death`) — despawn entities at 0 HP (generic, works for future fortresses)
4. **`spawn_health_bars`** + **`update_health_bars`** (`GameSet::Ui`) — auto-attach health bar children to any entity with `Health`, update bar width each frame

**Projectile flow:** `unit_attack` spawns a `Projectile` entity via deferred commands. On the *next* frame, `move_projectiles` picks it up and moves it toward the target. When it arrives, damage is applied and the projectile despawns. If the target entity no longer exists, the projectile despawns harmlessly.

---

## Phase 1: Combat Module — Attack & Death Systems

### Overview
Create the combat module with attack timer, damage application, and death/despawn logic.

### Changes Required:

#### 1. New file: `src/gameplay/combat/mod.rs`

**Components:**

```rust
/// Per-unit attack cooldown timer.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct AttackTimer(pub Timer);

/// A projectile in flight toward a target.
/// Spawned by `unit_attack`, moved by `move_projectiles`.
/// Damage is applied when the projectile reaches the target.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct Projectile {
    pub target: Entity,
    pub damage: f32,
    pub speed: f32,
}

/// Marker: red background bar (full width, shows "missing" HP).
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct HealthBarBackground;

/// Marker: green foreground bar (scales with current/max HP).
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct HealthBarFill;
```

**Constants:**

```rust
/// Projectile travel speed (pixels per second).
const PROJECTILE_SPEED: f32 = 200.0;

/// Projectile visual radius (pixels).
const PROJECTILE_RADIUS: f32 = 3.0;

/// Projectile color (yellow).
const PROJECTILE_COLOR: Color = Color::srgb(1.0, 1.0, 0.3);

/// Health bar dimensions (pixels).
const HEALTH_BAR_WIDTH: f32 = 20.0;
const HEALTH_BAR_HEIGHT: f32 = 3.0;

/// Y offset above the entity center for health bars.
const HEALTH_BAR_Y_OFFSET: f32 = 18.0;

/// Health bar colors.
const HEALTH_BAR_BG_COLOR: Color = Color::srgb(0.8, 0.1, 0.1);
const HEALTH_BAR_FILL_COLOR: Color = Color::srgb(0.1, 0.9, 0.1);
```

**Systems:**

```rust
/// Ticks attack timers and spawns projectiles toward targets in range.
/// Runs in GameSet::Combat.
fn unit_attack(
    time: Res<Time>,
    mut attackers: Query<
        (&CurrentTarget, &CombatStats, &mut AttackTimer, &GlobalTransform),
        With<Unit>,
    >,
    positions: Query<&GlobalTransform>,
    mut commands: Commands,
) {
    for (target, stats, mut timer, attacker_pos) in &mut attackers {
        let Some(target_entity) = target.0 else { continue };
        let Ok(target_pos) = positions.get(target_entity) else { continue };

        // Only attack when in range
        let distance = attacker_pos
            .translation()
            .truncate()
            .distance(target_pos.translation().truncate());
        if distance > stats.range {
            continue;
        }

        // Tick and spawn projectile when timer fires
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            commands.spawn((
                Projectile {
                    target: target_entity,
                    damage: stats.damage,
                    speed: PROJECTILE_SPEED,
                },
                Sprite::from_color(
                    PROJECTILE_COLOR,
                    Vec2::splat(PROJECTILE_RADIUS * 2.0),
                ),
                Transform::from_xyz(
                    attacker_pos.translation().x,
                    attacker_pos.translation().y,
                    crate::Z_UNIT + 0.5,
                ),
                DespawnOnExit(GameState::InGame),
            ));
        }
    }
}

/// Moves projectiles toward their targets. On arrival, applies damage and
/// despawns the projectile. If the target no longer exists, despawns the
/// projectile harmlessly.
/// Runs in GameSet::Combat (after unit_attack, no ApplyDeferred between them
/// so newly spawned projectiles start moving next frame).
fn move_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    mut projectiles: Query<(Entity, &Projectile, &mut Transform)>,
    mut healths: Query<&mut Health>,
    positions: Query<&GlobalTransform>,
) {
    for (entity, projectile, mut transform) in &mut projectiles {
        // Target gone — despawn projectile harmlessly
        let Ok(target_pos) = positions.get(projectile.target) else {
            commands.entity(entity).despawn();
            continue;
        };

        let target_translation = target_pos.translation();
        let direction = target_translation.truncate() - transform.translation.truncate();
        let distance = direction.length();
        let move_amount = projectile.speed * time.delta_secs();

        if move_amount >= distance {
            // Arrived — apply damage and despawn
            if let Ok(mut health) = healths.get_mut(projectile.target) {
                health.current -= projectile.damage;
            }
            commands.entity(entity).despawn();
        } else {
            // Move toward target
            let dir = direction / distance;
            transform.translation.x += dir.x * move_amount;
            transform.translation.y += dir.y * move_amount;
        }
    }
}

/// Despawns any entity whose health drops to 0 or below.
/// Runs in GameSet::Death. Generic — works for units, fortresses, etc.
fn check_death(mut commands: Commands, query: Query<(Entity, &Health)>) {
    for (entity, health) in &query {
        if health.current <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}
```

**Health bar systems (also in this file):**

```rust
/// Spawns health bar child entities on any entity that just received a Health component.
/// Runs in GameSet::Ui.
fn spawn_health_bars(mut commands: Commands, new_entities: Query<Entity, Added<Health>>) {
    for entity in &new_entities {
        commands.entity(entity).with_children(|parent| {
            // Red background (full width, always visible)
            parent.spawn((
                Sprite::from_color(HEALTH_BAR_BG_COLOR, Vec2::new(HEALTH_BAR_WIDTH, HEALTH_BAR_HEIGHT)),
                Transform::from_xyz(0.0, HEALTH_BAR_Y_OFFSET, 1.0),
                HealthBarBackground,
            ));
            // Green fill (scales with HP ratio, rendered in front of background)
            parent.spawn((
                Sprite::from_color(HEALTH_BAR_FILL_COLOR, Vec2::new(HEALTH_BAR_WIDTH, HEALTH_BAR_HEIGHT)),
                Transform::from_xyz(0.0, HEALTH_BAR_Y_OFFSET, 1.1),
                HealthBarFill,
            ));
        });
    }
}

/// Updates health bar fill width based on current/max HP.
/// Runs in GameSet::Ui, after spawn_health_bars.
fn update_health_bars(
    health_query: Query<(&Health, &Children)>,
    mut bar_query: Query<&mut Transform, With<HealthBarFill>>,
) {
    for (health, children) in &health_query {
        let ratio = (health.current / health.max).clamp(0.0, 1.0);
        for &child in children.iter() {
            if let Ok(mut transform) = bar_query.get_mut(child) {
                transform.scale.x = ratio;
                // Shift left to keep bar left-aligned as it shrinks
                transform.translation.x = HEALTH_BAR_WIDTH.mul_add(-(1.0 - ratio), 0.0) / 2.0;
            }
        }
    }
}
```

**Plugin registration:**

```rust
pub(super) fn plugin(app: &mut App) {
    app.register_type::<AttackTimer>()
        .register_type::<Projectile>()
        .register_type::<HealthBarBackground>()
        .register_type::<HealthBarFill>();

    // Combat: unit_attack spawns projectiles, move_projectiles resolves them.
    // chain_ignore_deferred so newly spawned projectiles don't move until next frame
    // (prevents instant-hit invisible projectiles).
    app.add_systems(
        Update,
        (unit_attack, move_projectiles)
            .chain_ignore_deferred()
            .in_set(crate::GameSet::Combat)
            .run_if(in_state(GameState::InGame).and(in_state(crate::menus::Menu::None))),
    );

    app.add_systems(
        Update,
        check_death
            .in_set(crate::GameSet::Death)
            .run_if(in_state(GameState::InGame).and(in_state(crate::menus::Menu::None))),
    );

    app.add_systems(
        Update,
        (spawn_health_bars, update_health_bars)
            .chain_ignore_deferred()
            .in_set(crate::GameSet::Ui)
            .run_if(in_state(GameState::InGame).and(in_state(crate::menus::Menu::None))),
    );
}
```

#### 2. Update `src/gameplay/mod.rs`

Add combat module to the compositor:

```rust
pub(crate) mod battlefield;
pub(crate) mod building;
pub(crate) mod combat;
pub(crate) mod units;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((battlefield::plugin, building::plugin, combat::plugin, units::plugin));
}
```

#### 3. Update `src/gameplay/building/production.rs`

Add `AttackTimer` to spawned player units. Add import:

```rust
use crate::gameplay::combat::AttackTimer;
```

In the spawn bundle (after `Movement`), add:

```rust
AttackTimer(Timer::from_seconds(
    1.0 / SOLDIER_ATTACK_SPEED,
    TimerMode::Repeating,
)),
```

#### 4. Update `src/dev_tools/mod.rs`

Add `AttackTimer` to debug-spawned enemy units. Add import:

```rust
use crate::gameplay::combat::AttackTimer;
```

In the spawn bundle (after `Movement`), add:

```rust
AttackTimer(Timer::from_seconds(
    1.0 / SOLDIER_ATTACK_SPEED,
    TimerMode::Repeating,
)),
```

#### 5. Update `src/lib.rs`

Remove `#[allow(dead_code)]` from `Z_HEALTH_BAR` since it's now used by combat module's health bar Z offset calculation (actually — the health bars use relative Z offsets as children, so `Z_HEALTH_BAR` remains unused by the combat module directly). Keep the `#[allow(dead_code)]` for now; GAM-8 may use it for fortress bars with absolute positioning.

Actually, `Z_HEALTH_BAR` won't be used yet since health bars are children with local Z offsets. Leave the `#[allow(dead_code)]` as-is.

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes (clippy, format)
- [ ] `make test` passes (all existing + new tests)
- [ ] New tests cover: projectile spawning on attack, projectile dealing damage on arrival, projectile despawn on missing target, attack out-of-range (no projectile), attack cooldown timing, death at 0 HP, death at negative HP, no death above 0 HP, health bar spawned on entities with Health, health bar fill scales correctly, health bar cleanup on entity death

#### Manual Verification:
- [ ] Spawn barracks (place building) → units spawn and walk right
- [ ] Press E → enemy units appear and walk left
- [ ] Units meet in the combat zone and stop (within attack range)
- [ ] Small yellow projectiles fly from attacker to target
- [ ] Projectiles arrive at target and disappear (not instant)
- [ ] Health bars appear above all units (both player and enemy)
- [ ] Health bars visibly shrink as units take damage (on projectile hit)
- [ ] Units at 0 HP disappear (no ghost entities)
- [ ] Surviving units resume moving toward the opposing fortress
- [ ] No orphan health bar sprites or stale projectiles remain after combat

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding.

---

## Phase 2: Cleanup & Polish

### Overview
Address any issues found during manual testing. This phase is reserved for:
- Adjusting projectile speed/size/color if they look wrong
- Adjusting health bar dimensions/colors if they look wrong
- Fixing edge cases discovered during manual play
- Any timer tuning needed

No pre-planned changes — this phase exists as a buffer for manual testing feedback.

---

## Testing Strategy

### Unit Tests (in `combat/mod.rs`):

1. **`constants_are_valid`** — projectile speed/radius, health bar width/height/offset are all positive

### Integration Tests (in `combat/mod.rs`):

**Attack + projectile tests:**
2. **`unit_spawns_projectile_in_range`** — spawn attacker + target in range, tick past timer → `Projectile` entity exists
3. **`unit_does_not_attack_out_of_range`** — spawn attacker + target far apart → no projectile spawned
4. **`attack_without_target_does_nothing`** — unit with `CurrentTarget(None)` → no crash, no projectile
5. **`projectile_deals_damage_on_arrival`** — spawn projectile near target, tick until arrival → target HP reduced
6. **`projectile_despawns_on_arrival`** — after dealing damage, projectile entity no longer exists
7. **`projectile_despawns_when_target_missing`** — target despawned before projectile arrives → projectile despawns, no crash
8. **`attack_respects_cooldown`** — multiple frames, projectile only on timer fire

**Death system tests:**
9. **`entity_despawned_at_zero_hp`** — entity with `Health { current: 0.0, max: 100.0 }` → despawned
10. **`entity_despawned_at_negative_hp`** — entity with negative HP → despawned
11. **`entity_survives_above_zero_hp`** — entity with `Health { current: 1.0, max: 100.0 }` → not despawned

**Health bar tests:**
12. **`health_bar_spawned_on_entity_with_health`** — entity gets `Health` → gets `HealthBarBackground` + `HealthBarFill` children
13. **`health_bar_fill_scales_with_damage`** — set HP to 50% → fill bar scale.x = 0.5
14. **`health_bar_despawned_with_parent`** — despawn parent → health bar children gone too

### Test Setup Patterns:

**Attack/projectile tests** — need `unit_attack` only, check for spawned `Projectile` entities:

```rust
fn create_attack_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_systems(Update, unit_attack);
    app.update(); // Initialize time (first frame delta=0)
    app
}
```

**Projectile movement tests** — need `move_projectiles` only, spawn `Projectile` entities directly:

```rust
fn create_projectile_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_systems(Update, move_projectiles);
    app.update(); // Initialize time
    app
}
```

**Death tests** — need `check_death` only:

```rust
fn create_death_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_systems(Update, check_death);
    app
}
```

**Health bar tests** — need `spawn_health_bars` + `update_health_bars`. Extra `app.update()` after spawning to process deferred `with_children` commands:

```rust
fn create_health_bar_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_systems(Update, (spawn_health_bars, update_health_bars).chain());
    app
}
```

## Performance Considerations

- `unit_attack` only processes units with `AttackTimer` component, not all entities
- `move_projectiles` iterates all active projectiles — prototype will have < 50 active at once
- `check_death` iterates all entities with `Health` — acceptable for prototype scale (< 100 entities)
- `spawn_health_bars` uses `Added<Health>` — only runs on newly spawned entities, not every frame
- `update_health_bars` iterates all entities with `Health` + `Children` — could be optimized with a `Changed<Health>` filter in the future, but unnecessary for prototype
- Projectiles are lightweight entities (Sprite + Transform + Projectile component) with `DespawnOnExit` for state cleanup

## References

- Linear ticket: GAM-5 (Combat)
- Depends on: GAM-4 (Movement & AI) — DONE
- Blocks: GAM-7 (Wave System), GAM-8 (Fortresses as Damageable Entities)
- Research doc: `thoughts/shared/research/2026-02-04-tano-style-game-research.md` (Section 2.3, 3.2)
