# Fortresses as Damageable Entities (GAM-8) Implementation Plan

## Overview

Make both fortresses damageable entities with health that units can attack. This is a small ticket because all existing systems (combat, death, health bars, targeting) are already generic — the main work is adding `Health` to fortress spawns and making health bars configurable for fortress-sized entities.

## Current State Analysis

**Fortresses already have:**
- `PlayerFortress` / `EnemyFortress` marker components (`battlefield/mod.rs:71-79`)
- `Team::Player` / `Team::Enemy` (`renderer.rs:49,93`)
- `Target` marker — makes them targetable by enemy units (`renderer.rs:50,94`)
- `DespawnOnExit(GameState::InGame)` cleanup

**Fortresses are missing:**
- `Health` component — needed for damage + death
- Appropriately-sized health bars (current bars are 20x3px, designed for tiny unit circles)

**Existing generic systems that "just work" once Health is added:**
- `move_projectiles` (`combat/mod.rs:118-150`) — applies damage to any entity with `Health`
- `check_death` (`combat/mod.rs:154-160`) — despawns any entity with `Health.current <= 0.0`
- `spawn_health_bars` (`combat/mod.rs:164-187`) — triggers on `Added<Health>`
- `update_health_bars` (`combat/mod.rs:191-205`) — reads `Health` + `Children`
- `unit_find_target` (`units/ai.rs:16-66`) — already targets fortresses via `Target` + `Team`

### Key Discoveries:
- Health bar system uses hardcoded `HEALTH_BAR_WIDTH` (20px) in both `spawn_health_bars` and `update_health_bars` (`combat/mod.rs:20,201`) — needs to be configurable
- Fortress sprite is 128x640px — health bar should be ~100x6px positioned above the fortress
- GAM-9 (Victory/Defeat) depends on this ticket — needs fortress entities to be destroyable. When fortress despawns at 0 HP, GAM-9 can detect it via empty `Query<&PlayerFortress>`.

## Desired End State

Both fortresses spawn with ~2000 HP and large, prominent health bars. Units attack enemy fortresses when no closer enemies exist (already works). Projectile damage reduces fortress HP visibly. At 0 HP, the fortress despawns (existing death system).

### How to verify:
1. Run `cargo run` — both fortresses should display health bars
2. Place barracks to spawn player units — they'll march right and attack the enemy fortress
3. Wait for enemy waves to reach the player fortress — HP should decrease
4. Let enough enemies through — player fortress should eventually despawn at 0 HP
5. All automated tests pass: `make check && make test`

## What We're NOT Doing

- Victory/Defeat screens (GAM-9)
- Fortress-specific death animations or effects
- Fortress repair mechanics
- Different fortress types or stats

## Implementation Approach

Single phase — add `HealthBarConfig` as a required component on all entities with health (units + fortresses), update the health bar systems to read it, add `Health` to fortresses.

## Phase 1: HealthBarConfig Component + Fortress Health

### Overview
Add a `HealthBarConfig` component that controls health bar width/height/offset. All entities with `Health` must also have `HealthBarConfig`. Update `spawn_health_bars` and `update_health_bars` to read it. Add `Health` + `HealthBarConfig` to fortress entities. Add `HealthBarConfig` to unit spawn sites.

### Changes Required:

#### 1. New `HealthBarConfig` component
**File**: `src/gameplay/combat/mod.rs`
**Changes**: Add component after existing health bar components (after line 56)

```rust
/// Configuration for health bar sizing. Required on all entities with `Health`.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct HealthBarConfig {
    pub width: f32,
    pub height: f32,
    pub y_offset: f32,
}
```

Register the type in the plugin function (line ~210):
```rust
app.register_type::<HealthBarConfig>();
```

#### 2. Add unit health bar constants
**File**: `src/gameplay/combat/mod.rs`
**Changes**: Rename existing constants to clarify they're defaults for units, or keep as-is and just use them in unit spawn sites.

Keep existing constants as-is (they become the values used by unit spawns):
```rust
const HEALTH_BAR_WIDTH: f32 = 20.0;   // used by unit spawns
const HEALTH_BAR_HEIGHT: f32 = 3.0;   // used by unit spawns
const HEALTH_BAR_Y_OFFSET: f32 = 18.0; // used by unit spawns
```

Export them for use by unit spawn sites:
```rust
pub const UNIT_HEALTH_BAR_WIDTH: f32 = 20.0;
pub const UNIT_HEALTH_BAR_HEIGHT: f32 = 3.0;
pub const UNIT_HEALTH_BAR_Y_OFFSET: f32 = 18.0;
```

#### 3. Update `spawn_health_bars` to read config
**File**: `src/gameplay/combat/mod.rs`
**Changes**: Modify query to include `&HealthBarConfig`, use config values

```rust
fn spawn_health_bars(
    mut commands: Commands,
    new_entities: Query<(Entity, &HealthBarConfig), Added<Health>>,
) {
    for (entity, config) in &new_entities {
        commands.entity(entity).with_children(|parent| {
            parent.spawn((
                Sprite::from_color(HEALTH_BAR_BG_COLOR, Vec2::new(config.width, config.height)),
                Transform::from_xyz(0.0, config.y_offset, 1.0),
                HealthBarBackground,
            ));
            parent.spawn((
                Sprite::from_color(HEALTH_BAR_FILL_COLOR, Vec2::new(config.width, config.height)),
                Transform::from_xyz(0.0, config.y_offset, 1.1),
                HealthBarFill,
            ));
        });
    }
}
```

#### 4. Update `update_health_bars` to read config
**File**: `src/gameplay/combat/mod.rs`
**Changes**: Add `&HealthBarConfig` to query, use config width for left-alignment

```rust
fn update_health_bars(
    health_query: Query<(&Health, &Children, &HealthBarConfig)>,
    mut bar_query: Query<&mut Transform, With<HealthBarFill>>,
) {
    for (health, children, config) in &health_query {
        let ratio = (health.current / health.max).clamp(0.0, 1.0);
        for child in children.iter() {
            if let Ok(mut transform) = bar_query.get_mut(child) {
                transform.scale.x = ratio;
                transform.translation.x = config.width.mul_add(-(1.0 - ratio), 0.0) / 2.0;
            }
        }
    }
}
```

#### 5. Add `HealthBarConfig` to player unit spawns
**File**: `src/gameplay/building/production.rs`
**Changes**: Add `HealthBarConfig` to the unit spawn bundle (line 30-52). Add import for `HealthBarConfig` and the unit health bar constants.

```rust
use crate::gameplay::combat::{
    AttackTimer, HealthBarConfig, UNIT_HEALTH_BAR_HEIGHT, UNIT_HEALTH_BAR_WIDTH,
    UNIT_HEALTH_BAR_Y_OFFSET,
};
```

Add to spawn bundle:
```rust
HealthBarConfig {
    width: UNIT_HEALTH_BAR_WIDTH,
    height: UNIT_HEALTH_BAR_HEIGHT,
    y_offset: UNIT_HEALTH_BAR_Y_OFFSET,
},
```

#### 6. Add `HealthBarConfig` to enemy unit spawns
**File**: `src/gameplay/units/spawn.rs`
**Changes**: Same as above — add `HealthBarConfig` to the enemy unit spawn bundle (line 98-120).

```rust
use crate::gameplay::combat::{
    AttackTimer, HealthBarConfig, UNIT_HEALTH_BAR_HEIGHT, UNIT_HEALTH_BAR_WIDTH,
    UNIT_HEALTH_BAR_Y_OFFSET,
};
```

Add to spawn bundle:
```rust
HealthBarConfig {
    width: UNIT_HEALTH_BAR_WIDTH,
    height: UNIT_HEALTH_BAR_HEIGHT,
    y_offset: UNIT_HEALTH_BAR_Y_OFFSET,
},
```

#### 7. Fortress HP constant
**File**: `src/gameplay/battlefield/mod.rs`
**Changes**: Add constant after existing constants (around line 40)

```rust
/// Fortress hit points. ~2000 HP — moderate buffer. A few leaked enemies are
/// survivable, but 20+ breaking through will destroy the fortress.
pub const FORTRESS_HP: f32 = 2000.0;
```

#### 8. Fortress health bar constants
**File**: `src/gameplay/battlefield/mod.rs`
**Changes**: Add constants for fortress health bar sizing

```rust
/// Fortress health bar dimensions — larger than unit bars for visibility.
const FORTRESS_HEALTH_BAR_WIDTH: f32 = 100.0;
const FORTRESS_HEALTH_BAR_HEIGHT: f32 = 6.0;
/// Y offset from fortress center (half of BATTLEFIELD_HEIGHT + padding).
const FORTRESS_HEALTH_BAR_Y_OFFSET: f32 = 330.0;
```

#### 9. Add Health + HealthBarConfig to fortress spawns
**File**: `src/gameplay/battlefield/renderer.rs`
**Changes**: Add `Health` and `HealthBarConfig` to both fortress spawn bundles

Add imports:
```rust
use crate::gameplay::combat::HealthBarConfig;
use crate::gameplay::units::Health;
use super::{FORTRESS_HP, FORTRESS_HEALTH_BAR_WIDTH, FORTRESS_HEALTH_BAR_HEIGHT, FORTRESS_HEALTH_BAR_Y_OFFSET};
```

Player fortress (line 47-58):
```rust
commands.spawn((
    PlayerFortress,
    Team::Player,
    Target,
    Health::new(FORTRESS_HP),
    HealthBarConfig {
        width: FORTRESS_HEALTH_BAR_WIDTH,
        height: FORTRESS_HEALTH_BAR_HEIGHT,
        y_offset: FORTRESS_HEALTH_BAR_Y_OFFSET,
    },
    Sprite::from_color(PLAYER_FORT_COLOR, fortress_size),
    Transform::from_xyz(
        zone_center_x(PLAYER_FORT_START_COL, FORTRESS_COLS),
        battlefield_center_y(),
        Z_ZONE,
    ),
    DespawnOnExit(GameState::InGame),
));
```

Enemy fortress (line 90-102) — same pattern with `EnemyFortress` + `Team::Enemy`.

#### 10. Update existing tests that spawn Health without HealthBarConfig
**File**: `src/gameplay/combat/mod.rs` (test modules)
**Changes**: All test helpers that spawn entities with `Health` must also add `HealthBarConfig`. Update:
- `spawn_target()` helper — add `HealthBarConfig` with unit defaults
- `create_health_bar_test_app()` tests — entities spawned with `Health` must also get `HealthBarConfig`
- `create_death_test_app()` tests — entities spawned with just `Health` need `HealthBarConfig` too (or the death system doesn't query it, so these may not need it — verify)

Note: `check_death` only queries `(Entity, &Health)` — no `HealthBarConfig` needed there. But `spawn_health_bars` queries `Added<Health>` + `&HealthBarConfig` — so any test that expects health bars to spawn needs `HealthBarConfig` on the entity.

### Success Criteria:

#### Automated Verification:
- [x] `make check` — clippy + format pass
- [x] `make test` — all tests pass (144 tests)
- [x] New tests pass (see Testing Strategy below)

#### Manual Verification:
- [ ] Both fortresses show large health bars (~100px wide) positioned above the fortress sprites
- [ ] Player units march to enemy fortress and attack it — HP bar shrinks
- [ ] Enemy units reach player fortress and attack it — HP bar shrinks
- [ ] Fortress at 0 HP despawns (both sprite and health bar disappear)
- [ ] Unit health bars (20x3px) still appear correctly on units

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation.

---

## Testing Strategy

### Unit Tests:
- `fortress_hp_constant_is_positive` — `assert!(FORTRESS_HP > 0.0)`
- `fortress_health_bar_constants_valid` — widths/heights positive

### Integration Tests:

#### In `battlefield/mod.rs` (existing test module):
- `player_fortress_has_health` — verify `PlayerFortress` entity has `Health` component with `current == max == FORTRESS_HP`
- `enemy_fortress_has_health` — same for `EnemyFortress`
- `fortress_has_health_bar_config` — verify `HealthBarConfig` component present on fortress entities

#### In `combat/mod.rs` (existing test module):
- Update existing `health_bar_spawned_on_entity_with_health` — add `HealthBarConfig` to spawned entity
- Update existing `health_bar_fill_scales_with_damage` — add `HealthBarConfig` to spawned entity
- `health_bar_uses_config_dimensions` — spawn entity with `Health` + `HealthBarConfig { width: 50.0, height: 8.0, y_offset: 40.0 }`, verify child sprites use the configured dimensions
- `update_health_bar_uses_config_width` — spawn entity with Health + HealthBarConfig, damage to 50%, verify fill bar's `translation.x` uses the configured width for left-alignment

#### In `building/production.rs` (existing test module):
- Update `spawned_unit_has_correct_components` — verify unit has `HealthBarConfig`

#### In `units/spawn.rs` (existing test module):
- Update `spawned_enemy_has_all_components` — verify enemy has `HealthBarConfig`

### Key Edge Cases:
- Fortress health bars despawn when fortress despawns (recursive despawn — already tested)
- Health bar fill correctly left-aligns with custom width

## Performance Considerations

None — adding 2 Health components and 4 health bar child entities is negligible.

## References

- Linear ticket: [GAM-8](https://linear.app/tayhu-home-lab/issue/GAM-8/fortresses-as-damageable-entities)
- Dependent ticket: [GAM-9](https://linear.app/tayhu-home-lab/issue/GAM-9/victorydefeat-and-game-loop) (Victory/Defeat — blocked by this)
- Dependencies: GAM-1 (Camera & Battlefield Layout — DONE), GAM-5 (Combat — DONE)
