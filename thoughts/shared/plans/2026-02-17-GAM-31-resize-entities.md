# Resize Units, Buildings, and Shots — Implementation Plan

## Overview

Shrink units, buildings, and projectiles visually and physically (colliders + sprites) while keeping the 64px grid cell unchanged. This creates gaps between adjacent buildings so units can walk between building blocks.

## Current State Analysis

| Entity | Visual | Collider | Constant |
|--------|--------|----------|----------|
| Grid cell | 64px | — | `CELL_SIZE` |
| Unit | ø24px (r=12) | Circle r=12 | `UNIT_RADIUS` |
| Building | 60×60px | Rect 60×60 | `BUILDING_SPRITE_SIZE` |
| Fortress | 128×128px | Rect 128×128 | Derived from `FORTRESS_COLS × CELL_SIZE` |
| Projectile | ø6px (r=3) | Circle r=3 | `PROJECTILE_RADIUS` |

Adjacent buildings sit 64px center-to-center. With 60px buildings, the gap is only 4px — units (24px diameter) can't fit through.

### Key Discoveries:
- All visual sizes and colliders derive from the same constants, so changing the constant updates both sprite and physics automatically
- Surface-distance-based gameplay (targeting, range, movement) works correctly regardless of size — no system logic changes needed
- Health bars use separate constants (width, height, y_offset) that need proportional scaling
- Fortress stays at 128×128 (unchanged)

## Desired End State

| Entity | Visual | Collider | Gap between adjacent |
|--------|--------|----------|---------------------|
| Unit | ø12px (r=6) | Circle r=6 | — |
| Building | 40×40px | Rect 40×40 | 24px (unit fits with 12px clearance) |
| Projectile | ø4px (r=2) | Circle r=2 | — |
| Fortress | 128×128px | Rect 128×128 | (unchanged) |

Health bars scale proportionally. All tests pass, including `make check`.

### Verification:
- `make check` passes (clippy + build)
- `make test` passes (all unit + integration tests)
- Manual: run the game, confirm entities appear smaller, buildings show visible gaps, units can walk between buildings

## What We're NOT Doing

- Changing grid cell size (stays 64px)
- Changing fortress size (stays 128×128)
- Changing attack ranges, speeds, or other gameplay stats
- Adding pathfinding or explicit "walk between buildings" logic (physics handles it)
- Changing unit spawn positions or wave spawning

## Implementation Approach

Pure constant changes across 4 files, plus 1 test position fix. No system logic changes. The codebase already uses surface-distance for all gameplay interactions, so shrinking entities "just works".

## Phase 1: Resize Constants and Fix Tests

### Changes Required:

#### 1. Unit Size
**File**: `src/gameplay/units/mod.rs`
**Change**: Line 21

```rust
// Before
pub const UNIT_RADIUS: f32 = 12.0;

// After
pub const UNIT_RADIUS: f32 = 6.0;
```

This auto-updates:
- Visual mesh: `Circle::new(UNIT_RADIUS)` (line 160)
- Collider: `Collider::circle(UNIT_RADIUS)` (line 131)

#### 2. Unit Health Bar
**File**: `src/gameplay/combat/health_bar.rs`
**Changes**: Lines 15, 18, 21

```rust
// Before
pub const UNIT_HEALTH_BAR_WIDTH: f32 = 20.0;
pub const UNIT_HEALTH_BAR_HEIGHT: f32 = 3.0;
pub const UNIT_HEALTH_BAR_Y_OFFSET: f32 = 18.0;

// After (proportional to unit size halving)
pub const UNIT_HEALTH_BAR_WIDTH: f32 = 10.0;
pub const UNIT_HEALTH_BAR_HEIGHT: f32 = 2.0;
pub const UNIT_HEALTH_BAR_Y_OFFSET: f32 = 10.0;
```

#### 3. Building Size and Health Bar
**File**: `src/gameplay/building/mod.rs`
**Changes**: Lines 19, 22, 25, 28

```rust
// Before
const BUILDING_SPRITE_SIZE: f32 = CELL_SIZE - 4.0;  // 60.0
const BUILDING_HEALTH_BAR_WIDTH: f32 = 40.0;
const BUILDING_HEALTH_BAR_HEIGHT: f32 = 4.0;
const BUILDING_HEALTH_BAR_Y_OFFSET: f32 = 36.0;

// After
const BUILDING_SPRITE_SIZE: f32 = 40.0;
const BUILDING_HEALTH_BAR_WIDTH: f32 = 28.0;
const BUILDING_HEALTH_BAR_HEIGHT: f32 = 3.0;
const BUILDING_HEALTH_BAR_Y_OFFSET: f32 = 26.0;
```

This auto-updates:
- Visual sprite: `Vec2::splat(BUILDING_SPRITE_SIZE)` (placement.rs:136)
- Collider: `Collider::rectangle(BUILDING_SPRITE_SIZE, BUILDING_SPRITE_SIZE)` (placement.rs:142)

#### 4. Projectile Size
**File**: `src/gameplay/combat/attack.rs`
**Change**: Line 17

```rust
// Before
const PROJECTILE_RADIUS: f32 = 3.0;

// After
const PROJECTILE_RADIUS: f32 = 2.0;
```

This auto-updates:
- Visual sprite: `Vec2::splat(PROJECTILE_RADIUS * 2.0)` (line 98)
- Collider: `Collider::circle(PROJECTILE_RADIUS)` (line 107)

#### 5. Fix Broken Test
**File**: `src/gameplay/combat/attack.rs`
**Change**: Test `unit_spawns_projectile_in_range` (line 292)

The test places attacker at x=100 and target at x=120 with `Collider::circle(5.0)`.
- **Before**: surface distance = 20 - 12 - 5 = 3 < attack_range(5) ✓
- **After**: surface distance = 20 - 6 - 5 = 9 > attack_range(5) ✗

Fix: move target closer so surface distance stays within range.

```rust
// Before
let target = spawn_target(app.world_mut(), 120.0, 100.0);
spawn_attacker(app.world_mut(), 100.0, Some(target)); // surface distance = 3 < range 5

// After
let target = spawn_target(app.world_mut(), 114.0, 100.0);
spawn_attacker(app.world_mut(), 100.0, Some(target)); // surface distance = 14 - 6 - 5 = 3 < range 5
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes (clippy + build)
- [ ] `make test` passes (all unit + integration tests)

#### Manual Verification:
- [ ] Run the game — units visibly smaller (12px diameter vs 24px)
- [ ] Buildings visibly smaller within their grid cells (40px in 64px cell)
- [ ] Visible gaps between adjacent buildings
- [ ] Projectiles still visible but smaller
- [ ] Health bars proportionally smaller and correctly positioned above entities
- [ ] Units can walk through gaps between adjacent buildings
- [ ] Fortress unchanged at 2×2 cell size

## Testing Strategy

### Existing Tests (auto-pass after constant changes):
- `movement.rs` — all 5 tests: positions are far enough that halved unit radius doesn't break range checks
- `attack.rs` — 8 of 9 tests pass unchanged (1 fixed above)
- `health_bar.rs` — tests use constant references, auto-update
- `building/placement.rs` — tests don't reference building pixel size
- `battlefield/mod.rs` — tests don't reference changed sizes (fortress unchanged)
- `avian.rs` — tests use hardcoded sizes for function testing, unaffected

### No New Tests Needed:
All existing tests cover the same gameplay behaviors at the new sizes. The resize is purely cosmetic + physics scaling — no new code paths to test.

## References

- Linear ticket: [GAM-31](https://linear.app/tayhu-games/issue/GAM-31/resize-units-buildings-shots)
