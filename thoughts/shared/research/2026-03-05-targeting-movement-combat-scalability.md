# Research: Targeting, Movement & Combat Architecture for 100k Units

**Date**: 2026-03-05
**Status**: Reviewed (post-team review)
**Purpose**: Design a scalable targeting/movement/combat architecture that handles 40k-100k units, replacing the current per-unit navmesh pathfinding approach with a layered flow field + spatial grid system
**Reviewed by**: 4-agent architecture review team (Movement Architect, Combat Designer, Performance Engineer, Systems Integrator)

---

## 1. Problem Statement

The current architecture uses **per-unit everything**: each unit individually queries a spatial hash for targets, gets its own navmesh path via `vleue_navigator`, follows its own waypoints, and checks attack range via GJK surface-distance every frame. This creates compounding O(n) costs that break down at scale.

### Current System Costs (per frame)

| System | Location | Cost | Problem at 100k |
|--------|----------|------|-----------------|
| `rebuild_target_grid` | `ai.rs` | O(n) insert all targets | Acceptable |
| `find_target` | `ai.rs` | O(n) spatial queries, staggered across 10 slots | Mass retarget spike when focused target dies |
| `compute_paths` | `pathfinding.rs` | O(n) navmesh A* calls, 0.5s burst | ALL units recompute same frame |
| `unit_movement` | `movement.rs` | O(n) GJK surface_distance | Expensive range check per unit per frame |
| `attack` | `attack.rs` | O(n) GJK surface_distance | Same expensive check, duplicated |
| `check_death` | `death.rs` | O(n) health poll | No event-driven notification |
| ORCA avoidance | `avoidance/` | O(n) with neighbor queries | Full velocity obstacle solver per unit |
| avian2d physics | `third_party/avian.rs` | O(n log n) broad-phase | Physics engine running on 100k rigid bodies |

### Known Bugs That Stem From Architecture

- **GAM-56**: Mass retarget spike when a focused enemy dies (all orphaned units bypass throttle)
- **GAM-55**: Units stop at navmesh edge instead of closing final gap to NavObstacle targets
- **GAM-47**: Dense crowds push enemies out of combat range via physics
- **GAM-43**: All navmesh paths refresh simultaneously every 0.5s

---

## 2. Research: Scalable Unit AI Algorithms

Research into algorithms used by large-scale RTS games (Supreme Commander, Planetary Annihilation, Factorio) and crowd simulators. Source: Gemini conversation + game architecture literature.

### 2.1 Spatial Partitioning (Targeting)

**Uniform Spatial Grid (already implemented via GAM-42)**

Divide the map into cells. Each entity registers in its cell. To find nearby enemies, only check current cell + neighbors. O(1) per query.

Our implementation: `SpatialHash` in `gameplay/spatial_hash.rs` — `HashMap<(i32, i32), Vec<Entity>>` with `insert`, `query_neighbors(pos, radius)`, `clear`.

**Verdict**: Keep and optimize. Replace `HashMap` with flat `Vec<Vec<Entity>>` for fixed-size battlefield (see Section 4.6).

### 2.2 Flow Fields (Macro Movement)

Instead of per-unit pathfinding, precompute a **direction grid** from a goal outward using Dijkstra. Each cell stores a vector pointing toward the neighboring cell with the lowest cost. Units just read their cell — O(1) per unit.

```
Goal: Enemy Fortress (right side)

Flow Field (8-connected with cost field):
→ → → ↗ ↗ ↗
→ → → → → ↗    ← buildings carved out as high-cost/blocked cells
→ → ↗ ■ ■ ↗       units flow around them automatically
→ → → → → →
```

**Recomputation**: Only when map topology changes (building placed or destroyed). For a 82×10 grid = 820 cells, Dijkstra takes microseconds.

**Multiple goals**: Each goal (fortress, key building, strategic point) gets its own flow field. Units read the field matching their assigned goal. Supporting player-directed attacks later is just "compute a new field, reassign units."

**Verdict**: Adopt. Replaces navmesh pathfinding entirely. Removes `vleue_navigator` dependency, `NavPath`, `PathRefreshTimer`, `compute_paths`, and the navmesh snap hack for off-mesh targets.

### 2.3 Leader-Follower / Platooning

Only "leader" units run spatial queries. Followers just track the friendly unit in front of them. Reduces targeting cost from 100k queries to ~5k.

**Dangers**: Circular references (A follows B follows C follows A), clumping (100 units converge on one point), death spirals (leader dies, cascade of retargeting).

**Verdict**: Skip. Flow fields + spatial grid targeting achieves the same goal more simply. Leader-follower adds complexity (cycle detection, chain management) for marginal benefit when flow fields already make movement O(1).

### 2.4 Influence Maps

Low-resolution grid where units "stamp" their presence. Values decay and blur outward. Units move toward high-enemy-influence zones instead of targeting specific entities.

**Verdict**: Defer. Useful for strategic AI decision-making (where to attack, when to retreat) but not needed for the auto-battler core loop where goals are explicit. Could layer on top of flow fields later for enemy AI behavior.

### 2.5 Flocking / Boids

Three rules based on nearby same-team units: Separation (push apart), Alignment (match heading), Cohesion (move toward group center). Plus Attraction (toward enemy).

**Verdict**: Adopt separation only, with team-weighted forces and a lateral nudge for opposing armies. Full boids is unnecessary — flow fields handle alignment and cohesion implicitly (all units in a region move the same direction). See Section 4.5 for details.

### 2.6 Bounding Volume Hierarchies (BVH)

Alternative to spatial grids. Wraps groups in bounding boxes, creates a tree. Better than grids when units are highly clustered in small areas of a huge map.

**Verdict**: Skip. Our map is a fixed-width corridor — uniform grid is ideal. BVH shines for sparse open-world maps.

### 2.7 Hierarchical Pathfinding (HPA*)

Divides map into chunks, pathfinds chunk-to-chunk first, then locally within chunks.

**Verdict**: Skip. Flow fields are superior for our use case (all units share the same precomputed field, no per-unit pathfinding at any level).

---

## 3. Research: Do We Still Need Physics?

Current physics usage via avian2d:

| Physics Feature | Current Use | Replacement |
|-----------------|-------------|-------------|
| Pushbox colliders (unit-unit) | Prevent overlap | Separation force |
| Pushbox colliders (unit-building) | Units can't walk through buildings | Flow field routes around buildings |
| `RigidBody::Dynamic` | Units moved by collisions | Direct `Transform` updates |
| `RigidBody::Kinematic` toggle | Proposed GAM-47 fix | Not needed — no physics pushing |
| Sensor colliders (projectile) | Hit detection | Distance check: `dist < threshold` |
| `surface_distance` (GJK) | Attack range check | `EntityExtent::surface_distance_from()` — circle or AABB distance |
| `CollisionLayers` | Hitbox/Hurtbox separation | Not needed without sensors |
| `CollidingEntities` | Projectile hit detection | Not needed with distance check |
| Broad-phase | Spatial acceleration | Already have spatial hash |

### AOE Attacks

AOE attacks need "who is within radius X of this point?" — the spatial grid handles this:

```rust
target_grid.for_each_neighbor(explosion_pos, aoe_radius, |entity| {
    if is_enemy(entity) && distance(explosion_pos, entity_pos) < aoe_radius {
        apply_damage(entity, aoe_damage);
    }
});
```

One spatial grid query + distance filter. O(1) lookup + check handful of nearby entities. No physics needed.

Works for all AOE patterns:
- **Circle** (fireball): query radius, distance check
- **Cone** (dragon breath): query radius, angle check
- **Line** (arrow volley): query along segments, perpendicular distance check
- **Persistent** (poison cloud): same query each tick
- **Chained** (lightning bounce): iterative spatial queries from each hit target

### Verdict: Remove avian2d

Every physics feature has a simpler, cheaper replacement. At 100k units, avoiding the physics broad-phase/narrow-phase is a massive performance win. All position updates become direct `Transform` mutations.

---

## 4. Proposed Architecture: 4-Layer System

### 4.1 Layer 1: Strategic — Goal Assignment

```rust
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct AssignedGoal(pub GoalId);

#[derive(Resource)]
pub struct GoalRegistry {
    pub goals: Vec<GoalInfo>,  // 2-4 goals max, linear scan faster than HashMap
}

pub struct GoalInfo {
    pub position: Vec2,
    pub team: Team,
    pub flow_field: FlowField,
}
```

- Each team has at least one goal (enemy fortress)
- Units default to `AssignedGoal::EnemyFortress`
- Future: player designates attack points, units reassigned
- Goal destroyed → reassign orphaned units to next goal
- `Vec<GoalInfo>` with `GoalId` as index — at 2-4 goals, linear search is faster than hashing

### 4.2 Layer 2: Macro Movement — Flow Fields

```rust
pub struct FlowField {
    pub cell_size: f32,           // 64px (matches existing battlefield grid)
    pub width: u32,               // 82 (TOTAL_COLS)
    pub height: u32,              // 10 (BATTLEFIELD_ROWS)
    pub directions: Vec<Vec2>,    // direction vector per cell (flattened 2D, cache-friendly)
    pub costs: Vec<f32>,          // movement cost per cell (3-tier cost field)
}
```

**Cost field** (not binary blocked/unblocked):

| Cell Type | Cost | Purpose |
|-----------|------|---------|
| Open | 1.0 | Normal movement |
| Adjacent to building (8 neighbors) | 3.0 | Discourage corner-hugging, prevents clipping |
| Building cell | `f32::INFINITY` | Impassable |

The 1-cell buffer ring at cost 3.0 provides 64px clearance for 12px-diameter units, preventing corner clipping without over-constraining routing in the 6-column-wide build zone.

**8-connected grid with corner-cutting prevention**:

Diagonal moves are allowed (smooth movement) but only if BOTH cardinal neighbors are passable:
```rust
// Only allow diagonal (x+1, y+1) if BOTH (x+1, y) AND (x, y+1) are passable
if dx != 0 && dy != 0 {
    if costs[cardinal_x] >= f32::INFINITY || costs[cardinal_y] >= f32::INFINITY {
        continue; // block diagonal through building corners
    }
}
// Diagonal cost = 1.414 * neighbor_cost (distance-correct)
```

**Computation** (Dijkstra from goal):
1. Set goal cell cost = 0, all others = MAX
2. Dijkstra outward on 8-connected grid with weighted costs
3. Each cell's direction = normalized vector pointing to lowest-cost neighbor
4. Goal cell and immediate neighbors: direction points at goal world-space center (not `Vec2::ZERO`) so units keep drifting toward the fortress instead of stopping
5. Store as flat `Vec<Vec2>` for cache-friendly access (~23KB for 820 cells, fits in L2 cache)

**Recomputation triggers**:
- Building placed or destroyed (sets `FlowFieldDirty` flag)
- New goal created
- NOT per-frame — flow fields are stable until topology changes

**Unit ejection on building placement**: When a building is placed, query spatial hash for units in the building's cells. Teleport each to the nearest unblocked cell center, pushing radially outward from building center. Walk up to 5 steps at CELL_SIZE intervals. Rare event, teleport is acceptable.

**Unit movement**:
```rust
fn flow_field_movement(
    units: Query<(&mut Transform, &AssignedGoal, &Movement, &TargetingState)>,
    goals: Res<GoalRegistry>,
    time: Res<Time>,
) {
    for (mut transform, goal, movement, state) in &mut units {
        match state {
            TargetingState::Moving | TargetingState::Seeking => {
                // Follow flow field
                let field = &goals.goals[goal.0].flow_field;
                let direction = field.direction_at(transform.translation.xy());
                transform.translation += (direction * movement.speed * time.delta_secs()).extend(0.0);
            }
            TargetingState::Engaging(target) => {
                // Steer directly toward target (closing distance for attack)
                // ... direct steering code
            }
            TargetingState::Attacking(_) => {
                // Stationary — velocity = 0, attack system handles firing
            }
        }
    }
}
```

**Map boundary enforcement**: `clamp_to_battlefield` system at end of `GameSet::Movement`:
```rust
fn clamp_to_battlefield(mut units: Query<&mut Transform, With<Unit>>) {
    for mut transform in &mut units {
        let pos = &mut transform.translation;
        pos.x = pos.x.clamp(UNIT_RADIUS, BATTLEFIELD_WIDTH - UNIT_RADIUS);
        pos.y = pos.y.clamp(UNIT_RADIUS, BATTLEFIELD_HEIGHT - UNIT_RADIUS);
    }
}
```

### 4.3 Layer 3: Micro Targeting — Spatial Grid + State Machine

```rust
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub enum TargetingState {
    /// Following flow field toward assigned goal. No spatial queries, no target.
    Moving,
    /// Detected enemies nearby. Running spatial queries to pick best target.
    /// Also the idle/default state for static entities (fortresses, turrets).
    Seeking,
    /// Locked onto a specific enemy. Movement system steers DIRECTLY toward target.
    Engaging(Entity),
    /// In attack range. Velocity = 0. Attack system fires on timer.
    /// If pushed out of range, transitions back to Engaging (not Moving).
    Attacking(Entity),
}
```

**State transitions**:

```
MOBILE UNITS:
Moving ──(enemy detected in detection radius)──→ Seeking
Seeking ──(found target)──→ Engaging(entity)
Seeking ──(no enemies nearby after N frames)──→ Moving
Engaging ──(target in attack range)──→ Attacking(entity)
Engaging ──(target dies)──→ Seeking  [via On<Remove, Target> observer]
Engaging ──(leash distance exceeded)──→ Moving
Attacking ──(pushed out of range + hysteresis)──→ Engaging(same entity)
Attacking ──(target dies)──→ Seeking  [via On<Remove, Target> observer]

STATIC ENTITIES (fortresses, turrets):
Seeking ──(found target in range)──→ Attacking(entity)
Attacking ──(target dies)──→ Seeking
Attacking ──(target out of range)──→ Seeking
(never enter Moving or Engaging — they don't move)
```

**Detection radius** (per unit type, derived from `CombatStats.range`):
```rust
fn detection_radius(range: f32) -> f32 {
    (range * 2.0).max(MIN_DETECTION_RADIUS)  // MIN_DETECTION_RADIUS = 64.0 (1 cell)
}
```

| Unit Type | Attack Range | Detection Radius | Cells |
|-----------|-------------|-----------------|-------|
| Soldier (melee) | 5.0 | 64.0 (min floor) | 1 |
| Future archer | 100.0 | 200.0 | ~3 |
| Future siege | 200.0 | 400.0 | ~6 |
| Fortress (static) | 200.0 | 400.0 | ~6 |

**Engagement leash** (prevents chasing targets too far backward):

```rust
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct EngagementLeash {
    pub origin: Vec2,        // Position when entering Engaging
    pub max_distance: f32,   // LEASH_DISTANCE = 192.0 (3 cells)
}
```

Set when transitioning `Seeking → Engaging`. If `distance(current_pos, leash_origin) > LEASH_DISTANCE`, transition back to `Moving`. This replaces the current directional backtrack limit with a spatial one.

**Attack range hysteresis** (prevents Attacking ↔ Engaging oscillation):
- Enter `Attacking`: when `surface_dist <= attack_range`
- Exit `Attacking`: when `surface_dist > attack_range + ATTACK_HYSTERESIS` (8px)

Both `LEASH_DISTANCE` (192px) and `ATTACK_HYSTERESIS` (8px) are global constants, not per-unit. The leash is a map-topology property (corridor depth), and hysteresis absorbs separation force jitter which is the same magnitude for all unit types.

**Backtrack limit preservation**: The current directional backtrack limit (`ai.rs:228-236`) is preserved in two places:
1. `find_target`: candidate filter — don't select targets behind the unit
2. `verify_targets`: disengage check — if target moves behind backtrack threshold, `Engaging → Moving`

**Target priority** (future): Add `TargetPriority` component (`Nearest`, `LowestHp`, `Buildings`, `Units`) as a tiebreaker in `find_target`. Not needed for prototype with one unit type, but the spatial query infrastructure supports it.

**Death handling — On<Remove, Target> observer** (no Dead marker):

When `check_death` despawns an entity, the `On<Remove, Target>` observer fires BEFORE component data is removed (verified in Bevy 0.18 source: `bevy_ecs-0.18.0/src/world/entity_access/world_mut.rs:1557-1617`). The dead entity's `Transform` and `Team` are fully queryable. This matches the existing `On<Remove, Building>` pattern in the codebase.

```rust
fn on_target_removed(
    trigger: On<Remove, Target>,
    dying: Query<(&Transform, &Team)>,
    mut seekers: Query<&mut TargetingState>,
) {
    let dead_entity = trigger.entity;
    // All orphaned units transition to Seeking — no inheritance
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

No target inheritance — orphaned units go to `Seeking` and find new targets via the stagger system. This avoids focus-fire clumping (200 units inheriting the same replacement → instant overkill → chain reaction). Units are already in the combat zone, so they find new targets within 0-150ms and naturally pick different nearby enemies based on their individual positions.

**Performance note**: The orphan scan is O(n) per death event. At 40k units with normal death rates (5-20/frame) this is fine. For AOE mass death (50+ deaths/frame), a reverse-lookup index (`HashMap<Entity, SmallVec<[Entity; 8]>>` mapping target → attackers) reduces to O(k) per death. Deferred to Ticket 6 profiling.

### 4.4 Layer 4: Combat — Attack System

**Range checks use `EntityExtent`** (not GJK):

```rust
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub enum EntityExtent {
    Circle(f32),           // radius — units
    Rect(f32, f32),        // half_width, half_height — fortresses, buildings
}

impl EntityExtent {
    /// Cheap surface distance from a point to this entity's boundary.
    pub fn surface_distance_from(&self, self_pos: Vec2, point: Vec2) -> f32 {
        match self {
            Self::Circle(r) => self_pos.distance(point) - r,
            Self::Rect(hw, hh) => {
                let d = (point - self_pos).abs();
                let dx = (d.x - hw).max(0.0);
                let dy = (d.y - hh).max(0.0);
                (dx * dx + dy * dy).sqrt()
            }
        }
    }
}
```

Circle distance is one subtraction. Rect distance is 4 ops + sqrt. Both replace GJK's ~10-20 iterations per pair.

**Attack system** reads `Attacking` state only:

```rust
fn attack_system(
    mut attackers: Query<(&Transform, &CombatStats, &mut AttackTimer, &TargetingState, &EntityExtent)>,
    targets: Query<(&Transform, &EntityExtent)>,
    time: Res<Time>,
) {
    for (attacker_tf, stats, mut timer, state, attacker_extent) in &mut attackers {
        let TargetingState::Attacking(target) = state else { continue };
        let Ok((target_tf, target_extent)) = targets.get(*target) else { continue };

        let surface_dist = target_extent.surface_distance_from(
            target_tf.translation.xy(), attacker_tf.translation.xy()
        ) - attacker_extent.radius();

        if surface_dist > stats.range + ATTACK_HYSTERESIS {
            // Pushed out of range — transition handled by verify_targets
            continue;
        }

        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            // spawn projectile
        }
    }
}
```

**Projectile hit detection** (no physics):

Projectiles are homing (100% hit rate). Acceptable for auto-battler prototype — DPS is cleanly `damage / attack_cooldown`, making balance transparent. Miss chance is trivial to add later (`if rng.gen() > accuracy`) with zero architectural commitment now.

```rust
fn move_projectiles(
    mut commands: Commands,
    mut projectiles: Query<(Entity, &mut Transform, &Projectile)>,
    targets: Query<&Transform, Without<Projectile>>,
    mut healths: Query<&mut Health>,
    time: Res<Time>,
) {
    for (entity, mut proj_tf, projectile) in &mut projectiles {
        let Ok(target_tf) = targets.get(projectile.target) else {
            commands.entity(entity).despawn(); // target gone
            continue;
        };
        let direction = (target_tf.translation - proj_tf.translation).normalize_or_zero();
        let move_amount = projectile.speed * time.delta_secs();
        let remaining = proj_tf.translation.distance(target_tf.translation);

        if move_amount >= remaining {
            // Hit
            if let Ok(mut health) = healths.get_mut(projectile.target) {
                health.current = (health.current - projectile.damage).max(0.0);
            }
            commands.entity(entity).despawn();
        } else {
            proj_tf.translation += direction * move_amount;
        }
    }
}
```

### 4.5 Separation Force (replaces ORCA + physics)

**Team-weighted separation with lateral nudge** — handles both same-team streaming and opposing army head-on collisions.

Two-system split avoids snapshot allocation:

```rust
/// Intermediate component — computed push vector before application.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct SeparationForce(pub Vec2);

/// System 1: Read Transform (immutable), write SeparationForce.
fn compute_separation(
    units: Query<(Entity, &Transform, &Team, &TargetingState), With<Unit>>,
    mut forces: Query<&mut SeparationForce>,
    unit_grid: Res<UnitSpatialHash>,
) {
    for (entity, transform, team, state) in &units {
        // Skip Moving units — they're spread out on the flow field
        if matches!(state, TargetingState::Moving) {
            if let Ok(mut f) = forces.get_mut(entity) { f.0 = Vec2::ZERO; }
            continue;
        }

        let pos = transform.translation.xy();
        let mut push = Vec2::ZERO;

        unit_grid.for_each_neighbor(pos, SEPARATION_RADIUS, |neighbor| {
            if let Ok((_, neighbor_tf, neighbor_team, _)) = units.get(neighbor) {
                let diff = pos - neighbor_tf.translation.xy();
                let dist = diff.length();
                if dist < f32::EPSILON || dist >= SEPARATION_RADIUS { return; }

                if *neighbor_team != *team {
                    // Cross-team: 3x stronger push + lateral slide
                    let lateral = diff.normalize().perp();
                    push += (diff.normalize() / dist) * CROSS_TEAM_STRENGTH
                          + lateral * CROSS_TEAM_SLIDE;
                } else {
                    // Same-team: gentle push
                    push += (diff.normalize() / dist) * SAME_TEAM_STRENGTH;
                }
            }
        });

        if let Ok(mut force) = forces.get_mut(entity) {
            force.0 = push.normalize_or_zero() * SEPARATION_MAX;
        }
    }
}

/// System 2: Read SeparationForce (immutable), write Transform.
fn apply_separation(
    mut units: Query<(&mut Transform, &SeparationForce), With<Unit>>,
    time: Res<Time>,
) {
    for (mut transform, force) in &mut units {
        transform.translation += (force.0 * time.delta_secs()).extend(0.0);
    }
}
```

**Constants** (starting values, tuned in Ticket 6):
- `SAME_TEAM_STRENGTH`: 30.0
- `CROSS_TEAM_STRENGTH`: 90.0 (3x same-team)
- `CROSS_TEAM_SLIDE`: 15.0 (perpendicular nudge to slide past)
- `SEPARATION_RADIUS`: 20.0 (slightly larger than 2x UNIT_RADIUS)
- `SEPARATION_MAX`: 60.0 (cap output magnitude)

**Key design**: The `perp()` lateral nudge prevents head-on oscillation by sliding opposing units sideways past each other, rather than bouncing back and forth. This is the critical piece that basic separation misses.

**Contact-zone optimization**: Only `Seeking`, `Engaging`, and `Attacking` units get separation applied. `Moving` units are streaming along the flow field, naturally spread out. Saves 60-75% of separation work at 40k units.

**Note**: ALL units are inserted into `UnitSpatialHash` regardless of state, so Seeking/Engaging units can push away from Moving units passing through. The filter is only on who RECEIVES a force.

### 4.6 Spatial Hash Optimization — Flat Grid

Replace `HashMap<(i32, i32), Vec<Entity>>` with flat `Vec<Vec<Entity>>` for fixed-size battlefield:

```rust
pub struct FlatSpatialGrid {
    cell_size: f32,
    width: u32,     // 82 (TOTAL_COLS)
    height: u32,    // 10 (BATTLEFIELD_ROWS)
    cells: Vec<Vec<Entity>>,  // length = width * height = 820
}

impl FlatSpatialGrid {
    fn cell_index(&self, position: Vec2) -> Option<usize> {
        let col = (position.x / self.cell_size).floor() as i32;
        let row = (position.y / self.cell_size).floor() as i32;
        if col < 0 || col >= self.width as i32 || row < 0 || row >= self.height as i32 {
            return None;
        }
        Some((row as usize) * (self.width as usize) + (col as usize))
    }

    /// Zero-allocation neighbor query via callback.
    pub fn for_each_neighbor(
        &self, position: Vec2, radius: f32, mut callback: impl FnMut(Entity),
    ) {
        let min_col = ((position.x - radius) / self.cell_size).floor().max(0.0) as usize;
        let max_col = ((position.x + radius) / self.cell_size).floor()
            .min((self.width - 1) as f32) as usize;
        let min_row = ((position.y - radius) / self.cell_size).floor().max(0.0) as usize;
        let max_row = ((position.y + radius) / self.cell_size).floor()
            .min((self.height - 1) as f32) as usize;

        for row in min_row..=max_row {
            for col in min_col..=max_col {
                for &entity in &self.cells[row * self.width as usize + col] {
                    callback(entity);
                }
            }
        }
    }

    fn clear(&mut self) {
        for cell in &mut self.cells { cell.clear(); } // retains allocated capacity
    }
}
```

**Benefits**: Zero hash overhead (direct array index), predictable memory layout, no hash collisions, no resize. 820 entries fits trivially in L1 cache headers.

**Two separate grids** (not unified):
- `TargetSpatialHash` (64px cells): all `Target` entities (~200 entities: buildings + fortresses + enemy units). Used by targeting queries.
- `UnitSpatialHash` (24px cells): all `Unit` entities (~40k). Used by separation force. Smaller cell size matches tighter `SEPARATION_RADIUS`.

Unifying them would force separation queries to iterate through non-unit targets unnecessarily.

---

## 5. System Pipeline (Revised GameSet Flow)

```
Every Frame:
┌──────────────────────────────────────────────────────┐
│ GameSet::Input                                        │
│ - Camera pan, building placement                     │
│ - Building change → set FlowFieldDirty               │
│ - Building placed → eject units from blocked cells   │
└──────────────────────────────────────────────────────┘
                    ↓
┌──────────────────────────────────────────────────────┐
│ GameSet::Production                                  │
│ - Barracks spawn timers                              │
│ - New units get AssignedGoal + TargetingState::Moving │
│ - Static entities spawn with TargetingState::Seeking  │
└──────────────────────────────────────────────────────┘
                    ↓
┌──────────────────────────────────────────────────────┐
│ GameSet::Ai                                           │
│ 1. recompute_flow_fields() [only if FlowFieldDirty]  │
│    - Dijkstra from each goal, 3-tier cost field       │
│    - 8-connected with corner-cutting prevention       │
│ 2. rebuild_target_grid()                             │
│    - Populate TargetSpatialHash with all Targets      │
│ 3. detect_enemies() [time-sliced]                    │
│    - Moving units check for nearby enemies            │
│    - detection_radius = max(range * 2.0, 64.0)       │
│    - Transition: Moving → Seeking                     │
│ 4. find_target() [time-sliced]                       │
│    - Seeking units pick best target from grid         │
│    - Backtrack limit as candidate filter              │
│    - Transition: Seeking → Engaging(entity)           │
│    - Set EngagementLeash origin                       │
│ 5. verify_targets()                                  │
│    - Engaging: check leash distance + backtrack       │
│    - Attacking: check range + hysteresis (8px)        │
│    - Invalid → Seeking or Moving                      │
└──────────────────────────────────────────────────────┘
                    ↓
┌──────────────────────────────────────────────────────┐
│ GameSet::Movement                                    │
│ 1. flow_field_movement()                             │
│    - Moving/Seeking: read flow field direction        │
│    - Engaging: steer directly toward target           │
│    - Attacking: velocity = 0                         │
│ 2. rebuild_unit_grid()                               │
│    - Populate UnitSpatialHash with all Units          │
│ 3. compute_separation() [Seeking/Engaging/Attacking]  │
│    - Team-weighted push + lateral nudge               │
│    - Write SeparationForce component                  │
│ 4. apply_separation()                                │
│    - Read SeparationForce, write Transform            │
│ 5. clamp_to_battlefield()                            │
│    - Prevent units from leaving map bounds            │
└──────────────────────────────────────────────────────┘
                    ↓
┌──────────────────────────────────────────────────────┐
│ GameSet::Combat                                      │
│ 1. attack()                                          │
│    - Attacking units: tick timer, spawn projectiles   │
│    - Range check: EntityExtent::surface_distance_from │
│ 2. move_projectiles()                                │
│    - Homing toward target, distance-based hit         │
│ 3. apply_damage()                                    │
│    - On hit: reduce health, despawn projectile       │
└──────────────────────────────────────────────────────┘
                    ↓
┌──────────────────────────────────────────────────────┐
│ GameSet::Death                                       │
│ 1. check_death()                                     │
│    - health <= 0 → despawn entity                    │
│    - On<Remove, Target> observer fires automatically  │
│      → orphaned Engaging/Attacking units → Seeking    │
└──────────────────────────────────────────────────────┘
                    ↓
┌──────────────────────────────────────────────────────┐
│ GameSet::Ui                                          │
│ - Health bars, gold display, etc.                    │
└──────────────────────────────────────────────────────┘
```

---

## 6. Performance Comparison

### Per-Frame Cost at 40k-100k Units

| Operation | Current | Proposed |
|-----------|---------|----------|
| **Movement direction** | O(n) navmesh waypoint follow | O(1) flow field lookup per unit |
| **Pathfinding** | O(n) A* calls every 0.5s burst | O(grid_size) Dijkstra only on map change |
| **Target finding** | O(n) spatial queries (staggered) | O(contact_zone) spatial queries (only nearby units) |
| **Range check** | O(n) GJK surface_distance (~10-20 iterations) | O(n) EntityExtent distance (1 subtraction or 4 ops + sqrt) |
| **Death handling** | O(n) poll + mass retarget spike | O(1) observer, orphans → Seeking |
| **Collision avoidance** | O(n) ORCA solver + O(n log n) physics | O(contact_zone) team-weighted separation |
| **Physics step** | O(n log n) broad-phase on all bodies | None — removed entirely |
| **Projectile hits** | O(p) sensor collision detection | O(p) distance check |
| **Spatial hash** | O(n) HashMap insertions (2 hashes) | O(n) flat array insertions (2 grids, cache-friendly) |

**Key insight**: At any given time, most units are just marching forward. Only units in the contact zone (~5-10% of total) need spatial queries, separation, and combat logic. The proposed architecture makes the 90% cheap path truly cheap (one array lookup) and limits expensive operations to the units that need them.

---

## 7. What Gets Removed

| Module/Dependency | Reason |
|-------------------|--------|
| `vleue_navigator` (crate) | Flow fields replace navmesh pathfinding |
| `avian2d` (crate) | Separation force + distance checks replace physics |
| `NavPath` component | No more per-unit waypoint paths |
| `PathRefreshTimer` resource | No periodic path recomputation |
| `NavObstacle` component | Buildings are blocked cells in flow field cost grid |
| `compute_paths` system | Replaced by `flow_field_movement` |
| `snap_to_mesh()` function | No navmesh edge to snap to |
| `units/avoidance/` module (ORCA) | Replaced by team-weighted separation force |
| `AvoidanceAgent` component | Not needed |
| `AvoidanceConfig` resource | Not needed |
| `AvoidanceSpatialHash` resource | Replaced by `UnitSpatialHash` |
| `third_party/avian.rs` | No physics |
| `surface_distance()` helper | Replaced by `EntityExtent::surface_distance_from()` |
| `RigidBody`, `Collider`, `Sensor` on all entities | No physics |
| `CollisionLayers`, `CollidingEntities` | No physics sensors |
| `LinearVelocity` component | Direct Transform updates |
| `PreferredVelocity` component | Flow field direction used directly |
| `CurrentTarget` component | Replaced by `TargetingState` enum |

---

## 8. What Gets Added

| New Code | Purpose |
|----------|---------|
| `FlowField` struct + Dijkstra computation | Direction grid per goal with 3-tier cost field |
| `FlowFieldDirty` flag resource | Trigger recomputation on map change |
| `GoalRegistry` resource (`Vec<GoalInfo>`) | Track active goals and their flow fields |
| `AssignedGoal` component | Which goal a unit is marching toward |
| `TargetingState` enum component | `Moving` / `Seeking` / `Engaging(Entity)` / `Attacking(Entity)` |
| `EngagementLeash` component | Origin + max distance for leash-based disengagement |
| `EntityExtent` enum component | `Circle(f32)` / `Rect(f32, f32)` for cheap range checks |
| `SeparationForce` component | Intermediate push vector (two-system split, zero snapshot) |
| `FlatSpatialGrid` struct | Flat `Vec<Vec<Entity>>` replacing HashMap-based spatial hash |
| `UnitSpatialHash` resource | All units, for separation force neighbor queries |
| `On<Remove, Target>` observer | Notifies orphaned units on target death → Seeking |
| `detect_enemies` system | Time-sliced check for Moving → Seeking transition |
| `verify_targets` system | Leash + backtrack + hysteresis checks |
| `flow_field_movement` system | Read flow field or steer to target, per TargetingState |
| `compute_separation` system | Team-weighted push + lateral nudge, writes SeparationForce |
| `apply_separation` system | Reads SeparationForce, writes Transform |
| `clamp_to_battlefield` system | Prevent units leaving map bounds |
| `eject_units_from_building` system | Push units out of newly-placed building cells |

---

## 9. Ticket Plan

### Ticket 1a: Add TargetingState + Death Observer (foundation, additive)

**Scope**: Add `TargetingState` component alongside existing `CurrentTarget` (both coexist). AI system writes both. Add `On<Remove, Target>` death observer. Add `EngagementLeash`. New units spawn with both `CurrentTarget(None)` and `TargetingState::Seeking`. Fortresses spawn with `TargetingState::Seeking`.

**Key detail**: Existing consumers (`movement.rs`, `attack.rs`, `pathfinding.rs`) keep reading `CurrentTarget`. Only the death observer reads `TargetingState`. This limits blast radius to ~5 test changes.

**Why first**: Incremental addition, no breaking changes. Death observer + orphan notification works immediately.

**Supersedes**: GAM-56 (mass retarget optimization)

**Files touched**: `gameplay/mod.rs` (new components), `gameplay/ai.rs` (write both), `gameplay/combat/death.rs` (add observer), spawn sites (add TargetingState), `testing.rs` (add to helpers)

### Ticket 1b: Migrate Consumers to TargetingState, Remove CurrentTarget

**Scope**: Migrate `unit_movement`, `attack`, `compute_paths` to read `TargetingState` instead of `CurrentTarget`. Remove `CurrentTarget` from `mod.rs`, all spawn sites, all test helpers. Update ~40 tests.

**Why separate**: Mechanical rename — no logic changes, easy to review despite touching many files.

**Files touched**: `gameplay/units/movement.rs`, `gameplay/combat/attack.rs`, `gameplay/units/pathfinding.rs`, `gameplay/mod.rs`, all spawn sites, `testing.rs`, ~40 tests

### Ticket 2: Combat State & Range Simplification

**Scope**: Add `EntityExtent` component to all targetable entities. Replace GJK `surface_distance` calls with `EntityExtent::surface_distance_from()` in ai, movement, and attack systems. Units: `EntityExtent::Circle(UNIT_RADIUS)`. Fortresses: `EntityExtent::Rect(64.0, 64.0)`.

**Why second**: Decouples combat from physics colliders. Removes per-frame GJK cost. Prepares for physics removal. During this ticket, entities carry both `Collider` (for physics) and `EntityExtent` (for game logic) — ~8 bytes redundancy per entity.

**Supersedes**: GAM-47 (crowd pushing — becomes non-issue once physics is removed)

**Files touched**: `gameplay/mod.rs` (EntityExtent), `gameplay/ai.rs`, `gameplay/units/movement.rs`, `gameplay/combat/attack.rs`, all entity spawn sites, `testing.rs`

### Ticket 3: Flow Field Infrastructure + Remove Navmesh

**Scope**: Implement `FlowField` struct with Dijkstra on 8-connected grid with 3-tier cost field. `FlowFieldDirty` flag on building change. `AssignedGoal` component (default: enemy fortress). `GoalRegistry` resource. Replace `compute_paths` + navmesh movement with `flow_field_movement`. Unit ejection on building placement. Remove `vleue_navigator` dependency, `NavPath`, `PathRefreshTimer`, `NavObstacle`, `snap_to_mesh`. Keep `PreferredVelocity` alive (flow field writes it, ORCA still reads it until Ticket 4).

**De-risk strategy**: Implement flow fields in new `gameplay/flow_field.rs` BEFORE removing navmesh. Add `dev_tools` runtime toggle between flow field and navmesh movement. Validate flow fields work. Remove navmesh in separate commit.

**Why third**: Biggest structural change. Depends on Ticket 2 (EntityExtent for range checks in movement).

**Supersedes**: GAM-43 (stagger navmesh paths — no navmesh), GAM-55 (final approach gap — direct steering in Engaging state handles it)

**Files touched**: New `gameplay/flow_field.rs`, `gameplay/units/movement.rs` (rewrite), `gameplay/units/pathfinding.rs` (delete), `gameplay/battlefield/` (dirty flag + ejection), `Cargo.toml`

### Ticket 4: Separation Force + Remove ORCA

**Scope**: Implement team-weighted separation with lateral nudge as two-system split (`compute_separation` + `apply_separation`). Add `SeparationForce` component. Add `UnitSpatialHash` (flat grid). Remove ORCA module, `AvoidanceAgent`, `AvoidanceConfig`, `AvoidanceSpatialHash`. Remove `PreferredVelocity` (no longer needed — flow field writes Transform directly, separation uses SeparationForce).

**Why fourth**: Depends on flow field being in place (ORCA was compensating for lack of global routing).

**Files touched**: New `gameplay/units/separation.rs`, delete `gameplay/units/avoidance/` (entire module), unit spawn sites, `gameplay/mod.rs`

### Ticket 5: Remove avian2d

**Scope**: Strip all physics components from all entities. Replace projectile sensor hit detection with distance check. Remove `third_party/avian.rs`. Remove avian2d from `Cargo.toml`. Direct `Transform` updates everywhere.

**Why fifth**: Final cleanup after all physics replacements are in place. Every system that read `Collider` was already migrated to `EntityExtent` in Ticket 2.

**Files touched**: `third_party/avian.rs` (delete), `third_party/mod.rs`, all entity spawn sites, `gameplay/combat/attack.rs` (projectile hit detection), `Cargo.toml`

### Ticket 6: Profiling & Tuning Pass (40k target)

**Scope**: Profile at 4k, 10k, 40k units. Switch unit rendering from `Mesh2d` to `Sprite::from_color()` (optimized sprite batching). Tune constants: flow field cell size, separation strength/radius, detection radius, spatial hash cell size, ATTACK_HYSTERESIS, LEASH_DISTANCE. Validate frame budget (<16ms at 40k). Smoke test at 100k. If orphan scan is hot, add reverse-lookup index. Identify remaining hotspots.

**Why last**: Validates the entire refactoring.

**Supersedes**: GAM-45 (profile O(n) hotspots — rewritten scope)

---

## 10. Component Migration Table

| Ticket | Added | Coexists (temporary) | Removed |
|--------|-------|---------------------|---------|
| **1a** | `TargetingState`, `EngagementLeash` | `CurrentTarget` (still written by AI, still read by movement/attack/pathfinding) | Nothing |
| **1b** | Nothing | Nothing | `CurrentTarget` |
| **2** | `EntityExtent` | `Collider` (still on entities for physics) + `EntityExtent` (used by range checks) | GJK `surface_distance` calls |
| **3** | `FlowField`, `FlowFieldDirty`, `GoalRegistry`, `AssignedGoal` | `Collider` + `RigidBody` (physics still active), `PreferredVelocity` (flow field writes it, ORCA reads it) | `NavPath`, `PathRefreshTimer`, `NavObstacle`, `vleue_navigator` |
| **4** | `UnitSpatialHash`, `SeparationForce` | `Collider` + `RigidBody` (physics still active) | `AvoidanceSpatialHash`, `AvoidanceAgent`, `AvoidanceConfig`, `PreferredVelocity`, entire `avoidance/` module |
| **5** | Nothing | Nothing | `Collider`, `RigidBody`, `Sensor`, `CollisionLayers`, `CollidingEntities`, `LockedAxes`, `LinearVelocity`, `avian2d` dep, `third_party/avian.rs` |
| **6** | Nothing | Nothing | Nothing (profiling + tuning only) |

### Spatial Hash Migration

| Ticket | TargetSpatialHash | AvoidanceSpatialHash | UnitSpatialHash |
|--------|-------------------|----------------------|-----------------|
| 1a-3 | Exists | Exists (for ORCA) | Not yet |
| 4 | Exists | **Removed** (ORCA deleted) | **Added** (flat grid) |
| 5-6 | Exists | Gone | Exists |

No period with 3 hashes coexisting. Direct replacement in Ticket 4.

---

## 11. Ticket Impact Analysis

### 11.1 Tickets to Close (subsumed by this rework)

| Ticket | Title | Reason |
|--------|-------|--------|
| **GAM-56** | Optimize mass retargeting when focused enemy dies | Subsumed by Ticket 1a (On<Remove, Target> observer) |
| **GAM-43** | Stagger navmesh path refreshes across frames | Subsumed by Ticket 3 (flow fields replace navmesh entirely) |
| **GAM-55** | Units stop at navmesh edge instead of closing final gap | Subsumed by Ticket 3 (Engaging state steers directly to target) |
| **GAM-47** | Dense unit crowds push enemies out of combat range | Subsumed by Ticket 5 (physics removed, no more pushing) |
| **GAM-45** | Profile and optimize remaining O(n) hotspots | Rewritten as Ticket 6 (new scope after architecture change) |

### 11.2 Tickets to Cancel (physics removal makes them obsolete)

| Ticket | Title | Reason |
|--------|-------|--------|
| **GAM-28** | Physics-based projectile movement (LinearVelocity + SweptCcd) | Moves in opposite direction — avian2d is being removed entirely. Projectiles become homing with distance-based hit detection. |
| **GAM-29** | Tier 2 integration test for hitbox/hurtbox collision layer wiring | Tests physics pipeline (broadphase → narrowphase → CollidingEntities) which is deleted in Ticket 5. Collision layers, sensors, CollidingEntities all removed. |

### 11.3 Tickets to Defer (scope changes after rework)

| Ticket | Title | Impact | Recommendation |
|--------|-------|--------|----------------|
| **GAM-34** | Refactor fortress/building spawning with named bundles | Spawn sites lose ~8 physics components (Collider, RigidBody, Sensor, CollisionLayers, LockedAxes, LinearVelocity...) — bundle composition changes drastically. | Defer until after Ticket 5. Refactor the simplified post-physics spawn sites. |
| **GAM-14** | Update test coverage to 95% | Many systems being tested will be rewritten or deleted (compute_paths, ORCA, physics wiring). | Defer until after Ticket 6. Coverage targets shift dramatically. |
| **GAM-52** | Test coverage push: 65% → 85%+ | Lists `compute_paths` as zero-coverage target, but it gets deleted in Ticket 3. Many systems rewritten. | Defer until after Ticket 6. Coverage targets shift dramatically. |

### 11.4 Tickets to Update (still valid, implementation changes)

| Ticket | Title | Impact |
|--------|-------|--------|
| **GAM-30** | Projectiles should continue flying after target dies | Still valid. Simpler to implement without physics — cache last known position, fly to it, despawn on arrival. No sensor/collider concerns. Update description to reflect non-physics approach. |

---

## 12. Migration Strategy

The 7 tickets are ordered so that **each one leaves the game in a working state**:

1. **Ticket 1a** (Add TargetingState) — additive, ~5 test changes, death observer works immediately
2. **Ticket 1b** (Remove CurrentTarget) — mechanical migration, ~40 test changes, no logic changes
3. **Ticket 2** (EntityExtent) — adds range check component alongside Collider, GJK replaced
4. **Ticket 3** (Flow fields) — removes navmesh, de-risked with dev toggle before navmesh deletion
5. **Ticket 4** (Separation) — removes ORCA, replaces with team-weighted separation
6. **Ticket 5** (Remove physics) — clean removal, all replacements already in place
7. **Ticket 6** (Profile) — validation and tuning, no functional changes

**Rollback strategy**: Hard-commit to flow fields (no feature-gated navmesh fallback). De-risk via dev_tools runtime toggle in Ticket 3 — both systems coexist during development. Once validated, navmesh code is deleted and never comes back. Maintaining both as a permanent option would require duplicating the entire movement pipeline.

---

## 13. Physics Engine Role at 100k Scale

### Principle: Query Database, Not Simulation Engine

At 100k units, a physics engine should **never** be used to move units or resolve collisions between them. The broad-phase/narrow-phase solver will choke on that many dynamic bodies. However, physics engines remain useful as **spatial query databases** — asking questions about the world geometry without simulating forces.

### When You Might Need Physics Queries

| Query Type | Use Case | Our Game Today | Future Need |
|------------|----------|----------------|-------------|
| **Raycast** | Line of sight (can unit see target through buildings?) | Not needed — auto-target nearest, no LOS | Archers (GAM-17) might need LOS through buildings |
| **Hitscan** | Instant-hit weapons (laser, gun) | Not needed — projectiles fly as entities | Possible future unit types |
| **Shape cast** | AOE overlap (who is within explosion radius?) | **Spatial grid handles this** — no physics needed | Same |
| **Overlap test** | "Is this build spot clear?" | Currently uses physics overlap | Can use grid cell occupancy check |

### Why We Can Remove Physics Entirely (For Now)

Every current physics use case has a cheaper replacement:

1. **AOE detection** → spatial grid `for_each_neighbor(pos, radius, callback)`. O(1) lookup.
2. **Build spot validation** → grid cell occupancy check. O(1) lookup.
3. **Attack range** → `EntityExtent::surface_distance_from()`. One subtraction or 4 ops + sqrt.
4. **Unit separation** → team-weighted separation force. ~30 lines of code.
5. **Projectile hits** → distance check: `dist < threshold`. Already manually moved.

### When to Bring Physics Back (Selectively)

If future features need true physics queries, re-introduce avian2d with **only static colliders** (buildings, walls) — no dynamic bodies on units. Use it as a query engine:

- **Raycasts for LOS**: Cast ray from archer to target, check if building collider blocks it. Only buildings are in the physics world (~50 entities), not 100k units.
- **Ragdoll on death**: Brief `RigidBody::Dynamic` on a dying unit with explosive impulse for visual flair. Only a few hundred dead bodies at once, sleep after landing. Purely cosmetic.
- **Vehicles/bosses**: Rare high-value entities (~10-100) that benefit from realistic momentum. The rest of the army stays physics-free.

The key insight: **static colliders for queries are essentially free** — the physics broad-phase cost comes from dynamic bodies that move every frame. 50 static building colliders + raycasts = negligible cost. 100k dynamic unit bodies = game over.

### Alternative to Physics for LOS

For our 2D grid-based game, LOS can be computed without any physics engine:

```rust
/// Check if line from A to B is blocked by any building.
/// Walk the flow field grid cells along the line — if any cell is blocked, no LOS.
fn has_line_of_sight(from: Vec2, to: Vec2, flow_field: &FlowField) -> bool {
    // Bresenham's line algorithm on the flow field grid
    for cell in bresenham_line(
        flow_field.world_to_cell(from),
        flow_field.world_to_cell(to),
    ) {
        if flow_field.is_blocked(cell) {
            return false;
        }
    }
    true
}
```

Cost: ~10-20 cell checks per ray. No physics engine needed. The flow field grid already knows which cells are buildings.

---

## 14. Future Extensions (Not In Scope)

| Extension | How It Fits |
|-----------|-------------|
| **Player-directed attack goals** | Add goals to `GoalRegistry`, compute new flow field, reassign units via `AssignedGoal` |
| **Multiple unit types with different speeds** | Each unit reads same flow field, applies own `Movement.speed` |
| **Ranged units** | Same `TargetingState`, but transition to `Attacking` at longer range. Larger detection radius via `range * 2.0` |
| **Target priority** | Add `TargetPriority` component (Nearest/LowestHp/Buildings/Units) as tiebreaker in `find_target` |
| **AOE attacks** | Spatial grid `for_each_neighbor` at impact point — already supported |
| **Chained effects** (lightning) | Iterative spatial queries from each hit target |
| **Influence maps** | Layer on top of flow fields for strategic AI (enemy team auto-targeting priorities) |
| **Formation/squad system** | `AssignedGoal` per squad, units in squad use squad's goal + separation |
| **Dynamic obstacles** (moving walls, bridges) | Trigger flow field recompute, same as building placement |
| **Line of sight** | Bresenham's line on flow field grid (blocked cells = buildings), no physics needed |
| **Miss chance / accuracy** | `if rng.gen() > accuracy { despawn projectile }` — one line in hit check |
| **Reverse-lookup death index** | `HashMap<Entity, SmallVec<[Entity; 8]>>` if O(n) orphan scan is hot at scale |
| **Rendering at 100k** | GPU instancing or custom compute-shader point renderer if sprite batching bottlenecks |

---

## 15. Team Review Summary

Architecture reviewed by 4 specialist agents on 2026-03-05. Key findings incorporated:

### Issues Found and Resolved

| Issue | Found By | Resolution |
|-------|----------|------------|
| Engaging state conflates steering + attacking | Combat Designer | Added 4th state: `Attacking(Entity)` |
| Target inheritance causes focus-fire clumping | Combat Designer | Dropped inheritance — orphans transition to `Seeking` |
| No oscillation prevention (Engaging ↔ Moving) | Combat Designer | Added `EngagementLeash` + `ATTACK_HYSTERESIS` |
| `UnitRadius` doesn't work for rectangles | Combat Designer | Replaced with `EntityExtent` enum (Circle/Rect) |
| Flow field binary blocked/unblocked clips corners | Movement Architect | 3-tier cost field with buffer ring |
| Separation can't handle head-on army collisions | Movement Architect | Team-weighted separation + lateral `perp()` nudge |
| Units stuck in cells after building placement | Movement Architect | Radial ejection to nearest unblocked cell |
| 4-connected grid produces staircase movement | Movement Architect | 8-connected with corner-cutting prevention |
| `Dead` marker causes archetype moves | Perf Engineer | Use `On<Remove, Target>` observer instead — zero archetype cost |
| HashMap spatial hash has poor cache locality | Perf Engineer | Flat `Vec<Vec<Entity>>` grid (820 cells, fits L1 cache) |
| `query_neighbors` allocates per call | Perf Engineer | Callback API: `for_each_neighbor(pos, radius, callback)` |
| Separation snapshot is O(n) memory | Perf Engineer | Two-system split: compute → apply via `SeparationForce` component |
| Rendering at scale unaddressed | Perf Engineer | Switch to `Sprite::from_color()`, smoke test at 40k in Ticket 6 |
| Ticket 1 blast radius underestimated (10 files, ~50 tests) | Sys Integrator | Split into 1a (additive, ~5 tests) + 1b (migration, ~40 tests) |
| Fortresses don't fit `Moving` state | Sys Integrator | Spawn with `Seeking`, never enter `Moving` |
| Backtrack limit lost silently | Sys Integrator | Preserved in `find_target` filter + `verify_targets` disengage |
| Death observer ordering unclear | Sys Integrator + Perf Engineer | Consensus: `On<Remove, Target>` — verified in Bevy 0.18 source |
| Spatial hash migration path unclear | Sys Integrator | 3-phase plan: 2 hashes → 2 hashes (rename) → 2 hashes |
| No rollback plan for flow fields | Sys Integrator | Dev toggle before navmesh removal, hard-commit after validation |

### Deferred to Ticket 6 (Profiling)

- Reverse-lookup index for O(n) orphan scan on mass death
- Flow field Dijkstra benchmark with complex building layouts
- Separation constant tuning (strength, radius, cross-team weight)
- Rendering benchmark at 40k (`Sprite::from_color()` vs `Mesh2d`)
- Verify `propagate_transforms` overhead with 40k flat entities
