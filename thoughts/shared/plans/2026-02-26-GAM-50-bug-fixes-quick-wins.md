# Bug Fixes & Quick Wins (GAM-50) Implementation Plan

## Overview

Fix 3 bugs and apply 5 code quality improvements identified during the 5-agent architecture review. All changes are small, isolated, and independently verifiable.

## Current State Analysis

### Bugs Found

1. **No health clamping on damage** — `attack.rs:172` does `health.current -= projectile.damage` with no floor. Health can go arbitrarily negative. While `award_kill_gold` iterates entities (not hits) so double-reward within a single frame is unlikely, negative health is incorrect state that could cause subtle issues (health bar rendering, future systems that read health).

2. **GridIndex not cleared on re-entry** — `GridIndex` is `init_resource` once at startup (`battlefield/mod.rs:192`). `spawn_battlefield` overwrites all 60 keys but never explicitly clears the HashMap. Currently safe because the grid geometry is fixed, but fragile — any future grid size change would leave stale entries.

3. **UnitAssets leaked on re-entry** — `setup_unit_assets` (`units/mod.rs:190`) unconditionally creates new mesh/material handles on every `OnEnter(InGame)`. Old handles remain in `Assets<Mesh>` and `Assets<ColorMaterial>` with no cleanup.

### Quick Improvements Found

4. **`Team::opposing()` missing** — opposing team logic is an inline `match` block in `ai.rs:83-86`. Will be needed everywhere as combat grows.

5. **`Z_PROJECTILE` missing** — magic `Z_UNIT + 0.5` at `attack.rs:101`. All other Z layers have named constants.

6. **`solid_entity_layers()` missing** — `CollisionLayers::new([Pushbox, Hurtbox], [Pushbox, Hitbox])` repeated 4× across 3 files.

7. **`spawn_unit` takes `Vec3` but only uses x,y** — both call sites do `spawn_xy.extend(Z_UNIT)` just to have Z ignored (`units/mod.rs:127` hardcodes `Z_UNIT`).

8. **Stale resources on re-entry** — `RetargetTimer` and `PathRefreshTimer` have no `OnEnter` reset. `SpatialHash` is already fine (clears itself every frame in `rebuild_spatial_hash`).

## Desired End State

- Health never goes negative after damage
- All resources are properly reset when re-entering `GameState::InGame`
- `UnitAssets` is created once, not leaked on re-entry
- Common patterns (`Team::opposing()`, `solid_entity_layers()`, `Z_PROJECTILE`) are centralized
- `spawn_unit` has a clean `Vec2` API
- All existing tests pass, new tests cover the bug fixes

### Key Discoveries:
- `award_kill_gold` (`income.rs:32`) iterates entities, not hits — double-reward from one entity per frame doesn't happen, but negative health is still wrong state
- `SpatialHash` already self-clears every frame (`rebuild_spatial_hash` calls `hash.clear()`) — no fix needed
- `GridIndex.slots` is private — need to add a `clear()` method
- Two `spawn_unit` call sites: `spawn.rs:110` and `production.rs:119`

## What We're NOT Doing

- Not adding a `Dead` marker component (health clamping + existing despawn timing is sufficient)
- Not fixing `SpatialHash` (already self-clears)
- Not changing `award_kill_gold` logic (the per-entity iteration is correct)
- Not refactoring `CollisionLayer` enum or physics setup
- Not expanding test coverage beyond the fixes in this ticket (that's GAM-52)

## Implementation Approach

Three phases: bug fixes first (most impactful), then quick improvements (mechanical refactors), then tests. Each phase is independently verifiable.

---

## Phase 1: Bug Fixes

### Overview
Fix the 3 real bugs: health clamping, GridIndex stale state, UnitAssets leak.

### Changes Required:

#### 1. Clamp health at 0 after damage
**File**: `src/gameplay/combat/attack.rs`
**Line**: 172

```rust
// Before:
health.current -= projectile.damage;

// After:
// Clamp at zero — negative health is invalid state. Double kill-reward is
// prevented by system ordering: `award_kill_gold` and `check_death` both
// run in `GameSet::Death` (award first, then despawn), so each entity is
// seen exactly once. If ordering changes, add a `Dead` marker component
// and filter `Without<Dead>` in `award_kill_gold`.
health.current = (health.current - projectile.damage).max(0.0);
```

#### 2. Clear GridIndex on re-entry
**File**: `src/gameplay/battlefield/mod.rs`

Add a `clear()` method to `GridIndex`:

```rust
impl GridIndex {
    /// Remove all entries (used on state re-entry before repopulating).
    pub fn clear(&mut self) {
        self.slots.clear();
    }

    // ... existing methods
}
```

**File**: `src/gameplay/battlefield/renderer.rs`
**Line**: 27 (top of `spawn_battlefield`)

```rust
pub(super) fn spawn_battlefield(mut commands: Commands, mut grid_index: ResMut<GridIndex>) {
    grid_index.clear(); // Reset stale entity refs from previous session
    // ... rest unchanged
```

#### 3. Guard UnitAssets with existence check
**File**: `src/gameplay/units/mod.rs`
**Lines**: 190-200

```rust
fn setup_unit_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    existing: Option<Res<UnitAssets>>,
) {
    if existing.is_some() {
        return; // Already created — don't leak handles
    }
    commands.insert_resource(UnitAssets {
        mesh: meshes.add(Circle::new(UNIT_RADIUS)),
        player_material: materials.add(palette::PLAYER_UNIT),
        enemy_material: materials.add(palette::ENEMY_UNIT),
    });
}
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes (clippy + type checking)
- [x] `make test` passes (all existing tests)

#### Manual Verification:
- [x] Start a game, let units fight, observe no negative health bars
- [x] Exit to main menu and start a new game — no crashes, grid works correctly

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation before proceeding to Phase 2.

---

## Phase 2: Quick Improvements

### Overview
Apply 5 code quality improvements: `Team::opposing()`, `Z_PROJECTILE`, `solid_entity_layers()`, `spawn_unit` Vec2, resource resets.

### Changes Required:

#### 4. Add `Team::opposing()` method
**File**: `src/gameplay/mod.rs`
**After line 38** (after enum definition)

```rust
impl Team {
    /// Returns the opposing team.
    #[must_use]
    pub const fn opposing(self) -> Self {
        match self {
            Self::Player => Self::Enemy,
            Self::Enemy => Self::Player,
        }
    }
}
```

**File**: `src/gameplay/ai.rs`
**Line**: 83-86 — replace inline match:

```rust
// Before:
let opposing_team = match team {
    Team::Player => Team::Enemy,
    Team::Enemy => Team::Player,
};

// After:
let opposing_team = team.opposing();
```

#### 5. Add `Z_PROJECTILE` constant
**File**: `src/lib.rs`
**After line 35** (after `Z_UNIT`):

```rust
/// Projectiles (above units).
pub(crate) const Z_PROJECTILE: f32 = 4.5;
```

**File**: `src/gameplay/combat/attack.rs`
**Line**: 9 — add to import:

```rust
use crate::{GameSet, Z_UNIT, Z_PROJECTILE, gameplay_running};
```

**Line**: 101 — replace magic number:

```rust
// Before:
Z_UNIT + 0.5,

// After:
Z_PROJECTILE,
```

Also update the Z-layer ordering test in `src/lib.rs` (line 139) to include `Z_PROJECTILE`:

```rust
assert!(Z_UNIT < Z_PROJECTILE); // new assertion
```

#### 6. Add `solid_entity_layers()` helper
**File**: `src/third_party/avian.rs`
**After line 25** (after `CollisionLayer` enum):

```rust
/// Collision layers for solid game entities (units, buildings, fortresses).
///
/// - Member of: Pushbox + Hurtbox (can be pushed and can be damaged)
/// - Collides with: Pushbox + Hitbox (blocked by solids, hit by projectiles)
#[must_use]
pub fn solid_entity_layers() -> CollisionLayers {
    CollisionLayers::new(
        [CollisionLayer::Pushbox, CollisionLayer::Hurtbox],
        [CollisionLayer::Pushbox, CollisionLayer::Hitbox],
    )
}
```

Replace all 4 occurrences:

| File | Line | Entity |
|------|------|--------|
| `src/gameplay/units/mod.rs` | 134-137 | Units |
| `src/gameplay/building/placement.rs` | 150-153 | Buildings |
| `src/gameplay/battlefield/renderer.rs` | 98-101 | Player fortress |
| `src/gameplay/battlefield/renderer.rs` | 183-186 | Enemy fortress |

Each replacement:
```rust
// Before:
CollisionLayers::new(
    [CollisionLayer::Pushbox, CollisionLayer::Hurtbox],
    [CollisionLayer::Pushbox, CollisionLayer::Hitbox],
)

// After:
solid_entity_layers()
```

Update imports in affected files to include `solid_entity_layers`.

#### 7. Change `spawn_unit` param from `Vec3` to `Vec2`
**File**: `src/gameplay/units/mod.rs`
**Line**: 90 — change signature:

```rust
// Before:
position: Vec3,

// After:
position: Vec2,
```

Body at line 127 stays the same (already uses `position.x, position.y`).

**Call site 1**: `src/gameplay/units/spawn.rs:110`
```rust
// Before:
spawn_xy.extend(Z_UNIT),

// After:
spawn_xy,
```

**Call site 2**: `src/gameplay/building/production.rs:119`
```rust
// Before:
spawn_xy.extend(Z_UNIT),

// After:
spawn_xy,
```

Remove unused `Z_UNIT` imports from `spawn.rs` and `production.rs` if they become dead imports after this change. Check each file:
- `spawn.rs:8` — `use crate::{GameSet, Z_UNIT, gameplay_running};` → remove `Z_UNIT` if unused
- `production.rs:7` — `use crate::Z_UNIT;` → remove if unused

#### 8. Reset stale resources on state re-entry
**File**: `src/gameplay/ai.rs`
**After line 125** (after existing systems registration), add reset system:

```rust
fn reset_retarget_timer(mut commands: Commands) {
    commands.insert_resource(RetargetTimer::default());
}
```

Register in plugin (after `init_resource` at line 120):
```rust
app.add_systems(OnEnter(GameState::InGame), reset_retarget_timer);
```

Add import for `GameState`:
```rust
use crate::screens::GameState;
```

**File**: `src/gameplay/units/mod.rs`
Add reset system for `PathRefreshTimer`:

```rust
fn reset_path_refresh_timer(mut commands: Commands) {
    commands.insert_resource(pathfinding::PathRefreshTimer::default());
}
```

Register after the existing `OnEnter(GameState::InGame)` system (line 218):
```rust
app.add_systems(
    OnEnter(GameState::InGame),
    (setup_unit_assets, reset_path_refresh_timer),
);
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes

#### Manual Verification:
- [x] Game plays identically to before (no visual/behavioral changes)
- [x] Exit to main menu → start new game works smoothly

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation before proceeding to Phase 3.

---

## Phase 3: Tests

### Overview
Add tests for the new code: `Team::opposing()`, `solid_entity_layers()`, `Z_PROJECTILE` ordering, health clamping, `GridIndex::clear()`.

### Changes Required:

#### Tests for `Team::opposing()`
**File**: `src/gameplay/units/mod.rs` (existing `tests` module, near line 253)

```rust
#[test]
fn team_opposing_returns_other_team() {
    use crate::gameplay::Team;
    assert_eq!(Team::Player.opposing(), Team::Enemy);
    assert_eq!(Team::Enemy.opposing(), Team::Player);
}
```

#### Test for `solid_entity_layers()`
**File**: `src/third_party/avian.rs` (existing `tests` module)

```rust
#[test]
fn solid_entity_layers_is_pushbox_hurtbox() {
    let layers = solid_entity_layers();
    // Verify it produces a valid CollisionLayers (non-default)
    let expected = CollisionLayers::new(
        [CollisionLayer::Pushbox, CollisionLayer::Hurtbox],
        [CollisionLayer::Pushbox, CollisionLayer::Hitbox],
    );
    assert_eq!(layers, expected);
}
```

Note: verify `CollisionLayers` derives `PartialEq`. If not, this test can be omitted (the 4 call sites already provide integration coverage).

#### Test for health clamping
**File**: `src/gameplay/combat/attack.rs` (existing `integration_tests` module)

```rust
#[test]
fn projectile_hit_clamps_health_at_zero() {
    let mut app = create_hit_test_app();

    let enemy = app
        .world_mut()
        .spawn((Team::Enemy, Health::new(10.0)))
        .id();
    // Damage exceeds HP — health should clamp to 0, not go negative
    spawn_test_projectile(app.world_mut(), Team::Player, enemy, 50.0, &[enemy]);

    app.update();

    let health = app.world().get::<Health>(enemy).unwrap();
    assert_eq!(health.current, 0.0); // Not -40.0
}
```

#### Test for `GridIndex::clear()`
**File**: `src/gameplay/battlefield/mod.rs` (existing `tests` module)

```rust
#[test]
fn grid_index_clear_removes_all_entries() {
    let mut index = GridIndex::default();
    index.insert(0, 0, Entity::from_bits(1));
    index.insert(1, 1, Entity::from_bits(2));
    index.clear();
    assert_eq!(index.get(0, 0), None);
    assert_eq!(index.get(1, 1), None);
}
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes
- [x] All new tests pass individually

#### Manual Verification:
- [x] None needed — all changes in this phase are test-only

---

## Testing Strategy

### Unit Tests (new):
- `Team::opposing()` — both variants
- `solid_entity_layers()` — returns expected layers
- `GridIndex::clear()` — removes all entries
- Health clamping — overkill damage clamps to 0

### Existing Tests (must continue passing):
- All combat tests in `attack.rs` — health assertions change from allowing negative to expecting 0
- All kill reward tests in `income.rs` — the `negative_hp_enemy` test spawns `Health { current: -10.0, ... }` directly, unaffected by clamp
- All building placement tests — `solid_entity_layers()` is a pure refactor
- All AI tests — `team.opposing()` is a pure refactor

### Manual Testing Steps:
1. Start a game, place barracks, let units fight for 2+ minutes
2. Observe health bars never show negative values
3. Exit to main menu via pause menu
4. Start a new game — verify grid, units, and combat work correctly
5. Repeat step 3-4 to verify no resource leaks across sessions

## References

- Linear ticket: [GAM-50](https://linear.app/tayhu-games/issue/GAM-50/bug-fixes-and-quick-wins-architecture-review-phase-1)
- Blocks: GAM-51 (ARCHITECTURE.md overhaul), GAM-52 (test coverage push)
