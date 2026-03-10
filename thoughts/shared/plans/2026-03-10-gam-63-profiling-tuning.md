# Profiling & Tuning Pass — 40k Unit Target (GAM-63) Implementation Plan

## Overview

Profile the game at 4k, 10k, and 40k units. Add profiling infrastructure (Tracy integration, FPS counter, stress-test keybind). Switch unit rendering from `Mesh2d` to `Sprite::from_color()` for sprite batching. Tune constants based on profiling data. Validate <16ms frame budget at 40k. Smoke test at 100k.

## Current State Analysis

**Rendering**: Units use `Mesh2d` + `MeshMaterial2d` with shared handles (`units/mod.rs:121-122`). Each unit gets a `Circle` mesh handle and a team-colored `ColorMaterial`. At 40k units this means 40k draw calls through the mesh2d pipeline, which lacks sprite batching.

**Spatial hashes**: `HashMap<(i32,i32), Vec<Entity>>` (`spatial_hash.rs:12-15`). Two instances:
- `AvoidanceSpatialHash` — cell size 24px, rebuilt every frame for all `With<Unit>` entities
- `TargetSpatialHash` — cell size 64px, rebuilt every frame for all `With<Target>` entities

**Separation system**: Boids-style (`avoidance/mod.rs:83-194`). Snapshots all unit positions into `Vec<SeparationSnapshot>`, builds `HashMap<Entity, usize>` index, iterates all units with non-zero velocity querying spatial hash neighbors. `resolve_overlaps` runs 3 iterations with similar snapshot+index+query pattern.

**AI targeting**: Staggered retargeting across 10 slots, 0.015s per slot (`ai.rs:37-41`). `rebuild_target_grid` is O(n) per frame. `find_target` queries with initial 576px radius, fallback to 5300px.

**Profiling infrastructure**: None. No `FrameTimeDiagnosticsPlugin`, no Tracy, no profiling cargo profile, no stress-test tools.

**Flow field**: 16px cells, 328×40 grid = 13,120 cells. Dijkstra recompute only on building placement/removal. O(1) direction lookup per unit.

### Key Discoveries:
- `Sprite::from_color(color, size)` confirmed in Bevy 0.18 (`bevy_sprite/src/sprite.rs:69`)
- Tracy built into Bevy via `trace_tracy` feature flag — no external crate needed
- `FrameTimeDiagnosticsPlugin` at `bevy::diagnostic` with `FPS`, `FRAME_TIME`, `FRAME_COUNT` diagnostics
- `LogDiagnosticsPlugin` logs diagnostics to console every 1s by default
- `apply_separation` allocates two `Vec` (snapshots + results) and one `HashMap` (index) per frame — potential allocation hotspot at scale
- `resolve_overlaps` allocates `Vec` + two `HashMap`s per iteration × 3 iterations = 9 allocations per frame
- `query_neighbors` allocates a new `Vec<Entity>` per call — at 40k units with ~8 neighbors each, that's 40k small Vec allocations per system per frame

## Desired End State

- Tracy profiling available via `cargo run --features tracy`
- On-screen FPS/frame-time counter in dev builds
- Stress-test keybind (F5) to mass-spawn units at target counts
- Unit rendering uses `Sprite::from_color()` instead of `Mesh2d`
- Frame budget <16ms at 40k units validated (or hotspots documented with mitigation plan)
- Constants tuned based on profiling data
- Profiling results documented

### Verification:
- `cargo build --features tracy` succeeds
- `cargo test` passes — all unit/integration tests work with Sprite rendering
- `cargo clippy` clean
- Manual: FPS counter visible, stress-test spawns units, Tracy captures frame data
- Manual: <16ms frame time at 40k units (or documented why not and what to fix)

## What We're NOT Doing

- GPU instancing or compute-shader rendering — only if Sprite batching isn't enough (conditional)
- Changing game logic or combat behavior — tuning only adjusts numeric constants
- Spatial hash flat-grid upgrade upfront — only if profiling shows it's a hotspot
- Reverse-lookup index for orphan scan — only if profiling shows mass-death spike
- Changing flow field algorithm — already O(1) per unit per frame

## Implementation Approach

Four phases, each independently testable:

1. **Profiling Infrastructure** — add Tracy feature, FPS diagnostics, stress-test tool. This gives us measurement capability before making changes.
2. **Rendering Optimization** — switch Mesh2d → Sprite. Known win at scale (sprite batching vs individual mesh draws).
3. **Profile & Tune** — iterative manual work. Run at scale, identify hotspots, tune constants.
4. **Conditional Optimizations** — implement only what profiling identifies as necessary.

---

## Phase 1: Profiling Infrastructure

### Overview
Add Tracy integration, on-screen FPS counter, console diagnostics, and a stress-test keybind for mass unit spawning. This phase adds no behavioral changes — only measurement tools.

### Changes Required:

#### 1. Add Tracy feature flag
**File**: `Cargo.toml`
**Changes**: Add a `tracy` feature that enables Bevy's built-in Tracy support.

```toml
[features]
default = ["dev"]
dev = ["bevy/dynamic_linking", "dep:bevy-inspector-egui"]
tracy = ["bevy/trace_tracy"]
```

#### 2. Add profiling cargo profile
**File**: `Cargo.toml`
**Changes**: Add a profile for profiling (release optimizations + debug symbols for Tracy).

```toml
# Profiling profile - release speed with debug symbols for Tracy
[profile.profiling]
inherits = "release"
debug = 1
```

#### 3. Add FPS diagnostics to dev_tools
**File**: `src/dev_tools/mod.rs`
**Changes**: Add `FrameTimeDiagnosticsPlugin` and `LogDiagnosticsPlugin` (logs FPS to console every second). Add on-screen FPS text overlay.

In the `plugin` function, add:
```rust
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};

// In plugin():
app.add_plugins(FrameTimeDiagnosticsPlugin::default());
app.add_plugins(LogDiagnosticsPlugin::default());
app.add_systems(Startup, setup_fps_counter);
app.add_systems(Update, update_fps_counter);
```

Add FPS counter UI:
```rust
/// Marker for the FPS text entity.
#[derive(Component)]
struct FpsText;

fn setup_fps_counter(mut commands: Commands) {
    commands.spawn((
        FpsText,
        Text::new("FPS: --"),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(5.0),
            right: Val::Px(5.0),
            ..default()
        },
    ));
}

fn update_fps_counter(
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
    mut text: Query<&mut Text, With<FpsText>>,
) {
    if let Ok(mut text) = text.single_mut() {
        if let Some(fps) = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|d| d.smoothed())
        {
            **text = format!("FPS: {fps:.0}");
        }
    }
}
```

Note: The `Text` + `Node` spawning pattern and `Single` vs `Query` usage needs verification against Bevy 0.18 UI API during implementation. The FPS counter should render as a top-right overlay regardless of game state.

#### 4. Add stress-test keybind
**File**: `src/dev_tools/mod.rs`
**Changes**: Add F5 keybind that cycles through spawn counts. Each press spawns a batch of units (split 50/50 between teams).

```rust
/// Stress test spawn counts. F5 cycles through these.
const STRESS_TEST_COUNTS: &[u32] = &[1000, 4000, 10_000, 40_000];

/// Current stress test level index.
#[derive(Resource, Default)]
struct StressTestLevel(usize);

fn stress_test_spawn(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    mut level: Local<usize>,
    assets: Option<Res<UnitAssets>>,
    registry: Option<Res<GoalRegistry>>,
) {
    if !input.just_pressed(KeyCode::F5) {
        return;
    }
    let (Some(assets), Some(registry)) = (assets, registry) else {
        return;
    };

    let count = STRESS_TEST_COUNTS[*level % STRESS_TEST_COUNTS.len()];
    *level += 1;

    // Spawn units split between teams, distributed around their fortresses
    let half = count / 2;
    for _ in 0..half {
        let pos = random_navigable_spawn(registry.player_fortress.center, 200.0);
        spawn_unit(&mut commands, UnitType::Soldier, Team::Player, pos, &assets);
    }
    for _ in 0..half {
        let pos = random_navigable_spawn(registry.enemy_fortress.center, 200.0);
        spawn_unit(&mut commands, UnitType::Soldier, Team::Enemy, pos, &assets);
    }

    info!("Stress test: spawned {count} units (total will include existing)");
}
```

Register in `plugin()`:
```rust
app.add_systems(
    Update,
    stress_test_spawn.run_if(crate::gameplay_running),
);
```

This requires importing `UnitAssets`, `spawn_unit`, `random_navigable_spawn`, `UnitType`, `Team` from gameplay modules.

#### 5. Add unit count display
**File**: `src/dev_tools/mod.rs`
**Changes**: Extend the FPS counter to also show total entity/unit count.

```rust
fn update_fps_counter(
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
    mut text: Query<&mut Text, With<FpsText>>,
    units: Query<(), With<Unit>>,
) {
    if let Ok(mut text) = text.single_mut() {
        let unit_count = units.iter().count();
        if let Some(fps) = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|d| d.smoothed())
        {
            **text = format!("FPS: {fps:.0} | Units: {unit_count}");
        }
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` succeeds
- [ ] `cargo build --features tracy` succeeds (no compile errors)
- [ ] `cargo test` passes
- [ ] `cargo clippy` clean

#### Manual Verification:
- [ ] FPS counter visible in top-right corner during gameplay
- [ ] Console logs FPS every second
- [ ] F5 spawns 1000 units first press, 4000 second press, etc.
- [ ] Unit count updates on FPS overlay after stress-test spawn
- [ ] Tracy GUI captures frame data when running with `--features tracy`

**Implementation Note**: After completing Phase 1, pause for manual verification that profiling tools work correctly before proceeding.

---

## Phase 2: Rendering Optimization — Mesh2d to Sprite

### Overview
Switch unit rendering from `Mesh2d` + `MeshMaterial2d` (per-entity mesh draw) to `Sprite::from_color()` (batched sprite rendering). Bevy's sprite renderer batches entities with the same texture into fewer draw calls, which is critical at 40k units.

### Changes Required:

#### 1. Simplify `UnitAssets` — remove mesh and materials
**File**: `src/gameplay/units/mod.rs`
**Changes**: `UnitAssets` no longer needs mesh or material handles. Units use `Sprite::from_color()` which embeds color directly. Remove the resource entirely — colors come from `palette` constants.

Delete `UnitAssets` struct, `setup_unit_assets` system, and the `OnEnter` system registration.

Remove from `plugin()`:
```rust
// DELETE: app.add_systems(OnEnter(GameState::InGame), setup_unit_assets);
```

Remove imports that are no longer needed: `Mesh2d`, `MeshMaterial2d`, `ColorMaterial`, `Circle`.

#### 2. Update `spawn_unit` to use `Sprite::from_color()`
**File**: `src/gameplay/units/mod.rs`
**Changes**: Replace `Mesh2d` + `MeshMaterial2d` with `Sprite::from_color()`. The function signature changes — no longer needs `assets: &UnitAssets`.

```rust
pub fn spawn_unit(
    commands: &mut Commands,
    unit_type: UnitType,
    team: Team,
    position: Vec2,
) -> Entity {
    let stats = unit_stats(unit_type);
    let color = match team {
        Team::Player => palette::PLAYER_UNIT,
        Team::Enemy => palette::ENEMY_UNIT,
    };

    commands
        .spawn((
            Name::new(format!("{team:?} {}", unit_type.display_name())),
            Unit,
            unit_type,
            team,
            Target,
            Health::new(stats.hp),
            HealthBarConfig {
                width: UNIT_HEALTH_BAR_WIDTH,
                height: UNIT_HEALTH_BAR_HEIGHT,
                y_offset: UNIT_HEALTH_BAR_Y_OFFSET,
            },
            CombatStats {
                damage: stats.damage,
                attack_speed: stats.attack_speed,
                range: stats.attack_range,
            },
            Movement {
                speed: stats.move_speed,
            },
            AttackTimer(Timer::from_seconds(
                1.0 / stats.attack_speed,
                TimerMode::Repeating,
            )),
            Sprite::from_color(color, Vec2::splat(UNIT_RADIUS * 2.0)),
            Transform::from_xyz(position.x, position.y, Z_UNIT),
            DespawnOnExit(GameState::InGame),
        ))
        .insert((
            TargetingState::Moving,
            match team {
                Team::Player => AssignedGoal::EnemyFortress,
                Team::Enemy => AssignedGoal::PlayerFortress,
            },
            EntityExtent::Circle(UNIT_RADIUS),
            PreferredVelocity::default(),
        ))
        .id()
}
```

Note: Units render as colored squares (`UNIT_RADIUS * 2.0` = 12px × 12px) instead of circles. This is acceptable for a prototype — circle sprites require a texture atlas, which is out of scope.

#### 3. Update all `spawn_unit` call sites
**Files**: All callers of `spawn_unit` must drop the `&assets` argument.

- `src/gameplay/units/spawn.rs` — `tick_enemy_spawner` and `tick_player_spawner`
- `src/dev_tools/mod.rs` — `stress_test_spawn` (from Phase 1)
- `src/testing.rs` — `spawn_test_unit` helper

Each call changes from `spawn_unit(&mut commands, type, team, pos, &assets)` to `spawn_unit(&mut commands, type, team, pos)`.

Systems that previously required `Res<UnitAssets>` no longer need it. Remove the parameter from:
- `tick_enemy_spawner` and `tick_player_spawner` system signatures
- `stress_test_spawn` system signature

#### 4. Update test helpers
**File**: `src/testing.rs`
**Changes**: `spawn_test_unit` no longer needs `UnitAssets`. Remove the asset setup from test app creation if it was only there for unit rendering. `init_asset_resources` may still be needed for other things — check during implementation.

**File**: `src/gameplay/units/mod.rs` (integration tests)
**Changes**: `unit_assets_created_on_enter_ingame` test should be deleted (resource no longer exists). Replace with a test that verifies units spawn with `Sprite` component.

#### 5. Update entity archetype documentation
**File**: `src/gameplay/mod.rs`
**Changes**: Update the doc comment listing unit archetype components — replace `Mesh2d` + `MeshMaterial2d` with `Sprite`.

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes
- [ ] `cargo clippy` clean
- [ ] `grep -r "Mesh2d" src/` returns nothing (except possibly doc comments)
- [ ] `grep -r "UnitAssets" src/` returns nothing
- [ ] `grep -r "ColorMaterial" src/` returns nothing (unless used by non-unit code)

#### Manual Verification:
- [ ] Units render as colored squares on screen
- [ ] Player units are the correct color, enemy units are the correct color
- [ ] Visual size is reasonable (~12px squares)
- [ ] No rendering artifacts or z-fighting
- [ ] Compare FPS before/after at 4k units — expect improvement

**Implementation Note**: After completing Phase 2, pause for manual verification that rendering looks correct and measure the FPS improvement before proceeding.

---

## Phase 3: Profile & Tune at Scale

### Overview
Iterative profiling phase. Run the game at 4k, 10k, and 40k units using Tracy and the stress-test keybind. Identify hotspots. Tune constants. This phase is manual and data-driven — the specific changes depend on profiling results.

### Profiling Procedure:

1. **Baseline at 4k**: Launch with `cargo run --features tracy --profile profiling`. Press F5 to spawn 4k units. Capture Tracy trace. Record frame time.

2. **Scale to 10k**: Additional F5 press. Capture Tracy trace. Compare system-level timings vs 4k.

3. **Scale to 40k**: Additional F5 presses. Capture Tracy trace. Identify systems exceeding budget.

4. **Smoke test at 100k**: Press F5 multiple times. Note what breaks (if anything).

### Expected Hotspots (ranked by likely impact):

| System | Why Hot | Estimated Cost at 40k |
|--------|---------|----------------------|
| `apply_separation` | O(n) snapshot + O(n×k) neighbor queries + allocations | High |
| `resolve_overlaps` | 3× (snapshot + HashMap + neighbor queries) | High |
| `rebuild_spatial_hash` (avoidance) | O(n) insert, 40k entities | Medium |
| `rebuild_target_grid` | O(n) insert, but fewer Target entities | Low |
| `propagate_transforms` | Bevy built-in, O(n) for flat hierarchy | Medium |
| Sprite rendering | Batched, but 40k entities still significant | Medium |

### Constants to Tune:

| Constant | Current | Location | Tuning Range | Notes |
|----------|---------|----------|-------------|-------|
| `FLOW_CELL_SIZE` | 16px | `flow_field.rs:22` | 16-64px | Larger = fewer cells, coarser movement |
| `SEPARATION_RADIUS` | 24px | `avoidance/mod.rs:20` | 16-32px | Smaller = fewer neighbors per query |
| `SEPARATION_STRENGTH` | 30.0 | `avoidance/mod.rs:22` | 20-50 | Adjust for visual quality |
| `OVERLAP_ITERATIONS` | 3 | `avoidance/mod.rs:18` | 1-3 | Fewer = faster but more overlap |
| `ATTACK_HYSTERESIS` | 8px | `ai.rs:45` | 4-16px | Wider = less state thrashing |
| `LEASH_DISTANCE` | 192px | `mod.rs:107` | 128-256px | Shorter = faster disengagement |
| `RETARGET_SLOTS` | 10 | `ai.rs:37` | 10-20 | More slots = smoother but slower retarget |
| `detection_radius` formula | `range * 2.0` | `ai.rs:56` | `range * 1.5` to `range * 3.0` | Wider = more candidates per query |

### Success Criteria:

#### Automated Verification:
- [ ] `cargo test` passes after any constant changes
- [ ] `cargo clippy` clean

#### Manual Verification:
- [ ] Frame time <16ms at 40k units (or documented why not)
- [ ] No visual regressions from constant changes
- [ ] Units still pathfind correctly after flow field changes
- [ ] Separation looks natural (no excessive clumping or jittering)
- [ ] Combat still works (attacks land, units die, targeting correct)

**Implementation Note**: This phase is iterative. Each tuning change should be tested manually before proceeding to the next. Document profiling results in this plan or a separate research document.

---

## Phase 4: Conditional Optimizations

### Overview
Implement optimizations only if profiling in Phase 3 identifies them as necessary. Each subsection is independent and should only be done if the corresponding hotspot is confirmed.

### 4A: Flat-Grid Spatial Hash (if HashMap is hot)

**Trigger**: Tracy shows `HashMap` operations in `rebuild_spatial_hash`, `apply_separation`, or `resolve_overlaps` consuming >2ms at 40k.

**Change**: Replace `HashMap<(i32,i32), Vec<Entity>>` with `Vec<Vec<Entity>>` flat grid. The battlefield is bounded (5248×640px), so we can pre-allocate a fixed grid.

For avoidance hash (24px cells): `(5248/24) × (640/24)` = 219 × 27 = 5,913 cells
For targeting hash (64px cells): `(5248/64) × (640/64)` = 82 × 10 = 820 cells

Both fit in L1 cache headers. Direct array indexing eliminates hash computation and collision resolution.

**Files**: `src/gameplay/spatial_hash.rs` — replace `HashMap` internals with `Vec<Vec<Entity>>` + `width`/`height` fields. Keep the same public API (`new`, `clear`, `insert`, `query_neighbors`). Constructor takes `cell_size`, `world_width`, `world_height`.

### 4B: Reverse-Lookup Index for Orphan Scan (if mass death is hot)

**Trigger**: Tracy shows orphan scan in `verify_targets` or observer spikes >2ms during mass death events (50+ deaths/frame).

**Change**: Add `HashMap<Entity, SmallVec<[Entity; 8]>>` mapping target → current attackers/engagers. When a target dies, iterate only its attackers instead of scanning all units. Maintained by `find_target` (on engagement) and `verify_targets` (on disengage).

**Files**: `src/gameplay/ai.rs` — new `AttackerRegistry` resource.

### 4C: Reduce `apply_separation` Allocations (if allocations are hot)

**Trigger**: Tracy shows allocation overhead in `apply_separation` or `resolve_overlaps` >1ms.

**Changes**:
- Replace per-frame `Vec<SeparationSnapshot>` with a reusable `Local<Vec<SeparationSnapshot>>` that's cleared and reused
- Replace per-frame `HashMap<Entity, usize>` index with a reusable `Local<HashMap<Entity, usize>>`
- Replace `query_neighbors` `Vec<Entity>` return with a callback API: `for_each_neighbor(pos, radius, |entity| { ... })`

**Files**: `src/gameplay/spatial_hash.rs` (callback API), `src/gameplay/units/avoidance/mod.rs` (Local buffers)

### 4D: Investigate `propagate_transforms` (if hot)

**Trigger**: Tracy shows `propagate_transforms` >2ms at 40k.

**Notes**: With flat hierarchy (no parent-child for units), this should be cheap. If it's hot, the issue is likely health bar children — each unit has child entities for the health bar background and fill. Options:
- Remove health bar entities from non-damaged units (spawn on first damage)
- Use a single health bar rendering pass with manual positioning (no child entities)

This is a larger refactor and may be deferred to a future ticket.

### Success Criteria:

#### Automated Verification:
- [ ] `cargo test` passes after any optimization
- [ ] `cargo clippy` clean

#### Manual Verification:
- [ ] Frame time improvement confirmed in Tracy
- [ ] No behavioral regressions
- [ ] Spatial hash queries still return correct neighbors

---

## Testing Strategy

### Unit Tests:
- Phase 1: No new unit tests needed (diagnostics are Bevy built-in)
- Phase 2: Update existing unit spawn tests to verify `Sprite` component instead of `Mesh2d`
- Phase 3: Run existing test suite after constant changes
- Phase 4: If flat grid implemented, port all `spatial_hash.rs` tests to new implementation

### Integration Tests:
- Existing integration tests continue to work (rendering components are not queried in tests)
- `unit_assets_created_on_enter_ingame` test deleted and replaced with sprite-based verification

### Manual Testing Steps:
1. Run game, verify units render correctly as colored squares
2. Press F5, verify mass spawn works at each count level
3. Check FPS counter shows reasonable values
4. At 40k units, verify game is still playable (<16ms frame time)
5. Verify combat still works (units attack, die, projectiles hit)

## Verified API Patterns (Bevy 0.18)

These were verified against the actual crate source:

- `Sprite::from_color(color: impl Into<Color>, size: Vec2)` — sets `color` and `custom_size: Some(size)` (`bevy_sprite/src/sprite.rs:69`)
- Tracy via `bevy = { features = ["trace_tracy"] }` — built-in, no external crate (`bevy-0.18.0/Cargo.toml:2564`)
- `FrameTimeDiagnosticsPlugin` at `bevy::diagnostic::FrameTimeDiagnosticsPlugin` — `::default()` creates with 120-frame history
- `FrameTimeDiagnosticsPlugin::FPS` — `DiagnosticPath` constant for FPS metric
- `LogDiagnosticsPlugin` at `bevy::diagnostic::LogDiagnosticsPlugin` — logs to console every 1s
- `DiagnosticsStore` resource (not `Diagnostics`) — query with `.get(&path).and_then(|d| d.smoothed())`
- `[profile.profiling]` with `inherits = "release"` + `debug = 1` — standard Cargo profile for profiling

## Performance Considerations

- **Sprite batching** is the biggest expected win. Bevy batches sprites with the same texture/material into single draw calls. `Sprite::from_color()` uses a default white texture with color tinting, so all units of the same color batch together → 2 draw calls instead of 40k.
- **HashMap spatial hash** may show up in profiling due to hash computation + pointer chasing. Flat grid is cache-friendly and O(1) lookup.
- **Allocation overhead** from per-frame Vec/HashMap creation in separation/overlap systems may be significant. `Local<T>` reuse eliminates this.
- **Flow field** is already O(1) per unit — unlikely to be a hotspot.
- **`propagate_transforms`** with flat hierarchy should be O(n) with good cache behavior — unlikely to be a hotspot unless health bar children add overhead.

## References

- Linear ticket: [GAM-63](https://linear.app/tayhu-games/issue/GAM-63/profiling-and-tuning-pass-40k-unit-target)
- Research: `thoughts/shared/research/2026-03-05-targeting-movement-combat-scalability.md` (Section 9, Ticket 6)
- Dependency: [GAM-62](https://linear.app/tayhu-games/issue/GAM-62/remove-avian2d-physics-engine) — must be completed first
- GAM-62 plan: `thoughts/shared/plans/2026-03-09-gam-62-remove-avian2d.md`
- GAM-61 (boids separation): already merged on main
