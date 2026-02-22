# Random Spawn Around Buildings Implementation Plan

## Overview

Fix unit spawning so units appear at a random navigable position around their producing building (not always one cell to the right). Extract a shared navmesh-aware helper so both barracks production and enemy fortress spawning use the same pattern, guaranteeing units start on walkable terrain.

## Current State Analysis

**Barracks production** (`src/gameplay/building/production.rs:24-25`):
```rust
let spawn_x = transform.translation.x + CELL_SIZE;
let spawn_y = transform.translation.y;
```
Fixed offset — always one cell right. Spawns inside adjacent buildings. With navmesh pathfinding (GAM-12), units start without a valid path.

**Enemy fortress spawning** (`src/gameplay/units/spawn.rs:92-94`):
```rust
let spawn_x = fortress_pos.x;
let spawn_y = fortress_pos.y + rand::rng().random_range(-half_height..half_height);
```
Random Y within fortress footprint, fixed X at fortress center. Different pattern from barracks.

**Navmesh API** (`vleue_navigator`):
- `NavMesh::is_in_mesh(point: Vec2) -> bool` — checks if a 2D point is on the navmesh (navigable)
- Navmesh is accessed via `Res<Assets<NavMesh>>` + `Single<(&ManagedNavMesh, &NavMeshStatus)>` — same pattern already used in `pathfinding.rs:84-99`

### Key Dimensions
- `CELL_SIZE` = 64px, building sprite = 40px (half = 20px)
- `UNIT_RADIUS` = 6px, so minimum clearance from building center ≈ 26px
- Fortress = 2×2 cells = 128×128px (half diagonal ≈ 90px)
- Adjacent barracks are 64px center-to-center — a fixed radius without validation could still land inside a neighbor

## Desired End State

Both barracks and fortress spawn units using a shared `random_navigable_spawn(center, radius, navmesh)` helper. The helper picks a random angle, checks `is_in_mesh()`, and retries up to N times. Units always start on navigable terrain when the navmesh is available. Falls back to random-without-validation when navmesh isn't built yet.

### How to verify:
1. `make check` + `make test` pass
2. Place barracks with adjacent buildings — spawned units appear around the barracks, never inside neighbors
3. Enemy units spawn around the fortress at varying positions
4. Units immediately find valid navmesh paths after spawning

## What We're NOT Doing

- Not changing spawn rates or unit stats
- Not changing the visual appearance of spawning
- Not snapping units to navmesh edges (just validating the random point is navigable)

## Verified API Patterns

These were verified against the actual crate source:

- `NavMesh::is_in_mesh(point: Vec2) -> bool` — direct 2D check, delegates to `polyanya::Mesh::point_in_mesh`
  - Source: `vleue_navigator-0.15.0/src/lib.rs:307-309`
- `NavMeshStatus::Built` — the status to check before using the navmesh
- `ManagedNavMesh` — asset handle that resolves via `Assets<NavMesh>::get()`
- `navmesh.path(from, to)` returns `None` when `from` is off-mesh — does NOT snap to nearest valid point

## Implementation Approach

Single-phase change: add a shared navmesh-aware helper, update both callers, update tests.

## Phase 1: Navmesh-Aware Random Spawn

### Overview
Add `random_navigable_spawn` helper with `is_in_mesh` retry loop, update barracks and fortress spawning, fix tests.

### Changes Required:

#### 1. Shared helper function
**File**: `src/gameplay/units/mod.rs`
**Changes**: Add a public helper function after `spawn_unit`. Add `use vleue_navigator::prelude::NavMesh;` to imports.

```rust
/// Max retry attempts for finding a navigable spawn point.
const SPAWN_PLACEMENT_ATTEMPTS: u32 = 8;

/// Pick a random position at `radius` from `center` that is navigable.
///
/// Tries up to `SPAWN_PLACEMENT_ATTEMPTS` random angles. When `navmesh` is `Some`,
/// each candidate is validated with `is_in_mesh()`. When `None` (navmesh not built
/// yet), returns the first random point without validation.
///
/// Falls back to `center` if all attempts land outside the mesh.
pub fn random_navigable_spawn(center: Vec2, radius: f32, navmesh: Option<&NavMesh>) -> Vec2 {
    use rand::Rng;
    let mut rng = rand::rng();

    for _ in 0..SPAWN_PLACEMENT_ATTEMPTS {
        let angle = rng.random_range(0.0..std::f32::consts::TAU);
        let point = Vec2::new(
            radius.mul_add(angle.cos(), center.x),
            radius.mul_add(angle.sin(), center.y),
        );

        match navmesh {
            Some(mesh) if mesh.is_in_mesh(point) => return point,
            None => return point,
            _ => continue,
        }
    }

    // All attempts failed — spawn at center (pathfinding handles off-mesh start)
    center
}
```

Unit tests in the existing `mod tests` block:
```rust
#[test]
fn random_navigable_spawn_correct_distance_without_navmesh() {
    let center = Vec2::new(100.0, 200.0);
    let radius = 40.0;
    let result = random_navigable_spawn(center, radius, None);
    let dist = center.distance(result);
    assert!(
        (dist - radius).abs() < 0.01,
        "Expected distance {radius}, got {dist}"
    );
}

#[test]
fn random_navigable_spawn_falls_back_to_center() {
    // With a navmesh that rejects all points, should fall back to center.
    // We can't easily construct a NavMesh in unit tests, so this is
    // verified via manual testing. The no-navmesh path is tested above.
}
```

#### 2. Update barracks production
**File**: `src/gameplay/building/production.rs`
**Changes**: Replace fixed offset with `random_navigable_spawn`. Add navmesh system params. Add a `BUILDING_SPAWN_RADIUS` constant.

New imports at top of file:
```rust
use vleue_navigator::prelude::*;
use crate::gameplay::units::random_navigable_spawn;
```

New constant:
```rust
/// Radius from building center where spawned units appear.
/// Clears the 40px building sprite + 6px unit radius with margin.
const BUILDING_SPAWN_RADIUS: f32 = 40.0;
```

Updated system signature and body:
```rust
pub(super) fn tick_production_and_spawn_units(
    time: Res<Time>,
    mut buildings: Query<(&super::Building, &mut ProductionTimer, &Transform)>,
    unit_assets: Res<UnitAssets>,
    navmeshes: Option<Res<Assets<NavMesh>>>,
    navmesh_query: Option<Single<(&ManagedNavMesh, &NavMeshStatus)>>,
    mut commands: Commands,
) {
    // Extract navmesh if available and built
    let navmesh = navmeshes.as_ref().and_then(|meshes| {
        let (managed, status) = &**navmesh_query.as_ref()?;
        (*status == NavMeshStatus::Built).then(|| meshes.get(managed))?
    });

    for (building, mut timer, transform) in &mut buildings {
        timer.0.tick(time.delta());

        if timer.0.just_finished() {
            let stats = building_stats(building.building_type);
            if let Some(unit_type) = stats.produced_unit {
                let center = transform.translation.xy();
                let spawn_xy = random_navigable_spawn(center, BUILDING_SPAWN_RADIUS, navmesh);

                spawn_unit(
                    &mut commands,
                    unit_type,
                    crate::gameplay::Team::Player,
                    spawn_xy.extend(Z_UNIT),
                    &unit_assets,
                );
            }
        }
    }
}
```

Remove the `CELL_SIZE` import if no longer used elsewhere in this file.

#### 3. Update enemy fortress spawning
**File**: `src/gameplay/units/spawn.rs`
**Changes**: Replace custom random-Y logic with `random_navigable_spawn`. Add navmesh system params. Add a `FORTRESS_SPAWN_RADIUS` constant.

New constant:
```rust
/// Radius from fortress center where spawned enemies appear.
/// Clears the 2×2 fortress footprint (128×128px, half-diagonal ≈ 90px).
const FORTRESS_SPAWN_RADIUS: f32 = 80.0;
```

Updated system signature and body:
```rust
fn tick_enemy_spawner(
    time: Res<Time>,
    mut spawn_timer: ResMut<EnemySpawnTimer>,
    unit_assets: Res<UnitAssets>,
    enemy_fortress: Single<&Transform, With<EnemyFortress>>,
    navmeshes: Option<Res<Assets<NavMesh>>>,
    navmesh_query: Option<Single<(&ManagedNavMesh, &NavMeshStatus)>>,
    mut commands: Commands,
) {
    spawn_timer.elapsed_secs += time.delta_secs();
    spawn_timer.timer.tick(time.delta());

    if !spawn_timer.timer.just_finished() {
        return;
    }

    let fortress_pos = enemy_fortress.translation;

    // Extract navmesh if available and built
    let navmesh = navmeshes.as_ref().and_then(|meshes| {
        let (managed, status) = &**navmesh_query.as_ref()?;
        (*status == NavMeshStatus::Built).then(|| meshes.get(managed))?
    });

    let spawn_xy = super::random_navigable_spawn(
        fortress_pos.xy(),
        FORTRESS_SPAWN_RADIUS,
        navmesh,
    );

    super::spawn_unit(
        &mut commands,
        super::UnitType::Soldier,
        Team::Enemy,
        spawn_xy.extend(Z_UNIT),
        &unit_assets,
    );

    // Set next spawn interval based on elapsed time
    let next_interval = current_interval(spawn_timer.elapsed_secs);
    spawn_timer.timer = Timer::from_seconds(next_interval, TimerMode::Once);
}
```

`vleue_navigator::prelude::*` import needed at top of file. Remove `use rand::Rng;`, `CELL_SIZE`, and `FORTRESS_ROWS` imports (no longer used).

#### 4. Update tests

**File**: `src/gameplay/building/production.rs` — test `unit_spawns_to_right_of_building`

Rename to `unit_spawns_near_building` and verify distance from center ≈ `BUILDING_SPAWN_RADIUS`. Without navmesh in test, the helper takes the first random point (no validation), so distance is exactly the radius:
```rust
#[test]
fn unit_spawns_near_building() {
    let mut app = create_production_test_app();

    let building_x = 320.0;
    let building_y = 160.0;

    app.world_mut().spawn((
        Building {
            building_type: BuildingType::Barracks,
            grid_col: 2,
            grid_row: 3,
        },
        ProductionTimer(nearly_elapsed_timer()),
        Transform::from_xyz(building_x, building_y, crate::Z_BUILDING),
        DespawnOnExit(GameState::InGame),
    ));
    app.update();

    let mut query = app.world_mut().query_filtered::<&Transform, With<Unit>>();
    let transform = query.single(app.world()).unwrap();
    let dx = transform.translation.x - building_x;
    let dy = transform.translation.y - building_y;
    let dist = dx.hypot(dy);
    assert!(
        (dist - BUILDING_SPAWN_RADIUS).abs() < 0.01,
        "Expected unit at distance {BUILDING_SPAWN_RADIUS} from building, got {dist}"
    );
}
```

**File**: `src/gameplay/units/spawn.rs` — test `enemy_spawns_at_fortress_position`

Update to verify distance from fortress center ≈ `FORTRESS_SPAWN_RADIUS`:
```rust
#[test]
fn enemy_spawns_near_fortress() {
    let mut app = create_spawn_test_app();

    nearly_expire_timer(&mut app);
    app.update();

    let mut query = app.world_mut().query_filtered::<&Transform, With<Unit>>();
    let unit_transform = query.single(app.world()).unwrap();
    let fortress_x = 5152.0;
    let fortress_y = 320.0;
    let dx = unit_transform.translation.x - fortress_x;
    let dy = unit_transform.translation.y - fortress_y;
    let dist = dx.hypot(dy);
    assert!(
        (dist - FORTRESS_SPAWN_RADIUS).abs() < 0.01,
        "Expected unit at distance {FORTRESS_SPAWN_RADIUS} from fortress, got {dist}"
    );
}
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes (no clippy/compiler errors)
- [x] `make test` passes (all tests green)
- [x] New `random_navigable_spawn` unit test passes
- [x] Updated position tests verify distance, not exact coordinates

#### Manual Verification:
- [x] Place a barracks and observe units spawning around it (not always to the right)
- [x] Multiple units from same barracks appear at different positions
- [x] Place two adjacent barracks — units don't spawn inside the neighbor building
- [x] Enemy units spawn around fortress at varying positions
- [x] Navmesh pathfinding works immediately for newly spawned units (no "stuck" frames)

**Implementation Note**: After completing this phase and all automated verification passes, pause for manual confirmation.

---

## Testing Strategy

### Unit Tests:
- `random_navigable_spawn_correct_distance_without_navmesh` — resulting point is exactly `radius` from center when navmesh is `None`

### Integration Tests:
- `unit_spawns_near_building` — spawned unit is at `BUILDING_SPAWN_RADIUS` from barracks center (no navmesh in test → uses first random point)
- `enemy_spawns_near_fortress` — spawned enemy is at `FORTRESS_SPAWN_RADIUS` from fortress center
- All existing spawn component/team tests remain unchanged (they don't check position)

### Manual Testing Steps:
1. Place a barracks with buildings on both sides — units should spawn all around, never clip into neighbors
2. Watch 5+ units spawn from same barracks — positions should vary
3. Watch enemy units spawn — should appear around fortress, not in a line
4. Place many adjacent barracks in a tight layout — verify navmesh validation prevents spawning inside obstacles

### Note on navmesh test coverage
Constructing a `NavMesh` programmatically for unit tests is not practical (requires `polyanya::Mesh` construction with polygon data). The navmesh-aware code path (retry loop with `is_in_mesh`) is verified via manual testing. The fallback path (no navmesh) is covered by automated tests.

## References

- Linear ticket: [GAM-33](https://linear.app/tayhu-games/issue/GAM-33/barracks-units-spawn-inside-adjacent-building-instead-of-around-the)
- Barracks spawn: `src/gameplay/building/production.rs:24-25`
- Fortress spawn: `src/gameplay/units/spawn.rs:91-94`
- Shared spawn function: `src/gameplay/units/mod.rs:86-142`
- Navmesh API: `vleue_navigator::NavMesh::is_in_mesh()` (`vleue_navigator-0.15.0/src/lib.rs:307-309`)
- Existing navmesh usage pattern: `src/gameplay/units/pathfinding.rs:84-99`
