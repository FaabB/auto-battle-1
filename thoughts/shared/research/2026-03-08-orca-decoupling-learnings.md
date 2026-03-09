# ORCA Decoupling Learnings (GAM-61 Experiment)

  

**Date**: 2026-03-08 – 2026-03-09

**Branch**: `worktree-gam-61-decouple-orca-direct-transform`

**Status**: Working well — ready for review/merge consideration.

  

## What We Did

  

Replaced avian2d physics-driven movement with direct `Transform` writes:

  

1. Added `AdjustedVelocity` component (replaces `LinearVelocity` from avian2d)

2. `compute_avoidance` writes ORCA result to `AdjustedVelocity` instead of `LinearVelocity`

3. New `apply_movement` system: `Transform.translation += AdjustedVelocity * dt`

4. Units switched from `RigidBody::Dynamic` to `RigidBody::Kinematic`

5. `LinearVelocity` removed from all unit entities

  

The ORCA algorithm itself (`orca.rs`) was NOT changed — only how its output integrates into the world.

  

## What Broke

  

### Problem 1: Units overlap when one is attacking and others approach

  

**Root cause**: Before, avian2d's collision solver was a safety net. Even if ORCA didn't perfectly prevent overlap, `RigidBody::Dynamic` bodies pushed each other apart. With `RigidBody::Kinematic`, there is zero collision resolution between units.

  

ORCA is **predictive** — it computes collision-free *velocities* for future timesteps. It cannot prevent overlap when:

- Multiple units converge to the same point (a target)

- A unit stops (attacking) and others keep approaching

- Units are already overlapping from spawn proximity

  

### Problem 2: Units overlap while marching (before engaging)

  

**Root cause**: Units spawning near each other start close/overlapping. ORCA generates weak constraints for parallel-moving agents (same direction = no collision course detected). Without physics pushback, overlapping units stay overlapping.

  

### Problem 3: Attacking units slide when dodging is enabled

  

We tried removing the zero-preferred-velocity short-circuit so attacking units would dodge incoming units. Result: attacking units slide around during combat, which looks wrong. **Attacking units must stand still.** Reverted.

  

## ORCA Tuning Attempts (All Insufficient Alone)

  

### 1. Inflated ORCA radius (kept, increased to 2.0×)

- `AVOIDANCE_RADIUS = UNIT_RADIUS * 2.0` (12.0px vs 6.0px visual)

- Started at 1.3×, increased to 2.0× for stronger separation

- Helps ORCA plan avoidance with more margin but doesn't prevent convergence

  

### 2. Removed velocity smoothing (kept)

- `DEFAULT_VELOCITY_SMOOTHING = 1.0` (was 0.85)

- ORCA corrections apply immediately instead of being diluted over frames

- Minor improvement, not sufficient alone

  

### 3. Static time horizon for attacking units (kept)

- Added `static_time_horizon = 0.5s` (vs 3.0s for moving neighbors)

- When a neighbor has zero preferred velocity (attacking), use shorter time horizon

- Makes ORCA constraints more aggressive near stationary units

- Did NOT prevent overlap — the constraint is still velocity-based, not positional

  

### 4. Increased time horizon (kept)

- `DEFAULT_TIME_HORIZON = 5.0` (was 3.0)

- Agents start avoiding earlier, giving more time to steer around

- Marginal improvement for dense groups

  

### 5. Removed zero-preferred short-circuit (reverted)

- Let attacking units participate in ORCA (dodge incoming units)

- Made attacking units slide around — unacceptable

- **Attacking units must stay still**, so they can't participate in avoidance

  

### 6. Let stationary units run ORCA only for overlapping neighbors (reverted)

- Instead of short-circuiting to zero, stationary units would only generate ORCA constraints for neighbors within combined radius (overlapping)

- Result: attacking units "dance" — ORCA pushes them out of attack range, AI transitions back to Engaging, they walk back, overlap, ORCA pushes again → oscillation

- **Key lesson**: ORCA-based separation for stationary units is fundamentally broken because it creates velocity that moves them out of combat range

  

## Position Projection / resolve_overlaps

  

### First attempt (earlier session, reverted)

- Symmetric: both units pushed equally → attacking units get shoved around

- Asymmetric proposed but not tested

  

### Second attempt (this session, kept)

- Added `resolve_overlaps` system after `apply_movement` as a hard constraint

- Detects overlapping pairs (center distance < 2× UNIT_RADIUS)

- Asymmetric: moving units get pushed, stationary units stay planted

- Both stationary → split equally to unstick

- **Result**: helps prevent the worst visual overlap but doesn't solve the fundamental convergence problem. With 50+ units, the O(n²) pairwise check is also a performance concern.

  

## Goal Spreading / Engagement Slot Attempts

  

### Attempt 1: AttackPosition with approach angle (reverted)

- Ring position based on unit's approach angle to target

- **Failed**: all units march from the same direction → same angle → same position

  

### Attempt 2: AttackPosition with entity index (reverted)

- Ring position based on `entity.index() % ATTACK_SLOTS`

- Naive distribution, didn't account for approach direction or available space

  

### Attempt 3: Grid-based BFS engagement slots (replaced)

- Small grid (cell_size = UNIT_RADIUS * 2.5) around target

- BFS from unit position to find nearest free cell within attack range

- **Failed**: attack range is 5px, cell size is 15px → only ~4 cells fit within attack range of a unit-sized target. Not enough slots for 5+ units.

- Also: BFS from unit position → when cells near target are full, units get assigned slots BEHIND themselves (away from target), forming a backwards queue

  

### Attempt 4: Grid-based BFS from target (replaced)

- Fixed the backwards queue by BFS from target center outward

- **Failed**: cells at target center are INSIDE the target body. Adjacent cells are just outside attack range. No cells exist in the valid attack ring due to cell size vs attack range geometry.

  

### Attempt 5: Ring-based positions with greedy matching (current)

- Concentric rings around target, first ring at attack range distance

- Ring 1 radius = target_surface + UNIT_RADIUS + attack_range × 0.5

- Each ring fits `floor(2π × radius / UNIT_SPACING)` positions

- For unit targets: ~7 positions on first ring (within attack range), ~13 on second ring (queue)

- Greedy matching: each unit gets the closest available ring position to their current position (prevents walking through target to reach far side)

- Recomputed every frame so units advance when slots free up

- `verify_targets` requires units to reach their slot before transitioning Engaging → Attacking

- **Result**: ring shape visible in screenshots, but with 50+ units the outer rings are still densely packed and overlapping. The system works conceptually but can't handle the sheer volume of units in practice.

  

## go-orca/RVO2 Reference Comparison

  

Compared our `orca.rs` against [go-orca](https://github.com/downflux/go-orca) (a Go implementation of RVO2/ORCA).

  

### CRITICAL: Overlap handling (fixed in orca.rs)

  

**Our code (before fix)**: Returned `None` when agents overlap — no ORCA constraint generated.

  

**go-orca/RVO2 reference**: Uses `minTau = 1e-3` when overlap detected — generates the most aggressive possible constraint, effectively saying "separate within 1ms."

  

**Fix applied**: Replaced the `None` return with `MIN_TAU = 1e-3` approach matching RVO2. Now overlapping agents get maximum-urgency separation constraints instead of being invisible to each other. Also handles degenerate case (co-located agents) with fallback direction.

  

**Impact**: Prevents the worst case where overlapping units could never self-correct. However, with many units converging, the LP3 fallback (infeasible constraint resolution) produces compromised velocities that don't fully separate dense groups.

  

### Neighbor search radius

  

**Ours**: Flat `config.neighbor_distance` = `time_horizon * 50.0 + avoidance_radius * 2.0`.

  

**go-orca**: Dynamic per-agent: `tau * agent.Speed + 2 * agent.Radius`. More precise.

  

### LP solver

  

Both implementations equivalent: 2D linear programming, fall back to constraint projection when infeasible.

  

## Key Architectural Insights

  

### 1. Physics solver was doing more than we thought

Avian2d's collision solver provided THREE things:

- **Velocity integration** (replaced by `apply_movement`) ✅

- **Predictive collision avoidance** (ORCA handles this) ✅

- **Positional overlap resolution** (partially handled by `resolve_overlaps`) ⚠️

  

The third one is the critical gap. `resolve_overlaps` does simple pairwise push but lacks the iterative convergence of a real physics solver.

  

### 2. ORCA is a velocity-space algorithm, not a position-space algorithm

ORCA answers: "what velocity should I use to avoid future collisions?" It does NOT answer: "I'm currently overlapping, how do I separate?" The `minTau` trick helps for pairs but doesn't converge for dense groups.

  

### 3. Units converging to the same point is unsolvable by ORCA alone

When N units all want to reach the same position, ORCA can slow them down and steer laterally, but eventually they'll all arrive at the same spot. Goal spreading (different destinations per unit) is the correct solution.

  

### 4. Attack range geometry makes goal spreading extremely difficult

- Soldier attack range: 5.0px (surface distance)

- Unit radius: 6.0px

- Valid attack ring (center-to-center): 12–17px from unit target, ~5px wide

- At this scale, grid cells (12-15px) don't align with the ring, and only ~7 ring positions fit

- **This is probably the most fundamental issue**: the attack range is too small for meaningful spatial distribution of units

  

### 5. Three-layer approach works but doesn't scale to 50+ units

For a complete solution without physics:

1. **ORCA** — predictive velocity-space avoidance ✅ (implemented with minTau fix)

2. **Goal spreading** — ring-based engagement slots ✅ (implemented with greedy matching)

3. **Position projection** — resolve_overlaps system ✅ (implemented, asymmetric)

  

Each layer solves a different class of problem. Together they produce visible improvement (ring shape forms around targets) but can't handle the volume of units in the game. With 50+ green units attacking 1 red unit, the outer rings are still densely packed.

  

### 6. Attacking units MUST NOT participate in avoidance

Every attempt to let attacking units move (dodge, ORCA separation, etc.) causes them to slide out of attack range → re-engage → walk back → oscillation. The only safe correction for overlapping attacking units is position projection (instant displacement, no velocity).

  

### 7. System ordering matters for engagement slots

- `assign_engagement_slots` runs in `GameSet::Ai` (same set as `find_target`)

- Within a set, system order is not guaranteed unless chained

- There's a potential 1-frame delay where a unit is Engaging without a slot

- Movement falls back to target-center steering during that frame

  

### 8. verify_targets must gate on slot proximity

Without the slot proximity check, units transition Engaging → Attacking as soon as they enter attack range, regardless of their assigned slot position. This causes all units approaching from the same side to freeze at the same point. Fix: only transition to Attacking if `distance_to_slot < UNIT_RADIUS * 2.0`.

  

## Files Changed (on this branch)

  

### Core changes (the decoupling):

- `src/gameplay/units/avoidance/mod.rs` — `AdjustedVelocity`, `PreferredVelocity`, `compute_avoidance`, `apply_movement`, `resolve_overlaps`

- `src/gameplay/units/avoidance/orca.rs` — `minTau` overlap handling (was returning `None`)

- `src/gameplay/units/mod.rs` — `RigidBody::Kinematic`, system chain with 5 systems

- `src/testing.rs` — updated test helpers

- `src/dev_tools/mod.rs` — debug viz uses `AdjustedVelocity`

  

### Engagement slot system (new):

- `src/gameplay/units/engagement.rs` — ring-based slot assignment, greedy matching, cleanup

- `src/gameplay/units/movement.rs` — steer toward `EngagementSlot` when engaging

- `src/gameplay/ai.rs` — `verify_targets` gates Engaging→Attacking on slot proximity

  

### ORCA tuning (experimental):

- `src/gameplay/units/avoidance/mod.rs` — inflated radius (2.0×), no smoothing, static time horizon, time horizon 5.0s

  

## Engager Cap + ORCA Priority (Session 2)

  

### Engager cap per unit target (kept, MAX = 6)

- `enforce_engager_cap` system runs after `find_target`, before `verify_targets`

- Groups all Engaging/Attacking units by target, keeps closest N, kicks farthest back to Moving

- Attacking units get priority (sorted first) so they're never evicted

- Only applies to unit targets (not buildings/fortresses)

- `find_target` also checks the cap when Seeking to prevent re-acquiring full targets

  

### Key fix: self-counting in engager cap

- First attempt: units counted themselves toward the cap → dropped their own target every retarget cycle → dancing

- Fix: pass `current_target` through to `search_radius`, exclude it from cap check

- Also needed: apply cap to retargeting branch too (not just Seeking), otherwise units switch TO full targets and get kicked → oscillation

  

### Dynamic ORCA responsibility (kept)

- Engaging/Attacking units: responsibility = 0.25 (they dodge less)

- Moving/Seeking units: responsibility = 0.75 (they yield to engaging units)

- Effect: marching units move out of the way of units heading to their target

  

### Engagement slots removed

- Ring-based engagement slots caused more problems than they solved at this scale

- Per-frame recomputation caused slot-swapping dance (units assigned different slots each frame)

- Sticky slots (keep existing) caused units to get stuck in outer rings forever

- With the engager cap (6 units max), density is manageable without spatial assignment

- Simpler approach: units steer directly toward target, stop at attack range, ORCA + resolve_overlaps handle separation

- Engagement slots may be worth revisiting when local steering is improved

  

### Boids-style separation (kept)

- `apply_separation` system runs after `rebuild_spatial_hash`, before `compute_avoidance`

- For each moving unit, queries neighbors within `SEPARATION_RADIUS` (4× unit radius = 24px)

- Adds repulsion vector away from each neighbor, weighted by inverse distance (closer = stronger)

- Blended into `PreferredVelocity`, clamped to max speed so separation doesn't speed units up

- Stationary units (attacking) are skipped — they stay planted

- `SEPARATION_STRENGTH = 30.0` — tuned to nudge without overpowering preferred velocity

- Complements the three-layer approach:

1. **Boids separation** — reactive, nudges nearby units apart before they overlap

2. **ORCA** — predictive, plans collision-free velocities for the future

3. **resolve_overlaps** — hard constraint, fixes any remaining overlap after movement

- Noticeably improved spreading in dense marching columns where ORCA alone was weak

  

## Current State (End of Session 2)

  

The engager cap + ORCA priority + boids separation approach produces good behavior:

- Units spread across available targets naturally (no pileups)

- Closest units get to attack (distance-based priority eviction)

- Marching units yield to engaging units (asymmetric ORCA responsibility)

- Boids separation smooths out dense columns that ORCA can't handle

- Simple direct steering toward target — no complex slot assignment

  

Remaining rough edges for future local steering work:

- Attack range geometry (5px) is very tight — may want to increase for visual clarity

- Separation + ORCA tuning constants may need further balancing with more unit types

  

## Target-Aware Tangential Separation + Same-Team Filtering (Session 3)

  

### Problem 1: units sharing a target push each other backwards

With radial separation (push away from neighbor), units engaging the same target push each other *away from the target*. This causes oscillation — units approach target, get pushed back, approach again.

  

### Fix: tangential push for shared targets (kept)

When two neighbors share the same target, the separation push is **tangential** (perpendicular to the target→unit line) instead of radial. This makes units slide *around* the target to fill gaps rather than pushing each other backwards.

  

- Direction from target to unit → compute perpendicular (tangent)

- Pick the tangent that points away from the neighbor (dot product with radial direction)

- Fallback to radial push if units are on top of target or don't share a target

  

### Problem 2: units avoiding enemies instead of fighting them (critical)

  

**Root cause**: Separation and ORCA were applied between ALL units regardless of team. This caused:

1. Engaging units getting separation push *away from* the enemy they're trying to reach

2. ORCA generating avoidance constraints against enemies, making units steer around them instead of closing to attack range

3. Units "walking past" enemies or "fleeing" at the start of engagement

  

**How we found it**: Debug visualization (target lines + velocity arrows, F3 toggle) showed engaging units with preferred velocity pointing along the flow field direction instead of toward their target. The green arrows (preferred velocity) pointed sideways — separation was overpowering the target-steering.

  

### Fix: same-team-only separation and ORCA (critical, kept)

  

Both `apply_separation` and `compute_avoidance` now filter by team:

- Separation only pushes against **same-team** neighbors

- ORCA only generates constraints for **same-team** neighbors

- Opposing-team units are completely invisible to both systems

- `resolve_overlaps` still applies between ALL units (hard positional constraint regardless of team)

  

**Key insight**: Avoidance systems answer "how do I not collide with this unit?" But for enemies, you *want* to collide — you want to get close and fight. Only same-team avoidance makes sense in a combat game.

  

### Engager cap kept at 12

- `MAX_ENGAGERS_PER_UNIT_TARGET = 12`

- Tested at 150 (effectively disabled) — looked good but units piled up

- Tested fully removed — looked nicer but "would be better with cap"

- 12 is the sweet spot: enough to surround targets, limits excessive pileups

  

### Debug visualization improved

- Yellow line: unit → target (Engaging state)

- Red line: unit → target (Attacking state)

- Green arrow: preferred velocity (after separation)

- Cyan arrow: ORCA-adjusted velocity (final)

- Toggle with F3 (cycles: player flow field → enemy flow field → off)

- Essential for debugging — immediately revealed the cross-team avoidance bug

  

---

  

## Final State — Complete Diff vs Main Branch

  

### System chains (final)

```

GameSet::Ai: rebuild_target_grid → find_target → enforce_engager_cap → verify_targets

GameSet::Movement: unit_movement → rebuild_spatial_hash → apply_separation → compute_avoidance → apply_movement → resolve_overlaps

```

  

### Critical code changes vs main (by file)

  

#### `src/gameplay/units/avoidance/mod.rs` — Core of all changes

  

**New components:**

- `AdjustedVelocity(Vec2)` — replaces `LinearVelocity` from avian2d. ORCA writes here, `apply_movement` reads.

  

**New constants:**

- `AVOIDANCE_RADIUS = UNIT_RADIUS * 2.0` — inflated radius for ORCA (was `UNIT_RADIUS`)

- `SEPARATION_RADIUS = UNIT_RADIUS * 4.0` — boids neighbor detection radius

- `SEPARATION_STRENGTH = 30.0` — boids repulsion force multiplier

- `ENGAGING_RESPONSIBILITY = 0.25` — ORCA: engaging units dodge less

- `MOVING_RESPONSIBILITY = 0.75` — ORCA: marching units yield more

  

**Tuning changes:**

- `DEFAULT_TIME_HORIZON = 5.0` (was 3.0) — look further ahead

- `DEFAULT_VELOCITY_SMOOTHING = 1.0` (was 0.85) — no smoothing, instant ORCA response

- `static_time_horizon = 0.5` (new) — shorter horizon for stationary neighbors

- `neighbor_distance` includes `AVOIDANCE_RADIUS * 2.0` margin

  

**`compute_avoidance` changes:**

- Query now includes `&TargetingState` and `&Team`

- Snapshots include `Team` for same-team filtering

- Dynamic responsibility based on `TargetingState` (not static `AvoidanceAgent.responsibility`)

- **Same-team filter**: skips ORCA constraints for opposing-team neighbors

- Static time horizon for stationary (attacking) neighbors

  

**New system: `apply_separation`**

- Boids-style reactive repulsion, runs before ORCA

- Same-team only — opposing units invisible

- Tangential push for shared targets (perpendicular to target→unit line)

- Radial push for different targets or no target

- Blended into `PreferredVelocity`, clamped to max speed

- Stationary units skipped

  

**New system: `apply_movement`**

- `Transform.translation += AdjustedVelocity * dt`

- Replaces avian2d's physics integration

  

**New system: `resolve_overlaps`**

- Hard positional overlap correction (ALL teams, not just same-team)

- Asymmetric: moving units get pushed, stationary (attacking) stay planted

- Both stationary → split equally to unstick

- Uses visual `UNIT_RADIUS`, not inflated `AVOIDANCE_RADIUS`

  

#### `src/gameplay/units/avoidance/orca.rs` — Bug fix

  

**Overlap handling (critical bug fix):**

- **Before**: `compute_orca_line` returned `None` when agents overlap — no constraint generated, agents invisible to each other

- **After**: Uses `MIN_TAU = 1e-3` (matching RVO2 reference) to generate aggressive separation constraint

- Handles degenerate case (co-located agents) with fallback direction

- This was a genuine bug that should be kept regardless of other changes

  

#### `src/gameplay/ai.rs` — Engager cap system

  

**New constant:** `MAX_ENGAGERS_PER_UNIT_TARGET = 12`

  

**New system: `enforce_engager_cap`**

- Runs after `find_target`, before `verify_targets`

- Groups Engaging/Attacking units by target

- Sorts: attacking first (never evicted), then by distance (closest first)

- Kicks excess units back to Moving/Seeking

- Only applies to unit targets (not buildings/fortresses)

  

**`find_target` changes:**

- Counts engagers per unit target before main loop (HashMap)

- Seeking branch passes engager cap with `current_target = None`

- Retargeting branch passes cap with `current_target` excluded from count (self-counting fix)

- `search_radius` skips candidates at cap

  

**Key fix: self-counting bug**

- First attempt: units counted themselves toward the cap → dropped own target every retarget → dancing

- Fix: pass `current_target` to `search_radius`, exclude from cap check

  

**`verify_targets` simplified:**

- Removed engagement slot proximity gate

- Simple range check: Engaging → Attacking when `distance <= stats.range`

  

#### `src/gameplay/units/mod.rs` — Wiring

  

- `RigidBody::Dynamic` → `RigidBody::Kinematic` (no more physics-driven movement)

- `LinearVelocity::ZERO` → `AdjustedVelocity::default()`

- System chain: 6 systems chained with `chain_ignore_deferred()`

- `engagement.rs` module declared (but systems not wired — dead code)

  

#### `src/gameplay/units/movement.rs` — Simplified steering

  

- Engaging branch: direct steer toward target, stop at attack range

- No engagement slot logic — removed `EngagementSlot` query

- Comment updated: ORCA writes `AdjustedVelocity` (not `LinearVelocity`)

  

#### `src/dev_tools/mod.rs` — Debug visualization

  

- Uses `AdjustedVelocity` instead of `LinearVelocity`

- Queries `&TargetingState` and `With<Target>`

- Draws target lines: yellow (Engaging), red (Attacking)

  

#### `src/testing.rs` — Test helpers

  

- Minor updates to accommodate new component requirements

  

#### `src/gameplay/units/engagement.rs` — Dead code (not wired)

  

- Ring-based slot system exists but is not in any schedule

- Generates `dead_code` warnings — candidate for deletion on merge

  

### What the full movement stack does now

  

1. **Engager cap (12)** — limits how many units can engage a single unit target

2. **Direct steering** — engaging units steer toward target, stop at attack range

3. **Same-team boids separation** — reactive repulsion between friendly units, tangential for shared targets

4. **Same-team ORCA** — predictive avoidance between friendly units only

5. **Direct transform writes** — `apply_movement` moves units without physics

6. **resolve_overlaps** — hard positional constraint for ALL units (cross-team)

  

Units now:

- March toward enemies without avoiding them

- Spread around targets tangentially instead of bunching on one side

- Friendly units avoid each other but not enemies

- Close to attack range cleanly without "dancing" or "fleeing"

- Excess engagers get evicted to find other targets

  

## Iterative Tuning Journeys

  

### Engager cap value: 6 → 12 → 150 → removed → 12

- **6**: Original value. Too low with tangential separation — units spread around targets well but cap forced too many to seek alternative targets

- **12**: Better. Enough to surround targets, still limits pileups

- **150**: Effectively disabled. Tested to see if cap was still needed — looked good

- **Removed entirely**: User said "looks good, remove it". Removed all cap code. Looked nicer initially

- **Back to 12**: User said "would be better with max engager cap". Restored via `git checkout HEAD -- src/gameplay/ai.rs`. **Lesson: keep features easy to toggle — `git checkout` is faster than re-implementing**

  

### Three separate "dancing" bugs (each with different root cause)

1. **Self-counting dance**: Units counted themselves toward engager cap → dropped own target every retarget cycle → re-acquired → loop. **Fix**: exclude `current_target` from cap check

2. **Retarget-to-full-target dance**: Cap applied to Seeking branch but not Engaging/Attacking retarget. Units retarget TO a full target, then get kicked by enforce_engager_cap → retarget again. **Fix**: apply cap to all `find_nearest_target` calls

3. **Kicked-unit re-acquisition dance**: `enforce_engager_cap` kicks unit to Moving, next frame detection triggers Seeking → acquires same full target → kicked again. **Fix**: also check cap in `find_target` Seeking branch, not just enforce_engager_cap post-hoc

  

### Far units blocking closer ones

- First-come-first-served cap: early-spawned far units counted toward cap, blocking closer units that spawned later

- **Fix**: `enforce_engager_cap` sorts by distance and evicts farthest, keeping closest N

  

## Why We Still Use avian2d

  

Units are `RigidBody::Kinematic` with `Collider::circle`, `LockedAxes`, and `solid_entity_layers()`. We still need avian2d for:

- **Collider queries** used by other systems (projectile hits, etc.)

- **Layer-based collision filtering** (pushbox/hurtbox layers)

- Potential future use for non-unit physics (projectiles, particles)

  

What we DON'T use from avian2d anymore:

- `LinearVelocity` — replaced by `AdjustedVelocity`

- `RigidBody::Dynamic` collision resolution — replaced by `resolve_overlaps`

- Physics integration (velocity → position) — replaced by `apply_movement`

  

The switch from Dynamic to Kinematic means avian2d's solver no longer moves units or resolves their overlaps. We do all of that ourselves now.

  

## Why resolve_overlaps Uses Visual Radius (Not AVOIDANCE_RADIUS)

  

`AVOIDANCE_RADIUS = UNIT_RADIUS * 2.0` is inflated so ORCA plans avoidance with margin. But `resolve_overlaps` is a hard positional push — using inflated radius would push units apart to 24px separation (2 × 12), which looks wrong. Visual radius (6px) means units can be side-by-side at 12px center distance without getting pushed, which matches their actual rendered size.

  

## Test Gotcha: Adding Components to Queries Breaks Tests

  

When we added `&Team` to `compute_avoidance` and `apply_separation` queries, the avoidance integration tests broke — test units spawned by `spawn_avoidance_unit` didn't have a `Team` component, so they silently stopped matching the query.

  

**Fix**: Add `Team::Player` to test unit spawning. **Lesson**: when adding required query components to systems, always grep for test helpers that spawn entities for those systems.

  

## Key Design Principles Discovered

  

1. **Avoidance is same-team only** — in a combat game, you want to reach enemies, not avoid them

2. **Tangential > radial** for shared-target separation — prevents backwards push oscillation

3. **Three layers serve different roles**: boids (reactive same-team), ORCA (predictive same-team), resolve_overlaps (hard constraint all-teams)

4. **Attacking units must stay planted** — never apply velocity-based corrections to attacking units

5. **Debug visualization is essential** — target lines + velocity arrows immediately revealed the cross-team avoidance bug

6. **Self-counting in caps causes oscillation** — always exclude the current entity's own target from cap checks

7. **Engagement slots don't work at small attack ranges** — 5px attack range means ~7 ring positions. Direct steering + cap + separation is simpler and works better

8. **Multiple "dancing" bugs can look identical** — always check debug viz to distinguish root cause (self-counting vs retarget-to-full vs re-acquisition)

9. **Iterative tuning needs easy rollback** — keeping code git-revertable per file saved significant time when we changed direction on the engager cap

10. **Physics engines do more than you think** — avian2d's Dynamic solver was providing overlap resolution, velocity integration, AND predictive avoidance correction all in one. Replacing it requires three separate systems.

  

## Implementation Guide for Fresh Agent

  

### Recommended implementation order

  

Do these in order — each step depends on the previous one compiling and tests passing.

  

1. **Fix orca.rs overlap handling** (independent, pure algorithm fix)

- Replace the `None` return in the overlap branch with `MIN_TAU = 1e-3` approach

- Add degenerate co-located fallback direction

- Update tests: `overlapping_agents_return_none` → `overlapping_agents_produce_separation_constraint`

  

2. **Add `AdjustedVelocity` component + `apply_movement` system**

- New component in `avoidance/mod.rs`

- Simple system: `Transform.translation += AdjustedVelocity * dt`

- Switch `compute_avoidance` to write `AdjustedVelocity` instead of `LinearVelocity`

- Switch units from `RigidBody::Dynamic` to `RigidBody::Kinematic`

- Replace `LinearVelocity::ZERO` with `AdjustedVelocity::default()` in `spawn_unit`

- Register type, update dev_tools debug viz

  

3. **Add `resolve_overlaps` system** (hard positional safety net)

- Pairwise overlap check using visual `UNIT_RADIUS`

- Asymmetric push: moving units get pushed, stationary stay planted

- Chain after `apply_movement`

  

4. **Tune ORCA constants**

- `AVOIDANCE_RADIUS = UNIT_RADIUS * 2.0`

- `DEFAULT_TIME_HORIZON = 5.0`, `static_time_horizon = 0.5`

- `DEFAULT_VELOCITY_SMOOTHING = 1.0`

- Add `static_time_horizon` to `AvoidanceConfig`

  

5. **Add dynamic ORCA responsibility**

- Add `&TargetingState` to `compute_avoidance` query

- `ENGAGING_RESPONSIBILITY = 0.25`, `MOVING_RESPONSIBILITY = 0.75`

- Override `AvoidanceAgent.responsibility` with dynamic value in snapshot

  

6. **Add same-team ORCA filtering**

- Add `&Team` to `compute_avoidance` query

- Include `Team` in snapshot tuple

- Skip ORCA constraints for opposing-team neighbors

  

7. **Add engager cap**

- `MAX_ENGAGERS_PER_UNIT_TARGET = 12` constant

- Engager counting HashMap in `find_target`

- Pass cap to `find_nearest_target` / `search_radius`

- `enforce_engager_cap` system after `find_target`

- Chain: `rebuild_target_grid → find_target → enforce_engager_cap → verify_targets`

  

8. **Add boids separation with tangential push**

- `apply_separation` system between `rebuild_spatial_hash` and `compute_avoidance`

- Same-team only, tangential for shared targets, radial otherwise

- Add `&TargetingState` and `&Team` to query

- Final movement chain: 6 systems with `chain_ignore_deferred()`

  

9. **Simplify movement.rs**

- Remove any engagement slot logic

- Engaging branch: direct steer toward target, stop at attack range

- `verify_targets`: simple range check, no slot proximity gate

  

### Core data flow pipeline

  

```

┌─────────────────┐

│ unit_movement │ Reads: TargetingState, flow field, target positions

│ │ Writes: PreferredVelocity (what the unit WANTS to do)

└────────┬────────┘

▼

┌─────────────────┐

│ rebuild_spatial │ Rebuilds AvoidanceSpatialHash with all unit positions

│ _hash │

└────────┬────────┘

▼

┌─────────────────┐

│ apply_separation │ Reads: PreferredVelocity, spatial hash, Team, TargetingState

│ │ Modifies: PreferredVelocity (adds same-team boids repulsion)

│ │ Tangential push for shared targets, radial for others

└────────┬────────┘

▼

┌─────────────────┐

│compute_avoidance │ Reads: PreferredVelocity, AdjustedVelocity (prev frame), Team

│ │ Writes: AdjustedVelocity (ORCA-corrected, same-team only)

│ │ Stationary units (Attacking) → AdjustedVelocity = ZERO

└────────┬────────┘

▼

┌─────────────────┐

│ apply_movement │ Reads: AdjustedVelocity

│ │ Writes: Transform.translation += velocity * dt

└────────┬────────┘

▼

┌─────────────────┐

│resolve_overlaps │ Reads: Transform, AdjustedVelocity (to know who's moving)

│ │ Writes: Transform (positional push, ALL teams)

│ │ Uses visual UNIT_RADIUS, not inflated AVOIDANCE_RADIUS

└─────────────────┘

```

  

### TargetingState → PreferredVelocity mapping (in unit_movement)

  

| State | PreferredVelocity | Notes |

|-------|-------------------|-------|

| `Moving` | flow field direction × speed | Follow assigned goal (enemy/player fortress) |

| `Seeking` | flow field direction × speed | Same as Moving for velocity purposes |

| `Engaging(target)` | direction toward target × speed | Direct steer. Zero if already in attack range |

| `Attacking(target)` | `Vec2::ZERO` | Stay planted. Never move during combat |

  

### Two separate spatial hashes

  

| Hash | Entity type | Used by | Cell size |

|------|-------------|---------|-----------|

| `TargetSpatialHash` | `With<Target>` (units, buildings, fortresses) | `find_target` in AI | `CELL_SIZE` (64px) |

| `AvoidanceSpatialHash` | `With<Unit>` | `compute_avoidance`, `apply_separation` | `CELL_SIZE` (64px) |

  

Both are rebuilt every frame. They contain different entity sets.

  

### Key query signatures (get these right first try)

  

```rust

// compute_avoidance

Query<(Entity, &GlobalTransform, &mut AdjustedVelocity, &PreferredVelocity,

&AvoidanceAgent, &Movement, &TargetingState, &Team), With<Unit>>

  

// apply_separation

Query<(Entity, &GlobalTransform, &mut PreferredVelocity, &Movement,

&TargetingState, &Team), With<Unit>>

  

// apply_movement

Query<(&AdjustedVelocity, &mut Transform), With<Unit>>

  

// resolve_overlaps

Query<(Entity, &mut Transform, &AvoidanceAgent, &AdjustedVelocity), With<Unit>>

  

// unit_movement

Query<(&TargetingState, &Movement, &CombatStats, &GlobalTransform,

&EntityExtent, &AssignedGoal, &mut PreferredVelocity), With<Unit>>

  

// enforce_engager_cap

Query<(Entity, &GlobalTransform, &mut TargetingState, Option<&Movement>)>

```

  

### Edge cases and guard conditions

  

- **Zero-length vectors**: Always check `length_squared() > f32::EPSILON` before normalizing. Appears in: separation push direction, engaging steer direction, tangent computation, ORCA relative position

- **Despawned targets mid-frame**: `targets.get(entity)` returns `Err` → skip with `continue`. Death observer handles state transitions separately

- **All targets at cap**: `find_nearest_target` returns `None` → unit stays Moving/Seeking, will check again next frame

- **Co-located agents**: ORCA overlap branch uses fallback direction `Vec2::new(1.0, 0.0)` when relative position is zero

- **Self in spatial hash**: Always skip `if neighbor_entity == *entity` in neighbor loops

- **Stationary units in separation**: Check `preferred.0.length_squared() < f32::EPSILON` before applying — don't push attacking units

  

### Interaction with existing systems

  

| System | Interaction | Notes |

|--------|-------------|-------|

| **Flow field** | `unit_movement` reads `GoalRegistry` for Moving/Seeking states | No changes needed to flow field |

| **Death observer** | Transitions orphaned units to Moving/Seeking when target dies | Runs independently in `GameSet::Death`, no conflict |

| **Combat/Attack** | Reads `TargetingState::Attacking(target)` to fire projectiles | No changes needed — still works with TargetingState |

| **Projectile hits** | Uses `Collider` for hit detection | Still needs avian2d Collider, hence RigidBody::Kinematic kept |

| **verify_targets** | Transitions Engaging→Attacking based on range check | Simplified: no slot proximity gate, just `distance <= stats.range` |

| **Retarget timer** | Staggered re-evaluation of existing targets | Unchanged — engager cap is checked during retarget |

  

### Performance characteristics

  

| System | Complexity | Concern | Notes |

|--------|-----------|---------|-------|

| `rebuild_spatial_hash` | O(n) | None | Runs every frame, very fast |

| `apply_separation` | O(n × k) | Low | k = avg neighbors per unit (bounded by separation radius) |

| `compute_avoidance` | O(n × m) | Low | m = max_neighbors (capped at 10) |

| `apply_movement` | O(n) | None | Trivial multiply + add |

| `resolve_overlaps` | **O(n²)** | **High** | Pairwise check, no spatial hash. OK for ~50 units, will degrade at 200+ |

| `enforce_engager_cap` | O(n log n) | Low | Sort per target group, usually small groups |

| `apply_separation` snapshots | O(n) memory | Low | Vec + HashMap allocation every frame |

  

`resolve_overlaps` is the bottleneck. Future improvement: use `AvoidanceSpatialHash` for neighbor lookup instead of all-pairs.

  

### DO NOT TRY (anti-patterns that failed)

  

1. **DO NOT let attacking units participate in ORCA** — they'll slide out of attack range, re-engage, walk back, oscillate forever. Attacking units MUST have `PreferredVelocity = ZERO` and be skipped by ORCA.

  

2. **DO NOT use engagement slots with 5px attack range** — ring geometry doesn't work. ~7 positions fit in the first ring of a unit-sized target. Grid BFS also fails because cell size doesn't align with the narrow attack ring. Direct steering + cap is simpler and works better.

  

3. **DO NOT apply separation/ORCA between opposing teams** — units will avoid enemies instead of fighting them. Walking past, fleeing, never closing to attack range. Avoidance is SAME-TEAM ONLY.

  

4. **DO NOT use radial separation for units sharing a target** — pushes units backwards away from target. Use tangential (perpendicular to target→unit line) so they slide AROUND the target.

  

5. **DO NOT count the current unit toward its own engager cap** — causes every unit to drop its target every retarget cycle. Always exclude `current_target` from the cap check.

  

6. **DO NOT apply engager cap in only one branch of find_target** — if Seeking checks the cap but retargeting doesn't, units retarget TO full targets, get kicked, retarget again → oscillation. Apply cap in ALL branches.

  

7. **DO NOT use `resolve_overlaps` with `AVOIDANCE_RADIUS`** — inflated radius pushes units apart to 24px, which looks wrong. Use visual `UNIT_RADIUS` (6px) so units can stand side-by-side naturally.

  

8. **DO NOT add velocity smoothing** — ORCA corrections must apply immediately. Smoothing (blend factor < 1.0) dilutes corrections and allows overlap at close range.

  

## Future Improvements

  

- **Iterative resolve_overlaps** — multiple passes for better convergence in dense groups

- **Spatial hash for resolve_overlaps** — use AvoidanceSpatialHash instead of O(n²) all-pairs

- **Increase attack range** — 5px is extremely tight, 15-20px would help spread naturally

- **Separation/ORCA tuning** — constants may need rebalancing with more unit types

- **Clean up engagement.rs** — dead code, should be deleted or feature-gated

- **Cross-team resolve_overlaps tuning** — may want asymmetric behavior (engaging unit pushes through, stationary enemy stays)

- **Consider removing avian2d dependency entirely** — if projectile collisions move to spatial hash queries, avian2d may not be needed at all