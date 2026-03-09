# Decouple ORCA from Physics + Direct Transform Movement (GAM-61)

## Overview

Replace avian2d physics-driven unit movement with direct `Transform` writes. Keep ORCA algorithm but decouple it from `LinearVelocity`. Add boids separation, engager cap, same-team filtering, and overlap resolution as complementary layers.

This is the critical bridge between GAM-60 (flow field) and GAM-62 (remove avian2d entirely).

## Current State Analysis

**Movement pipeline (3 systems chained in `GameSet::Movement`):**
```
unit_movement → rebuild_spatial_hash → compute_avoidance
```

- `unit_movement` writes `PreferredVelocity` based on `TargetingState`
- `compute_avoidance` reads `PreferredVelocity`, runs ORCA, writes `LinearVelocity`
- avian2d physics integrates `LinearVelocity` to move `RigidBody::Dynamic` entities

**Key issues:**
- `compute_avoidance` writes `LinearVelocity` (avian2d component) directly — tight coupling
- `orca.rs:108-114` returns `None` for overlapping agents — bug, should generate aggressive separation
- Velocity smoothing = 0.85 dilutes ORCA corrections
- No same-team filtering — ORCA makes units avoid enemies instead of fighting them
- No boids separation for dense marching columns where ORCA is weak
- No engager cap — all units pile onto one target
- No positional overlap resolution backup (physics did this implicitly)

### Key Discoveries:
- `avoidance/mod.rs:121` — `compute_avoidance` queries `&mut LinearVelocity`
- `avoidance/mod.rs:161-163` — zero-preferred short-circuit returns `Vec2::ZERO`
- `orca.rs:108-114` — overlap case returns `None` with comment about avian2d handling it
- `units/mod.rs:134` — `RigidBody::Dynamic` on all units
- `units/mod.rs:139` — `LinearVelocity::ZERO` on all units
- `dev_tools/mod.rs:128` — debug viz queries `&LinearVelocity`
- `testing.rs:181-189` — test helper spawns `LinearVelocity::ZERO`
- `avoidance/mod.rs:229-247` — test helper `spawn_avoidance_unit` spawns `LinearVelocity`
- `ai.rs:466` — AI chain: `rebuild_target_grid → find_target → verify_targets` (no engager cap)

## Desired End State

**Movement pipeline (6 systems chained in `GameSet::Movement`):**
```
unit_movement → rebuild_spatial_hash → apply_separation → compute_avoidance → apply_movement → resolve_overlaps
```

**AI pipeline (4 systems chained in `GameSet::Ai`):**
```
rebuild_target_grid → find_target → enforce_engager_cap → verify_targets
```

**Verification:**
- Units march without avoiding enemies (same-team ORCA only)
- Units spread around targets tangentially instead of bunching
- Friendly units avoid each other (ORCA + boids)
- Attacking units stay planted (never moved by avoidance)
- Excess engagers get evicted to find other targets
- No `LinearVelocity` on units, `RigidBody::Kinematic`
- `make check` and `make test` pass

## What We're NOT Doing

- Engagement slots (failed at 5px attack range — ~7 positions in first ring)
- Letting attacking units participate in ORCA (causes oscillation)
- Cross-team separation/ORCA (causes units to avoid enemies)
- Removing avian2d entirely (that's GAM-62)
- Increasing attack range (separate concern)
- Further `resolve_overlaps` tuning (iteration count, convergence rate)

## DO NOT TRY (Anti-patterns from learnings)

1. **DO NOT let attacking units participate in ORCA** — they slide out of attack range, re-engage, oscillate
2. **DO NOT use engagement slots with 5px attack range** — ring geometry doesn't work
3. **DO NOT apply separation/ORCA between opposing teams** — units avoid enemies instead of fighting
4. **DO NOT use radial separation for shared targets** — pushes units backwards; use tangential
5. **DO NOT count current unit toward its own engager cap** — causes drop/reacquire oscillation
6. **DO NOT apply engager cap in only one branch of find_target** — must be in Seeking + retarget
7. **DO NOT use `resolve_overlaps` with `AVOIDANCE_RADIUS`** — use visual `UNIT_RADIUS`
8. **DO NOT add velocity smoothing** — ORCA corrections must apply immediately

---

## Phase 1: Fix orca.rs Overlap Handling

### Overview
Fix the bug where overlapping agents get no ORCA constraint. Replace `None` return with `MIN_TAU = 1e-3` approach matching the RVO2 reference implementation.

### Changes Required:

#### 1. `src/gameplay/units/avoidance/orca.rs`

**File**: `src/gameplay/units/avoidance/orca.rs`

Add constant at top of file (after imports):

```rust
/// Minimum time horizon for overlapping agents.
/// Matches RVO2 reference: generates maximum-urgency separation constraint.
const MIN_TAU: f32 = 1e-3;
```

**Remove `Option` return type** — with MIN_TAU + fallback directions, `compute_orca_line` always produces a valid constraint. This is cleaner long-term: callers don't need to handle `None`, and every neighbor pair produces a constraint.

**Change signature** (`orca.rs:37`): `pub fn compute_orca_line(...) -> OrcaLine`

**Update doc comment** (`orca.rs:34-36`):

```rust
/// Compute the ORCA half-plane constraint for agent `a` avoiding agent `b`.
///
/// Always produces a constraint. When agents overlap, uses `MIN_TAU` for aggressive
/// separation matching the RVO2 reference implementation.
```

**Replace the overlap branch** (`orca.rs:108-114`) — the `else` block of `if dist_sq > combined_radius_sq`:

```rust
    } else {
        // Agents are already overlapping — generate aggressive separation constraint.
        // Uses MIN_TAU (matching RVO2 reference) for maximum-urgency separation.
        let inv_tau = 1.0 / MIN_TAU;
        let w = rel_vel - inv_tau * rel_pos;
        let w_length = w.length();

        // Degenerate case: agents at exactly the same position
        let unit_w = if w_length < f32::EPSILON {
            // Fallback direction — arbitrary but deterministic
            Vec2::new(1.0, 0.0)
        } else {
            w / w_length
        };

        let direction = Vec2::new(unit_w.y, -unit_w.x);
        let u = combined_radius.mul_add(inv_tau, -w_length) * unit_w;

        OrcaLine {
            point: a.velocity + a.responsibility * u,
            direction,
        }
    }
```

**Replace the cutoff circle degenerate case** (`orca.rs:63-64`) — instead of returning `None`, use a fallback direction:

```rust
if w_length < f32::EPSILON {
    // Degenerate: use fallback direction based on relative position
    let fallback_dir = if rel_pos.length_squared() > f32::EPSILON {
        rel_pos.normalize()
    } else {
        Vec2::new(1.0, 0.0)
    };
    let direction = Vec2::new(fallback_dir.y, -fallback_dir.x);
    return OrcaLine {
        point: a.velocity,
        direction,
    };
}
```

**Update all `Some(OrcaLine { ... })` returns** to plain `OrcaLine { ... }` (lines 70-73, 88-91, 102-105).

**Update caller** in `compute_avoidance` (`avoidance/mod.rs:179-184`) — remove the `if let Some(line)` pattern:

```rust
let line = orca::compute_orca_line(agent, neighbor, config.time_horizon);
lines.push(line);
neighbor_count += 1;
```

**Update `compute_avoiding_velocity`** — no change needed, it already takes `&[OrcaLine]`.

**Update tests** that called `compute_orca_line` and checked `.expect("should produce a constraint")` — remove the `.expect()` since it returns `OrcaLine` directly now. Tests at lines 345, 368, 390, 535, 563, 566.

#### 2. Update test: `overlapping_agents_return_none` → `overlapping_agents_produce_separation_constraint`

**File**: `src/gameplay/units/avoidance/orca.rs` (test module)

Replace test at line 428:

```rust
#[test]
fn overlapping_agents_produce_separation_constraint() {
    // Agents at the same x position, overlapping.
    let a = agent(Vec2::new(0.0, 0.0), Vec2::ZERO, Vec2::new(50.0, 0.0));
    let b = agent(Vec2::new(5.0, 0.0), Vec2::ZERO, Vec2::new(-50.0, 0.0));

    // Combined radius = 12, distance = 5 → overlapping.
    // Now returns OrcaLine directly (no Option).
    let line = compute_orca_line(&a, &b, 3.0);

    // The constraint should push agent a away from b
    let result = compute_avoiding_velocity(a.preferred, a.max_speed, &[line]);
    assert!(
        result.length() > 0.1,
        "Separation constraint should produce non-zero velocity, got {result:?}"
    );
}
```

Add new test for co-located agents:

```rust
#[test]
fn co_located_agents_produce_separation_constraint() {
    // Agents at exactly the same position — uses fallback direction
    let a = agent(Vec2::new(0.0, 0.0), Vec2::ZERO, Vec2::new(50.0, 0.0));
    let b = agent(Vec2::new(0.0, 0.0), Vec2::ZERO, Vec2::new(-50.0, 0.0));

    // Should not panic — produces a valid constraint via fallback direction
    let line = compute_orca_line(&a, &b, 3.0);
    let result = compute_avoiding_velocity(a.preferred, a.max_speed, &[line]);
    assert!(
        result.length() <= a.max_speed + 0.1,
        "Result should be within max_speed"
    );
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes — all existing orca tests pass, new overlap test passes

#### Manual Verification:
- [ ] None needed — pure algorithm fix

---

## Phase 2: Add `AdjustedVelocity` + `apply_movement` + Switch to Kinematic

### Overview
Add the new velocity component, make ORCA write to it instead of `LinearVelocity`, add direct transform movement, and switch units from Dynamic to Kinematic.

### Changes Required:

#### 1. New component: `AdjustedVelocity`

**File**: `src/gameplay/units/avoidance/mod.rs`

Add after `PreferredVelocity` (line 29):

```rust
/// The final velocity after ORCA adjustment.
/// Written by `compute_avoidance`, read by `apply_movement`.
/// Replaces `LinearVelocity` from avian2d for unit movement.
#[derive(Component, Debug, Clone, Copy, Reflect, Default)]
#[reflect(Component)]
pub struct AdjustedVelocity(pub Vec2);
```

#### 2. Change `compute_avoidance` to write `AdjustedVelocity`

**File**: `src/gameplay/units/avoidance/mod.rs`

Replace `&mut LinearVelocity` with `&mut AdjustedVelocity` in the query (line 121). Update the snapshot to read from `AdjustedVelocity` instead of `LinearVelocity` (line 138: `velocity: adjusted.0`). Update the write phase (line 204-206) to write `adjusted.0 = new_velocity`.

Remove `use avian2d::prelude::*;` import (line 7) — replace with nothing (no longer needed in this file).

Full updated `compute_avoidance` signature:

```rust
pub fn compute_avoidance(
    config: Res<AvoidanceConfig>,
    hash: Res<AvoidanceSpatialHash>,
    mut agents: Query<
        (
            Entity,
            &GlobalTransform,
            &mut AdjustedVelocity,
            &PreferredVelocity,
            &AvoidanceAgent,
            &Movement,
        ),
        With<Unit>,
    >,
)
```

Snapshot reads `adjusted.0` for `velocity` field. Write phase: `adjusted.0 = new_velocity`.

#### 3. New system: `apply_movement`

**File**: `src/gameplay/units/avoidance/mod.rs`

Add after `compute_avoidance`:

```rust
/// Apply ORCA-adjusted velocity to transform directly.
/// Replaces avian2d physics integration for unit movement.
pub fn apply_movement(
    time: Res<Time>,
    mut units: Query<(&AdjustedVelocity, &mut Transform), With<Unit>>,
) {
    let dt = time.delta_secs();
    for (velocity, mut transform) in &mut units {
        transform.translation.x += velocity.0.x * dt;
        transform.translation.y += velocity.0.y * dt;
    }
}
```

#### 4. Switch units to `RigidBody::Kinematic`, replace `LinearVelocity`

**File**: `src/gameplay/units/mod.rs`

In `spawn_unit` (line 134): `RigidBody::Dynamic` → `RigidBody::Kinematic`

Remove `LinearVelocity::ZERO` (line 139), add `AdjustedVelocity::default()`.

Update imports: add `AdjustedVelocity` to the avoidance import line (line 10).

#### 5. Register `AdjustedVelocity` type

**File**: `src/gameplay/units/mod.rs`

In `plugin` function, add `app.register_type::<AdjustedVelocity>();` after the other register calls.

#### 6. Update system chain

**File**: `src/gameplay/units/mod.rs`

Change the system chain (lines 211-218) to include `apply_movement`:

```rust
app.add_systems(
    Update,
    (
        movement::unit_movement,
        avoidance::rebuild_spatial_hash,
        avoidance::compute_avoidance,
        avoidance::apply_movement,
    )
        .chain_ignore_deferred()
        .in_set(GameSet::Movement)
        .run_if(gameplay_running),
);
```

#### 7. Update dev_tools

**File**: `src/dev_tools/mod.rs`

Replace `use avian2d::prelude::LinearVelocity;` (line 8) with:
```rust
use crate::gameplay::units::avoidance::AdjustedVelocity;
```

Update `debug_draw_avoidance` query (line 128):
```rust
fn debug_draw_avoidance(
    units: Query<(&GlobalTransform, &AdjustedVelocity, &PreferredVelocity), With<Unit>>,
    mut gizmos: Gizmos,
) {
```

Update variable name in loop: `velocity` → `adjusted` (line 132, 141).

#### 8. Update test helpers

**File**: `src/testing.rs`

In `spawn_test_unit` (line 181-189): Replace `LinearVelocity::ZERO` with `AdjustedVelocity::default()`. Add import.

**File**: `src/gameplay/units/avoidance/mod.rs` (test module)

In `spawn_avoidance_unit` (line 237-246): Replace `LinearVelocity(current_vel)` with `AdjustedVelocity(current_vel)`.

Update all test assertions that read `LinearVelocity` to read `AdjustedVelocity` instead (lines 260, 286-287, 302, 324).

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes (no `LinearVelocity` references in unit code)
- [ ] `make test` passes — all avoidance integration tests work with `AdjustedVelocity`

#### Manual Verification:
- [ ] Run game — units move across battlefield following flow field
- [ ] Units engage and attack enemies
- [ ] F3 debug overlay shows green (preferred) and cyan (adjusted) velocity arrows

**Pause for manual testing before proceeding.**

---

## Phase 3: Add `resolve_overlaps` System

### Overview
Add hard positional overlap correction as a safety net after movement. Uses `AvoidanceSpatialHash` for O(n × k) neighbor lookup instead of O(n²) pairwise. Runs multiple iterations for better convergence in dense groups. Asymmetric: moving units get pushed, attacking units stay planted. Uses visual `UNIT_RADIUS`, not inflated avoidance radius.

### Changes Required:

#### 1. New constant

**File**: `src/gameplay/units/avoidance/mod.rs`

```rust
/// Number of iterations for resolve_overlaps. Multiple passes improve convergence
/// in dense groups where a single pass can't fully separate all overlapping pairs.
const OVERLAP_ITERATIONS: u32 = 3;
```

#### 2. New system: `resolve_overlaps`

**File**: `src/gameplay/units/avoidance/mod.rs`

Add after `apply_movement`:

```rust
/// Hard positional overlap correction. Runs after `apply_movement` as a safety net.
///
/// Uses `AvoidanceSpatialHash` for O(n × k) neighbor lookup per iteration.
/// Runs `OVERLAP_ITERATIONS` passes for better convergence in dense groups.
///
/// Overlap rules:
/// - Moving unit vs moving: split equally
/// - Moving unit vs stationary (attacking): only the moving unit gets pushed
/// - Both stationary: split equally to unstick
///
/// Uses visual UNIT_RADIUS (not inflated AVOIDANCE_RADIUS) so units can stand side-by-side.
/// Applies to ALL teams (cross-team overlap resolution).
pub fn resolve_overlaps(
    hash: Res<AvoidanceSpatialHash>,
    mut units: Query<(Entity, &mut Transform, &AdjustedVelocity), With<Unit>>,
) {
    let min_dist = UNIT_RADIUS * 2.0;
    let min_dist_sq = min_dist * min_dist;

    for _ in 0..OVERLAP_ITERATIONS {
        // Snapshot positions for consistent reads within this iteration
        let snapshots: Vec<(Entity, Vec2, bool)> = units
            .iter()
            .map(|(e, t, v)| {
                let is_stationary = v.0.length_squared() < f32::EPSILON;
                (e, t.translation.xy(), is_stationary)
            })
            .collect();

        let index_map: HashMap<Entity, usize> = snapshots
            .iter()
            .enumerate()
            .map(|(i, (e, ..))| (*e, i))
            .collect();

        // Collect corrections (entity → displacement) to apply after iteration
        let mut corrections: HashMap<Entity, Vec2> = HashMap::new();

        for &(entity_a, pos_a, stationary_a) in &snapshots {
            // Use spatial hash for neighbor lookup instead of all-pairs
            let neighbors = hash.query_neighbors(pos_a, min_dist);

            for neighbor_entity in neighbors {
                // Only process each pair once (a < b by entity index)
                if neighbor_entity.index() <= entity_a.index() {
                    continue;
                }

                let Some(&idx_b) = index_map.get(&neighbor_entity) else {
                    continue;
                };
                let (entity_b, pos_b, stationary_b) = snapshots[idx_b];

                let diff = pos_b - pos_a;
                let dist_sq = diff.length_squared();

                if dist_sq >= min_dist_sq || dist_sq < f32::EPSILON * f32::EPSILON {
                    continue;
                }

                let dist = dist_sq.sqrt();
                let overlap = min_dist - dist;
                let direction = diff / dist;

                match (stationary_a, stationary_b) {
                    (false, true) => {
                        // Only push the moving unit (a) away
                        *corrections.entry(entity_a).or_default() -= direction * overlap;
                    }
                    (true, false) => {
                        // Only push the moving unit (b) away
                        *corrections.entry(entity_b).or_default() += direction * overlap;
                    }
                    _ => {
                        // Both moving or both stationary: split equally
                        let half_overlap = overlap * 0.5;
                        *corrections.entry(entity_a).or_default() -= direction * half_overlap;
                        *corrections.entry(entity_b).or_default() += direction * half_overlap;
                    }
                }
            }
        }

        // Apply accumulated corrections
        for (entity, correction) in corrections {
            if let Ok((_, mut transform, _)) = units.get_mut(entity) {
                transform.translation.x += correction.x;
                transform.translation.y += correction.y;
            }
        }
    }
}
```

**Key design decisions:**
- Uses `AvoidanceSpatialHash` (already rebuilt every frame) for O(n × k) neighbor lookup
- Entity index comparison (`neighbor.index() <= entity_a.index()`) ensures each pair is processed exactly once
- Corrections are accumulated per-entity per iteration, then applied — prevents order-dependent results within an iteration
- `OVERLAP_ITERATIONS = 3` — each pass resolves more of the remaining overlap, converging toward separation
- The spatial hash cell size (150px+) is much larger than `min_dist` (12px), so all overlapping neighbors are guaranteed to be found

#### 3. Wire into system chain

**File**: `src/gameplay/units/mod.rs`

Add `resolve_overlaps` to the chain after `apply_movement`:

```rust
(
    movement::unit_movement,
    avoidance::rebuild_spatial_hash,
    avoidance::compute_avoidance,
    avoidance::apply_movement,
    avoidance::resolve_overlaps,
)
    .chain_ignore_deferred()
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes

#### Manual Verification:
- [ ] Units don't visually overlap when bunching around targets
- [ ] Attacking units stay in place (don't get pushed by resolve_overlaps)
- [ ] Dense groups resolve cleanly (no jittering from insufficient iterations)

**Pause for manual testing before proceeding.**

---

## Phase 4: Tune ORCA Constants

### Overview
Adjust ORCA parameters based on learnings. Inflated avoidance radius, longer time horizon, no velocity smoothing, static time horizon for stationary neighbors.

### Changes Required:

#### 1. New constants and config field

**File**: `src/gameplay/units/avoidance/mod.rs`

Add/update constants:

```rust
/// Inflated ORCA avoidance radius (2× visual radius).
/// Gives ORCA more margin for planning without affecting visual overlap detection.
pub const AVOIDANCE_RADIUS: f32 = UNIT_RADIUS * 2.0;

const DEFAULT_TIME_HORIZON: f32 = 5.0;           // was 3.0
const DEFAULT_VELOCITY_SMOOTHING: f32 = 1.0;     // was 0.85 — no smoothing
```

Add to `AvoidanceConfig`:

```rust
pub struct AvoidanceConfig {
    pub time_horizon: f32,
    pub max_neighbors: u32,
    pub neighbor_distance: f32,
    pub velocity_smoothing: f32,
    /// Shorter time horizon for stationary (attacking) neighbors.
    pub static_time_horizon: f32,
}
```

Default: `static_time_horizon: 0.5`.

Update `neighbor_distance` default to use new `DEFAULT_TIME_HORIZON`:
```rust
neighbor_distance: DEFAULT_TIME_HORIZON * 50.0 + AVOIDANCE_RADIUS * 2.0,
```

#### 2. Update `AvoidanceAgent` default radius

**File**: `src/gameplay/units/avoidance/mod.rs`

```rust
impl Default for AvoidanceAgent {
    fn default() -> Self {
        Self {
            radius: AVOIDANCE_RADIUS,  // was UNIT_RADIUS
            responsibility: 0.5,
        }
    }
}
```

#### 3. Use `static_time_horizon` in `compute_avoidance`

In the ORCA computation loop, when building constraints:

```rust
// Use shorter time horizon for stationary neighbors
let time_horizon = if neighbor.preferred.length_squared() < f32::EPSILON {
    config.static_time_horizon
} else {
    config.time_horizon
};

if let Some(line) = orca::compute_orca_line(agent, neighbor, time_horizon) {
```

#### 4. Update spatial hash cell size

**File**: `src/gameplay/units/mod.rs`

The spatial hash cell size should match the new `neighbor_distance`:

```rust
let config = AvoidanceConfig::default();
app.insert_resource(AvoidanceSpatialHash(SpatialHash::new(
    config.neighbor_distance,
)));
```

(This already works since it reads from `AvoidanceConfig::default()` — just verify the new default is used.)

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes — update any tests that assert specific velocity values if tolerance is broken

#### Manual Verification:
- [ ] Units avoid each other more proactively (larger avoidance radius visible in behavior)
- [ ] ORCA corrections feel immediate (no smoothing lag)

**Pause for manual testing before proceeding.**

---

## Phase 5: Dynamic ORCA Responsibility

### Overview
Engaging/Attacking units dodge less (responsibility=0.25), Moving/Seeking units yield more (responsibility=0.75). This makes marching units move out of the way of units heading to their target.

### Changes Required:

#### 1. New constants

**File**: `src/gameplay/units/avoidance/mod.rs`

```rust
/// ORCA responsibility for engaging/attacking units (dodge less).
const ENGAGING_RESPONSIBILITY: f32 = 0.25;
/// ORCA responsibility for moving/seeking units (yield more).
const MOVING_RESPONSIBILITY: f32 = 0.75;
```

#### 2. Add `&TargetingState` to `compute_avoidance` query

**File**: `src/gameplay/units/avoidance/mod.rs`

Update query to include `&TargetingState`:

```rust
mut agents: Query<
    (
        Entity,
        &GlobalTransform,
        &mut AdjustedVelocity,
        &PreferredVelocity,
        &AvoidanceAgent,
        &Movement,
        &TargetingState,
    ),
    With<Unit>,
>,
```

#### 3. Override responsibility in snapshot

In the snapshot phase, override `AvoidanceAgent.responsibility` with dynamic value:

```rust
let responsibility = match *targeting_state {
    TargetingState::Engaging(_) | TargetingState::Attacking(_) => ENGAGING_RESPONSIBILITY,
    TargetingState::Moving | TargetingState::Seeking => MOVING_RESPONSIBILITY,
};
// ...
AgentSnapshot {
    // ...
    responsibility,
}
```

#### 4. Update test helpers

**File**: `src/gameplay/units/avoidance/mod.rs` (test module)

`spawn_avoidance_unit` must now include `TargetingState`:

```rust
fn spawn_avoidance_unit(
    world: &mut World,
    x: f32,
    y: f32,
    preferred: Vec2,
    current_vel: Vec2,
) -> Entity {
    world
        .spawn((
            Unit,
            Movement { speed: 50.0 },
            Transform::from_xyz(x, y, 0.0),
            GlobalTransform::from(Transform::from_xyz(x, y, 0.0)),
            PreferredVelocity(preferred),
            AvoidanceAgent::default(),
            AdjustedVelocity(current_vel),
            TargetingState::Moving,
        ))
        .id()
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes

#### Manual Verification:
- [ ] Marching units yield to engaging units (visible in F3 debug — cyan arrows deflect more for moving units)

---

## Phase 6: Same-Team ORCA Filtering

### Overview
ORCA only generates constraints for same-team neighbors. Opposing-team units become invisible to avoidance — you *want* to collide with enemies in a combat game.

### Changes Required:

#### 1. Add `&Team` to `compute_avoidance` query

**File**: `src/gameplay/units/avoidance/mod.rs`

Update query:

```rust
mut agents: Query<
    (
        Entity,
        &GlobalTransform,
        &mut AdjustedVelocity,
        &PreferredVelocity,
        &AvoidanceAgent,
        &Movement,
        &TargetingState,
        &Team,
    ),
    With<Unit>,
>,
```

#### 2. Include `Team` in snapshot

Add `team: Team` field to each snapshot tuple. In the ORCA computation loop, skip opposing-team neighbors:

```rust
// Same-team ORCA only — don't avoid enemies, you want to fight them
if neighbor_team != agent_team {
    continue;
}
```

#### 3. Update test helpers

`spawn_avoidance_unit` must now include `Team`:

```rust
fn spawn_avoidance_unit(
    world: &mut World,
    x: f32,
    y: f32,
    preferred: Vec2,
    current_vel: Vec2,
) -> Entity {
    world
        .spawn((
            Unit,
            Movement { speed: 50.0 },
            Transform::from_xyz(x, y, 0.0),
            GlobalTransform::from(Transform::from_xyz(x, y, 0.0)),
            PreferredVelocity(preferred),
            AvoidanceAgent::default(),
            AdjustedVelocity(current_vel),
            TargetingState::Moving,
            Team::Player,
        ))
        .id()
}
```

**IMPORTANT**: All existing avoidance tests use same-team units (all `Team::Player`), so ORCA constraints will still be generated between them. Tests should pass without modification beyond the spawn helper.

#### 4. Add test for cross-team non-avoidance

```rust
#[test]
fn opposing_team_units_no_avoidance() {
    let mut app = create_avoidance_test_app();
    let a = {
        let world = app.world_mut();
        world
            .spawn((
                Unit,
                Movement { speed: 50.0 },
                Transform::from_xyz(100.0, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(100.0, 100.0, 0.0)),
                PreferredVelocity(Vec2::new(50.0, 0.0)),
                AvoidanceAgent::default(),
                AdjustedVelocity(Vec2::new(50.0, 0.0)),
                TargetingState::Moving,
                Team::Player,
            ))
            .id()
    };
    let _b = {
        let world = app.world_mut();
        world
            .spawn((
                Unit,
                Movement { speed: 50.0 },
                Transform::from_xyz(130.0, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(130.0, 100.0, 0.0)),
                PreferredVelocity(Vec2::new(-50.0, 0.0)),
                AvoidanceAgent::default(),
                AdjustedVelocity(Vec2::new(-50.0, 0.0)),
                TargetingState::Moving,
                Team::Enemy,
            ))
            .id()
    };
    app.update();
    let vel = app.world().get::<AdjustedVelocity>(a).unwrap();
    // Should NOT steer laterally — enemy is invisible to ORCA
    assert!(
        vel.0.y.abs() < 1.0,
        "Opposing team units should not trigger avoidance, got {:?}",
        vel.0
    );
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes — including new cross-team test

#### Manual Verification:
- [ ] Units march directly toward enemies without swerving around them
- [ ] Units still avoid same-team neighbors normally

**Pause for manual testing before proceeding.**

---

## Phase 7: Engager Cap

### Overview
Limit how many units can simultaneously engage a single unit target. Prevents pileups. `MAX_ENGAGERS_PER_UNIT_TARGET = 12`.

### Changes Required:

#### 1. New constant and system

**File**: `src/gameplay/ai.rs`

Add constant:

```rust
/// Maximum number of units that can simultaneously engage/attack a single unit target.
/// Only applies to unit targets (not buildings/fortresses).
const MAX_ENGAGERS_PER_UNIT_TARGET: u32 = 12;
```

Add import for `Unit` marker:
```rust
use crate::gameplay::units::Unit;
```

#### 2. New system: `enforce_engager_cap`

**File**: `src/gameplay/ai.rs`

```rust
/// Evict excess engagers from unit targets. Keeps closest N, kicks farthest to Moving/Seeking.
/// Attacking units get priority (never evicted). Only applies to unit targets.
fn enforce_engager_cap(
    mut units: Query<(Entity, &GlobalTransform, &mut TargetingState, Option<&Movement>)>,
    unit_targets: Query<&GlobalTransform, With<Unit>>,
    mut commands: Commands,
) {
    // Group engagers by target
    let mut engagers_by_target: HashMap<Entity, Vec<(Entity, f32, bool, bool)>> = HashMap::new();

    for (entity, transform, state, movement) in &units {
        let target_entity = match *state {
            TargetingState::Engaging(t) | TargetingState::Attacking(t) => t,
            _ => continue,
        };
        // Only cap unit targets
        if unit_targets.get(target_entity).is_err() {
            continue;
        }

        let is_attacking = matches!(*state, TargetingState::Attacking(_));
        let target_pos = unit_targets.get(target_entity).unwrap().translation().xy();
        let distance = transform.translation().xy().distance(target_pos);
        let is_mobile = movement.is_some();

        engagers_by_target
            .entry(target_entity)
            .or_default()
            .push((entity, distance, is_attacking, is_mobile));
    }

    // Enforce cap per target
    for (_target, mut engagers) in engagers_by_target {
        if engagers.len() as u32 <= MAX_ENGAGERS_PER_UNIT_TARGET {
            continue;
        }

        // Sort: attacking first (never evicted), then by distance (closest first)
        engagers.sort_by(|a, b| {
            b.2.cmp(&a.2) // attacking = true sorts first
                .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        });

        // Evict excess (from the end — farthest non-attacking)
        for &(entity, _, _, is_mobile) in engagers.iter().skip(MAX_ENGAGERS_PER_UNIT_TARGET as usize) {
            if let Ok((_, _, mut state, _)) = units.get_mut(entity) {
                *state = if is_mobile {
                    TargetingState::Moving
                } else {
                    TargetingState::Seeking
                };
                commands.entity(entity).remove::<EngagementLeash>();
            }
        }
    }
}
```

Add `use std::collections::HashMap;` at top if not already imported.

#### 3. Cap check in `find_target`

**File**: `src/gameplay/ai.rs`

Before the main loop in `find_target`, build an engager count map:

```rust
// Count current engagers per unit target (for cap check)
let mut engager_counts: HashMap<Entity, u32> = HashMap::new();
for (_, _, _, _, _, state, _) in &seekers {
    if let Some(target) = state.target_entity() {
        *engager_counts.entry(target).or_default() += 1;
    }
}
```

In `search_radius`, add a parameter for the engager counts and current_target:

```rust
fn search_radius(
    grid: &TargetSpatialHash,
    radius: f32,
    seeker_entity: Entity,
    seeker_pos: Vec2,
    seeker_extent: &EntityExtent,
    opposing_team: Team,
    is_mobile: bool,
    seeker_team: Team,
    all_targets: &Query<(Entity, &Team, &GlobalTransform, &EntityExtent), With<Target>>,
    unit_markers: &Query<(), With<Unit>>,
    engager_counts: &HashMap<Entity, u32>,
    current_target: Option<Entity>,
) -> Option<Entity> {
```

In the candidate filter loop, add cap check:

```rust
// Skip unit targets at engager cap (exclude current target from count)
if unit_markers.get(cand_entity).is_ok() {
    let count = engager_counts.get(&cand_entity).copied().unwrap_or(0);
    let effective_count = if current_target == Some(cand_entity) {
        count.saturating_sub(1) // Don't count self
    } else {
        count
    };
    if effective_count >= MAX_ENGAGERS_PER_UNIT_TARGET {
        continue;
    }
}
```

Pass `current_target`:
- Seeking branch: `current_target = None`
- Engaging/Attacking retarget branch: `current_target = state.target_entity()`

Add `unit_markers: Query<(), With<Unit>>` parameter to `find_target`.

Update `find_nearest_target` and `search_radius` to thread through these new parameters.

#### 4. Wire into AI system chain

**File**: `src/gameplay/ai.rs`

```rust
(rebuild_target_grid, find_target, enforce_engager_cap, verify_targets)
    .chain_ignore_deferred()
    .in_set(GameSet::Ai)
    .run_if(gameplay_running),
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes
- [ ] Add test: spawn 20 player units Seeking 1 enemy → after a few frames, at most 12 are Engaging/Attacking

#### Manual Verification:
- [ ] Units spread across available targets (not all piling onto one)
- [ ] No "dancing" (units dropping and reacquiring targets)

**Pause for manual testing before proceeding.**

---

## Phase 8: Boids Separation with Tangential Push

### Overview
Add reactive boids-style repulsion between same-team units. Tangential push for shared targets (prevents backwards oscillation), radial push for different targets.

### Changes Required:

#### 1. New constants

**File**: `src/gameplay/units/avoidance/mod.rs`

```rust
/// Boids separation neighbor detection radius (4× unit radius).
const SEPARATION_RADIUS: f32 = UNIT_RADIUS * 4.0;
/// Boids separation force strength.
const SEPARATION_STRENGTH: f32 = 30.0;
```

#### 2. New system: `apply_separation`

**File**: `src/gameplay/units/avoidance/mod.rs`

```rust
/// Boids-style reactive repulsion between same-team units.
///
/// - Same-team only — opposing units are invisible
/// - Tangential push for units sharing a target (perpendicular to target→unit line)
/// - Radial push for units with different targets
/// - Skips stationary (attacking) units
/// - Blended into PreferredVelocity, clamped to max speed
pub fn apply_separation(
    hash: Res<AvoidanceSpatialHash>,
    mut units: Query<
        (
            Entity,
            &GlobalTransform,
            &mut PreferredVelocity,
            &Movement,
            &TargetingState,
            &Team,
        ),
        With<Unit>,
    >,
    targets: Query<&GlobalTransform>,
) {
    // Snapshot: can't read neighbor data while iterating query mutably
    let snapshots: Vec<(Entity, Vec2, Vec2, f32, Option<Entity>, Team)> = units
        .iter()
        .map(|(e, gt, pv, mv, ts, team)| {
            (
                e,
                gt.translation().xy(),
                pv.0,
                mv.speed,
                ts.target_entity(),
                *team,
            )
        })
        .collect();

    let index_map: HashMap<Entity, usize> = snapshots
        .iter()
        .enumerate()
        .map(|(i, (e, ..))| (*e, i))
        .collect();

    let results: Vec<(Entity, Vec2)> = snapshots
        .iter()
        .map(|(entity, pos, preferred, max_speed, target, team)| {
            // Skip stationary units
            if preferred.length_squared() < f32::EPSILON {
                return (*entity, *preferred);
            }

            let mut separation = Vec2::ZERO;
            let neighbors = hash.query_neighbors(*pos, SEPARATION_RADIUS);

            for neighbor_entity in neighbors {
                if neighbor_entity == *entity {
                    continue;
                }
                let Some(&idx) = index_map.get(&neighbor_entity) else {
                    continue;
                };
                let (_, n_pos, _, _, n_target, n_team) = &snapshots[idx];

                // Same-team only
                if n_team != team {
                    continue;
                }

                let diff = *pos - *n_pos;
                let dist_sq = diff.length_squared();
                let sep_radius_sq = SEPARATION_RADIUS * SEPARATION_RADIUS;
                if dist_sq >= sep_radius_sq || dist_sq < f32::EPSILON {
                    continue;
                }

                let dist = dist_sq.sqrt();
                let strength = SEPARATION_STRENGTH * (1.0 - dist / SEPARATION_RADIUS);

                // Tangential push for shared targets
                let push_dir = if *target == *n_target && target.is_some() {
                    let target_entity = target.unwrap();
                    if let Ok(target_gt) = targets.get(target_entity) {
                        let to_unit = *pos - target_gt.translation().xy();
                        if to_unit.length_squared() > f32::EPSILON {
                            // Perpendicular to target→unit line
                            let tangent = Vec2::new(-to_unit.y, to_unit.x).normalize();
                            // Pick tangent that points away from neighbor
                            let radial = diff / dist;
                            if tangent.dot(radial) >= 0.0 {
                                tangent
                            } else {
                                -tangent
                            }
                        } else {
                            diff / dist // fallback radial
                        }
                    } else {
                        diff / dist // target despawned, fallback radial
                    }
                } else {
                    diff / dist // different targets or no target — radial push
                };

                separation += push_dir * strength;
            }

            let mut new_preferred = *preferred + separation;
            // Clamp to max speed
            if new_preferred.length_squared() > max_speed * max_speed {
                new_preferred = new_preferred.normalize() * max_speed;
            }
            (*entity, new_preferred)
        })
        .collect();

    // Write results
    for (entity, new_preferred) in results {
        if let Ok((_, _, mut preferred, _, _, _)) = units.get_mut(entity) {
            preferred.0 = new_preferred;
        }
    }
}
```

#### 3. Wire into system chain

**File**: `src/gameplay/units/mod.rs`

Final 6-system chain:

```rust
(
    movement::unit_movement,
    avoidance::rebuild_spatial_hash,
    avoidance::apply_separation,
    avoidance::compute_avoidance,
    avoidance::apply_movement,
    avoidance::resolve_overlaps,
)
    .chain_ignore_deferred()
    .in_set(GameSet::Movement)
    .run_if(gameplay_running),
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes

#### Manual Verification:
- [ ] Dense marching columns spread out noticeably (boids + ORCA working together)
- [ ] Units engaging the same target spread around it tangentially (not bunch on one side)
- [ ] No backwards oscillation when units share a target

**Pause for manual testing before proceeding.**

---

## Phase 9: Dev Tools Enhancement + Final Cleanup

### Overview
Add target lines to debug visualization (yellow=Engaging, red=Attacking). Clean up any dead code warnings. Final test pass.

### Changes Required:

#### 1. Enhanced debug viz

**File**: `src/dev_tools/mod.rs`

Update `debug_draw_avoidance` to also draw target lines:

```rust
fn debug_draw_avoidance(
    units: Query<(&GlobalTransform, &AdjustedVelocity, &PreferredVelocity, &TargetingState), With<Unit>>,
    targets: Query<&GlobalTransform, With<Target>>,
    mut gizmos: Gizmos,
) {
    let scale = 0.5;
    for (transform, adjusted, preferred, targeting_state) in &units {
        let pos = transform.translation().xy();

        // Target lines
        match targeting_state {
            TargetingState::Engaging(target) => {
                if let Ok(target_gt) = targets.get(*target) {
                    let target_pos = target_gt.translation().xy();
                    gizmos.line_2d(pos, target_pos, Color::srgb(1.0, 1.0, 0.0)); // Yellow
                }
            }
            TargetingState::Attacking(target) => {
                if let Ok(target_gt) = targets.get(*target) {
                    let target_pos = target_gt.translation().xy();
                    gizmos.line_2d(pos, target_pos, Color::srgb(1.0, 0.0, 0.0)); // Red
                }
            }
            _ => {}
        }

        // Green arrow: preferred velocity
        if preferred.0.length_squared() > f32::EPSILON {
            gizmos.arrow_2d(pos, pos + preferred.0 * scale, Color::srgb(0.0, 1.0, 0.0));
        }

        // Cyan arrow: ORCA-adjusted velocity
        if adjusted.0.length_squared() > f32::EPSILON {
            gizmos.arrow_2d(pos, pos + adjusted.0 * scale, Color::srgb(0.0, 1.0, 1.0));
        }
    }
}
```

Add imports for `TargetingState`, `Target`.

#### 2. Final test additions

Add integration tests for:
- `resolve_overlaps` pushes overlapping units apart
- `apply_separation` modifies preferred velocity for close same-team units
- Engager cap evicts excess units

#### 3. Clippy cleanup

Run `make check` and fix any:
- `dead_code` warnings (especially if engagement.rs was partially created)
- `clippy::too_many_arguments` (may need `#[allow]` on `search_radius`)
- `clippy::cast_precision_loss` on any new index math

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes with zero warnings
- [ ] `make test` passes — all existing + new tests

#### Manual Verification:
- [ ] F3 shows yellow lines (Engaging) and red lines (Attacking) to targets
- [ ] Green arrows show preferred velocity with separation influence
- [ ] Cyan arrows show final ORCA-adjusted velocity
- [ ] Full battle plays correctly: units march, engage, attack, spread, die, retarget

---

## Testing Strategy

### Unit Tests (orca.rs):
- Existing 9 tests updated (overlap test renamed)
- New: co-located agents produce constraint
- Existing LP tests unchanged

### Integration Tests (avoidance/mod.rs):
- All 4 existing tests updated for `AdjustedVelocity` + `Team` + `TargetingState`
- New: opposing team units don't trigger avoidance
- New: resolve_overlaps pushes apart overlapping units
- New: apply_separation modifies preferred velocity

### Integration Tests (ai.rs):
- All existing tests unchanged (engager cap only affects new `enforce_engager_cap` system)
- New: engager cap evicts excess units
- New: self-counting doesn't cause oscillation

### Manual Testing:
1. Run game with ~50 units per team
2. F3 debug: verify target lines, velocity arrows
3. Observe: units march without avoiding enemies
4. Observe: units spread around targets (tangential separation)
5. Observe: max 12 units engage each unit target
6. Observe: attacking units stay planted

## Performance Characteristics

| System | Complexity | Notes |
|--------|-----------|-------|
| `rebuild_spatial_hash` | O(n) | Unchanged |
| `apply_separation` | O(n × k) | k = avg neighbors in SEPARATION_RADIUS |
| `compute_avoidance` | O(n × m) | m = max_neighbors (capped at 10) |
| `apply_movement` | O(n) | Trivial |
| `resolve_overlaps` | O(n × k × iter) | Spatial hash lookup, 3 iterations |
| `enforce_engager_cap` | O(n log n) | Sort per target group |

## References

- Linear ticket: [GAM-61](https://linear.app/tayhu-games/issue/GAM-61/decouple-orca-from-physics-direct-transform-movement)
- Learnings doc: `thoughts/shared/research/2026-03-08-orca-decoupling-learnings.md`
- Research doc: `thoughts/shared/research/2026-03-05-targeting-movement-combat-scalability.md`
- Depends on: [GAM-60](https://linear.app/tayhu-games/issue/GAM-60/flow-field-infrastructure-remove-navmesh) (Done)
- Blocks: [GAM-62](https://linear.app/tayhu-games/issue/GAM-62/remove-avian2d-physics-engine) (remove avian2d)
