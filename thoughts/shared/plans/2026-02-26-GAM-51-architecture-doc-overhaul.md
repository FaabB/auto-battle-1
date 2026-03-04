# ARCHITECTURE.md Overhaul Implementation Plan

## Overview

Bring `ARCHITECTURE.md` fully up to date with the current codebase. The 5-agent architecture review found 40+ discrepancies and 20 undocumented patterns. This is a **documentation-only** ticket — no code changes, only ARCHITECTURE.md edits.

## Current State Analysis

The current ARCHITECTURE.md (723 lines) was last overhauled during the foxtrot guidelines migration (2026-02-14). Since then, significant features have been added (combat, economy, AI, avoidance, HUD, third-party integrations, endgame detection) without corresponding doc updates.

### Key Discoveries:
- Module tree is stale — 5+ modules missing, 2 paths wrong, 2 descriptions outdated
- State tables incomplete — `Victory`/`Defeat` menu variants missing
- Z-layer table has phantom `Z_HEALTH_BAR` and stale "(future)" markers
- Third-party section says "not needed yet" despite existing `third_party/` module
- Testing helpers table lists 6 of 14 functions
- Cargo.toml section missing 3 lint entries, full dep table, release profile
- 11 codebase patterns are undocumented
- `main.rs` return type documented wrong in examples

## Desired End State

Every section of ARCHITECTURE.md reflects the current codebase with:
- Accurate module tree showing all 30+ source files
- Complete state and Z-layer tables
- Full third-party documentation
- 11 new pattern sections with code examples and file:line references
- Complete testing helpers table (14 entries)
- Accurate Cargo.toml conventions

### How to verify:
- Run `make check` to ensure no build breakage (doc-only, should be no-op)
- Manual review: every module tree entry matches a real file path
- Manual review: every Z-layer constant matches `lib.rs`
- Manual review: every test helper matches `testing.rs`

## What We're NOT Doing

- No code changes — only ARCHITECTURE.md edits
- No restructuring of the document's major sections — keep the existing heading hierarchy
- No adding speculative/aspirational sections (e.g., "future audio system")
- No changes to CLAUDE.md — it references ARCHITECTURE.md correctly

## Implementation Approach

Three phases: (1) fix all existing sections that have errors, (2) add new pattern documentation sections, (3) final consistency verification. Each phase modifies only ARCHITECTURE.md.

---

## Phase 1: Fix Existing Sections

### Overview
Correct all inaccuracies in current sections. This is the bulk of the work — module tree, state tables, Z-layers, third-party, testing, Cargo.toml, and misc fixes.

### Changes Required:

#### 1. Module Tree (lines 55–104)
**File**: `ARCHITECTURE.md`
**Changes**: Replace the entire module tree with the accurate current structure.

Current tree is missing:
- `gameplay/ai.rs` (currently wrongly listed under `units/ai.rs`)
- `gameplay/endgame_detection.rs` (listed as `endgame.rs`)
- `gameplay/hud/` directory (3 files: `mod.rs`, `bottom_bar.rs`, `elapsed_time.rs`)
- `units/avoidance/` directory (3 files: `mod.rs`, `orca.rs`, `spatial_hash.rs`)
- `third_party/` directory (not shown in tree, despite existing section below)
- `theme/interaction.rs` (listed as "(future)" but exists)
- `units/pathfinding.rs` description needs update

Replace with:

```
src/
├── main.rs              # App assembly only (DefaultPlugins + auto_battle::plugin)
├── lib.rs               # Z-layer constants, GameSet, gameplay_running(), top-level compositor
├── testing.rs           # Test helpers (#[cfg(test)])
├── ui_camera.rs         # Global UI camera that persists across all states
├── screens/             # Screen state management
│   ├── mod.rs           # GameState enum (Loading, MainMenu, InGame)
│   ├── loading.rs       # Loading screen
│   ├── main_menu.rs     # MainMenu → opens Menu::Main
│   └── in_game.rs       # InGame → ESC opens Menu::Pause
├── menus/               # Menu overlay state and UI
│   ├── mod.rs           # Menu enum (None, Main, Pause, Victory, Defeat) + virtual time pause
│   ├── main_menu.rs     # Main menu UI and input
│   ├── pause.rs         # Pause menu UI and input
│   └── endgame.rs       # Victory/Defeat overlay UI and input
├── gameplay/            # Cross-cutting components + compositor for domain plugins
│   ├── mod.rs           # Team, Health, Target, CurrentTarget, Movement, CombatStats + entity archetype docs
│   ├── ai.rs            # Staggered target finding and retargeting (RetargetTimer)
│   ├── endgame_detection.rs  # Victory/defeat detection (fortress health checks)
│   ├── battlefield/     # Grid layout, zones, camera panning, rendering
│   │   ├── mod.rs       # Grid constants, fortress markers, BattlefieldSetup set, GridIndex
│   │   ├── camera.rs    # Camera setup and panning
│   │   └── renderer.rs  # Zone backdrops, fortress/grid/navmesh spawning
│   ├── building/        # Placement systems, grid cursor, building components
│   │   ├── mod.rs       # Building, BuildingType, BuildingStats, building_stats(), observer
│   │   ├── placement.rs # Grid cursor tracking and click-to-place
│   │   └── production.rs# Barracks unit spawning on timer
│   ├── combat/          # Attack, death, health bars
│   │   ├── mod.rs       # Compositor + re-exports (AttackTimer, Hitbox, DeathCheck, HealthBarConfig)
│   │   ├── attack.rs    # Projectile spawning, movement, and hit detection
│   │   ├── death.rs     # DeathCheck SystemSet + despawn dead entities
│   │   └── health_bar.rs# Health bar spawning and updates
│   ├── economy/         # Gold, shop, income, UI
│   │   ├── mod.rs       # Gold resource, building costs, compositor
│   │   ├── income.rs    # Farm income + kill rewards
│   │   ├── shop.rs      # Shop logic (cards, reroll, selection)
│   │   ├── shop_ui.rs   # Shop panel UI (card buttons, reroll button)
│   │   └── ui.rs        # Gold HUD display
│   ├── hud/             # In-game HUD elements
│   │   ├── mod.rs       # HUD plugin compositor
│   │   ├── bottom_bar.rs# Bottom UI bar layout
│   │   └── elapsed_time.rs # Game timer display
│   └── units/           # Unit components, AI, movement, spawning
│       ├── mod.rs       # Unit, UnitType, UnitStats, unit_stats(), UnitAssets, spawn_unit()
│       ├── spawn.rs     # Enemy spawning with ramping difficulty
│       ├── movement.rs  # Unit movement toward targets (preferred velocity)
│       ├── pathfinding.rs # NavPath component and navmesh path computation
│       └── avoidance/   # ORCA local avoidance
│           ├── mod.rs   # PreferredVelocity, AvoidanceAgent, AvoidanceConfig
│           ├── orca.rs  # ORCA velocity obstacle algorithm
│           └── spatial_hash.rs # Spatial hash for neighbor lookup
├── theme/               # Shared color palette and UI widget constructors
│   ├── mod.rs           # Theme plugin compositor
│   ├── palette.rs       # Color constants + font size tokens
│   ├── interaction.rs   # Button hover/press feedback using observers
│   └── widget.rs        # Reusable widget constructors (header, label, overlay, button)
├── third_party/         # Third-party plugin isolation
│   ├── mod.rs           # Compositor + re-exports (CollisionLayer, NavObstacle, surface_distance)
│   ├── avian.rs         # Avian2d physics: CollisionLayer, solid_entity_layers(), surface_distance()
│   └── vleue_navigator.rs # vleue_navigator: NavObstacle, navmesh updater, cleanup on exit
└── dev_tools/           # Debug-only tools (feature-gated on `dev`)
    └── mod.rs           # World inspector (F4), navmesh debug overlay (F3), avoidance gizmos
```

#### 2. State & Z-Layer Tables

**Menu table** (line 168): Add Victory, Defeat variants and update the table:

```markdown
| State | Defined in | Purpose | Variants |
|-------|-----------|---------|----------|
| `GameState` | `screens/mod.rs` | Which screen is active | `Loading`, `MainMenu`, `InGame` |
| `Menu` | `menus/mod.rs` | Which menu overlay is shown | `None`, `Main`, `Pause`, `Victory`, `Defeat` |
```

**Add dual-layer pause documentation** after the Menu table (after line ~172):

```markdown
### Pause mechanism (dual-layer)

Pausing uses two complementary mechanisms:

1. **`run_if` gate**: `gameplay_running()` checks `in_state(GameState::InGame).and(in_state(Menu::None))`. When any menu is open, gameplay systems simply don't run.
2. **Virtual time pause**: `OnExit(Menu::None)` calls `time.pause()`, `OnEnter(Menu::None)` calls `time.unpause()`. This freezes `Time<Virtual>` so physics, timers, and animations also stop.

Both layers are needed — `run_if` prevents system execution, virtual time prevents third-party plugins (physics, navmesh) from advancing.
```

**Z-layer table** (lines 586–595): Replace with accurate values. Remove phantom `Z_HEALTH_BAR`, add `Z_FORTRESS` and `Z_PROJECTILE`, remove "(future)" markers:

```markdown
| Constant | Value | Purpose |
|----------|-------|---------|
| `Z_BACKGROUND` | -1.0 | Background fill |
| `Z_ZONE` | 0.0 | Fortress and zone backdrop sprites |
| `Z_FORTRESS` | 0.5 | Fortress entities (above zone backdrops) |
| `Z_GRID` | 1.0 | Build zone grid cells |
| `Z_GRID_CURSOR` | 2.0 | Hover highlight |
| `Z_BUILDING` | 3.0 | Placed buildings |
| `Z_UNIT` | 4.0 | Units |
| `Z_PROJECTILE` | 4.5 | Projectiles (above units) |
```

#### 3. Third-Party Section (lines 466–514)

Remove the "Not needed yet" paragraph (line 484) and update the introductory text. The section already has good CollisionLayer and surface_distance docs — keep those.

**Changes**:
- Remove: "Not needed yet for auto-battle-1 (no complex third-party deps beyond Bevy itself). Create when needed."
- Update intro to reflect that `third_party/` exists with avian and vleue_navigator wrappers
- Add vleue_navigator subsection after the existing avian/collision docs:

```markdown
### vleue_navigator Integration (`third_party/vleue_navigator.rs`)

Wraps the `vleue_navigator` crate for navmesh pathfinding.

#### `NavObstacle` marker component

Static obstacles (buildings, fortresses) that should carve holes in the navmesh:

```rust
use crate::third_party::NavObstacle;

commands.spawn((
    Name::new("Player Fortress"),
    PlayerFortress,
    NavObstacle,  // Tells navmesh updater to carve around this entity
    RigidBody::Static,
    Collider::rectangle(w, h),
    // ...
));
```

The `NavmeshUpdaterPlugin<Collider, NavObstacle>` automatically rebuilds the navmesh when `NavObstacle` entities are added or removed.

#### Cleanup on state exit

`NavObstacle` components must be stripped **before** `DespawnOnExit` batch-despawns entities, otherwise late navmesh rebuild tasks cause hangs on exit:

```rust
// In vleue_navigator.rs plugin:
app.add_systems(OnExit(GameState::InGame), strip_nav_obstacles_before_despawn);
```

This is the same pattern used by `Building` observer cleanup (see Observer Safety section).
```

#### 4. Testing Helpers Table (lines 522–530)

Replace the 6-entry table with the full 14-entry table:

```markdown
| Helper | Description |
|--------|-------------|
| `create_test_app()` | Bare `MinimalPlugins` app |
| `create_test_app_with_state::<S>()` | `MinimalPlugins` + one generic state |
| `create_base_test_app()` | States + InputPlugin + WindowPlugin + Camera2d |
| `create_base_test_app_no_input()` | States + WindowPlugin + Camera2d (no InputPlugin) |
| `transition_to_ingame(app)` | Sets `GameState::InGame` and runs two updates |
| `count_entities::<F>(app)` | Count entities matching a query filter |
| `assert_entity_count::<F>(app, n)` | Assert exactly N entities match a filter |
| `tick_multiple(app, count)` | Run `app.update()` N times |
| `nearly_expire_timer(timer)` | Set elapsed to `duration - 1ns` for guaranteed `just_finished()` |
| `init_asset_resources(app)` | Init `Assets<Mesh>` + `Assets<ColorMaterial>` |
| `init_economy_resources(app)` | Init `Gold` + `Shop` resources |
| `init_input_resources(app)` | Init `ButtonInput<KeyCode>` + `ButtonInput<MouseButton>` |
| `spawn_test_unit(world, team, x, y)` | Spawn full Soldier archetype with all components |
| `spawn_test_target(world, team, x, y)` | Spawn minimal targetable entity (Team + Target + Collider) |
```

#### 5. Cargo.toml Section (lines 599–628)

Update the lints subsection to include all current lints:

```markdown
### Lints

```toml
[lints.rust]
unsafe_code = "forbid"

[lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
nursery = { level = "warn", priority = -1 }
needless_pass_by_value = "allow"  # Bevy systems take ownership
too_many_arguments = "allow"      # Bevy queries can have many params
type_complexity = "allow"         # Bevy queries can be complex
```
```

Update the Dependencies subsection to list all current deps:

```markdown
### Dependencies

```toml
bevy = { version = "0.18", default-features = false, features = ["2d"] }
avian2d = { version = "0.5", default-features = false, features = ["2d", "parry-f32", "debug-plugin", "parallel"] }
vleue_navigator = { version = "0.15", default-features = false, features = ["avian2d"] }
rand = "0.9"
bevy-inspector-egui = { version = "0.36", optional = true }  # gated on `dev` feature
```
```

Add release profile:

```markdown
### Build Profiles

```toml
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3

[profile.release]
lto = "thin"
codegen-units = 1
```
```

Update features section to include bevy-inspector-egui:

```markdown
### Features

```toml
[features]
default = ["dev"]
dev = ["bevy/dynamic_linking", "vleue_navigator/debug-with-gizmos", "dep:bevy-inspector-egui"]
```
```

#### 6. Debug Keybindings Table (line ~460)

Add F4:

```markdown
| Key | Action | Details |
|-----|--------|---------|
| F3 | Toggle navmesh debug overlay | Shows red navmesh triangulation + yellow unit path lines + green/cyan avoidance vectors. Off by default. |
| F4 | Toggle world inspector | Shows bevy-inspector-egui entity/component browser. Off by default. |
```

#### 7. App Assembly / main.rs (line ~120)

Fix the `main.rs` example to show `fn main()` returning `()` (current code), not `AppExit`:

```rust
fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin { ... })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(auto_battle::plugin)
        .run();
}
```

(The current doc already shows this correctly — verify and leave as-is if correct.)

#### 8. Theme Section (lines 269–275)

Remove "(future)" from `interaction.rs`:

```markdown
- `theme/palette.rs` -- color constants + font size tokens
- `theme/interaction.rs` -- button hover/press feedback using observers
- `theme/widget.rs` -- reusable widget constructors (header, label, overlay, button)
```

Remove the "Button pattern (future)" subsection (lines 297–314) since buttons are now implemented in `interaction.rs` and `widget.rs`.

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes (no code changes, should be trivial)

#### Manual Verification:
- [ ] Module tree: every entry in the tree corresponds to a real file
- [ ] Module tree: every real file under `src/` appears in the tree
- [ ] Z-layer table matches constants in `lib.rs`
- [ ] Menu table matches `Menu` enum in `menus/mod.rs`
- [ ] Testing helpers table matches all `pub fn` in `testing.rs`
- [ ] Cargo.toml section matches actual `Cargo.toml`
- [ ] F3/F4 keybindings match `dev_tools/mod.rs`

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 2: Add New Pattern Sections

### Overview
Add 11 new documentation sections covering patterns discovered by the architecture review. Each section includes a brief description and 1-2 code examples with file:line references.

### Changes Required:

All new sections are added to ARCHITECTURE.md. Insert them in logical locations within the existing document structure.

#### 1. Data-Driven Stats Lookup (insert after Component Co-Location section, ~line 264)

```markdown
## Data-Driven Stats Lookup

Game entity stats use `const fn` match expressions instead of trait implementations:

```rust
// gameplay/building/mod.rs:64
pub const fn building_stats(building_type: BuildingType) -> BuildingStats {
    match building_type {
        BuildingType::Barracks => BuildingStats { hp: 300.0, cost: 100, ... },
        BuildingType::Farm => BuildingStats { hp: 150.0, cost: 50, ... },
    }
}
```

Same pattern in `gameplay/units/mod.rs:60` with `unit_stats()`. Benefits:
- **Compile-time evaluation** — stats are constants, no runtime overhead
- **Exhaustive matching** — adding a new variant forces updating all stat lookups
- **Single source of truth** — one function per entity category, no inheritance

Convenience delegates (e.g., `building_hp()`, `building_color()`) call through to the main stats function.
```

#### 2. Entity Archetypes (insert after Data-Driven Stats section)

```markdown
## Entity Archetypes

Each entity type has a canonical component bundle documented in `gameplay/mod.rs` and a single spawn function:

| Entity | Spawn Location | Key Components |
|--------|---------------|----------------|
| Unit | `units/mod.rs:spawn_unit()` | `Unit`, `UnitType`, `Team`, `Target`, `CurrentTarget`, `Health`, `CombatStats`, `Movement`, `AttackTimer`, `Mesh2d`, `RigidBody::Dynamic`, `Collider`, `PreferredVelocity`, `AvoidanceAgent`, `NavPath` |
| Building | `building/placement.rs` | `Building`, `BuildingType`, `Team`, `Target`, `Health`, `ProductionTimer`/`IncomeTimer`, `RigidBody::Static`, `Collider`, `NavObstacle` |
| Fortress | `battlefield/renderer.rs` | `PlayerFortress`/`EnemyFortress`, `Team`, `Target`, `CurrentTarget`, `Health`, `CombatStats`, `AttackTimer`, `RigidBody::Static`, `Collider`, `NavObstacle` |
| Projectile | `combat/attack.rs` | `Projectile`, `Team`, `Hitbox`, `Sensor`, `RigidBody::Kinematic`, `Collider`, `CollidingEntities` |

The doc comment at `gameplay/mod.rs:3-17` serves as the canonical archetype reference.
```

#### 3. `chain` vs `chain_ignore_deferred` (insert after Global System Ordering section, ~line 348)

```markdown
### `chain()` vs `chain_ignore_deferred()`

In Bevy 0.18, `.chain()` auto-inserts `ApplyDeferred` between chained systems. Use `.chain_ignore_deferred()` when you **don't** want deferred commands to flush between systems.

| Method | Behavior | When to use |
|--------|----------|-------------|
| `.chain()` | Inserts `ApplyDeferred` between each pair | Systems that spawn entities needed by later systems in the chain |
| `.chain_ignore_deferred()` | Pure ordering, no flush | Systems that share queries but shouldn't see each other's spawns yet |

**Examples in this codebase:**

- `.chain()` in `battlefield/mod.rs:199` — `spawn_battlefield` then `setup_camera_for_battlefield` (camera needs battlefield entities)
- `.chain_ignore_deferred()` in `combat/attack.rs:189` — `attack` → `move_projectiles` → `handle_projectile_hits` (newly spawned projectiles shouldn't move until next frame)
- `.chain_ignore_deferred()` in `building/mod.rs:217` — `update_grid_cursor` → `handle_building_placement` (cursor position read, not entity spawns)
- `.chain_ignore_deferred()` in `units/mod.rs:238` — `unit_movement` → `rebuild_spatial_hash` → `compute_avoidance` (avoidance pipeline, no intermediate spawns)
```

#### 4. Observer Safety During State Transitions (insert after existing Observers section, ~line 440)

```markdown
### Observer safety during state transitions

When `DespawnOnExit` batch-despawns entities, `On<Remove, T>` observers fire for each component removal. If the observer queries other entities (e.g., grid slots), those entities may already be despawned.

**Pattern: strip markers before batch despawn**

```rust
// building/mod.rs:184 — OnExit system strips Building markers first
fn strip_buildings_before_despawn(
    mut commands: Commands,
    buildings: Query<Entity, With<Building>>,
) {
    for entity in &buildings {
        commands.entity(entity).remove::<Building>();
    }
}
```

This fires the `On<Remove, Building>` observer while slot entities still exist. Same pattern in `third_party/vleue_navigator.rs` for `NavObstacle` cleanup.

**Always guard observer queries** with `Query::get()`:

```rust
fn clear_build_slot_on_building_removed(
    remove: On<Remove, Building>,
    buildings: Query<&Building>,
    // ...
) {
    let Ok(building) = buildings.get(remove.entity) else { return };
    // Safe: building data still accessible during Remove
}
```
```

#### 5. Cross-Plugin OnEnter Ordering (insert after Observer Safety section)

```markdown
### Cross-plugin `OnEnter` ordering

When multiple plugins register `OnEnter(GameState::InGame)` systems, use `SystemSet` markers to control ordering:

```rust
// battlefield/mod.rs:184 — defines a set
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BattlefieldSetup;

// building/mod.rs:213 — orders after it
app.add_systems(
    OnEnter(GameState::InGame),
    spawn_grid_cursor.after(BattlefieldSetup),
);
```

Same pattern with `DeathCheck` in `combat/death.rs:10` — other systems can order `.before(DeathCheck)` instead of referencing private system functions.

**Rule**: if another plugin needs to order relative to your `OnEnter`/`Update` system, export a `SystemSet` marker instead of making the system `pub`.
```

#### 6. `Single` vs `Query` (insert after Naming Conventions section or as a subsection of a new "System Parameter Patterns" section)

```markdown
## System Parameter Patterns

### `Single<>` vs `Query<>`

`Single<D, F>` is for exactly-one-entity queries. If 0 or >1 entities match, the system is **silently skipped** (no panic).

| Use `Single<>` when | Use `Query<>` when |
|---------------------|-------------------|
| Exactly one entity expected (camera, window, fortress) | Zero or many entities (units, buildings, projectiles) |
| System should no-op if entity is missing | System must handle empty/multiple results explicitly |

**`Single` as graceful degradation** (`units/spawn.rs:84`):

```rust
fn tick_enemy_spawner(
    enemy_fortress: Single<&Transform, With<EnemyFortress>>,
    // If fortress is destroyed (despawned), system silently stops running
    // → no more enemy spawns. No explicit "is fortress alive?" check needed.
) { ... }
```
```

#### 7. `Option<Res<T>>` (add as subsection under System Parameter Patterns)

```markdown
### `Option<Res<T>>`

Use `Option<Res<T>>` when a resource may not exist yet or is conditionally inserted:

```rust
// units/mod.rs:191 — prevent handle leaks on state re-entry
fn setup_unit_assets(
    existing: Option<Res<UnitAssets>>,
    // ...
) {
    if existing.is_some() { return; }
    // Create assets only on first entry
}

// dev_tools/mod.rs:41 — toggle resource presence
fn toggle_world_inspector(
    existing: Option<Res<ShowWorldInspector>>,
    // ...
) {
    if existing.is_some() { commands.remove_resource::<ShowWorldInspector>(); }
    else { commands.insert_resource(ShowWorldInspector); }
}
```

Also used for third-party resources that load asynchronously (`Option<Res<Assets<NavMesh>>>` in `units/spawn.rs`).
```

#### 8. `Name::new()` Convention (add as subsection under Naming Conventions)

```markdown
### `Name::new()` convention

All spawned entities include a `Name` component for inspector visibility:

| Pattern | When | Example |
|---------|------|---------|
| Static string | Singletons, unique entities | `Name::new("Player Fortress")` |
| `format!()` | Multiple instances | `Name::new(format!("{team:?} {}", unit_type.display_name()))` |
| Coordinate-based | Grid entities | `Name::new(format!("Build Slot ({col}, {row}"))` |

This makes the F4 world inspector useful for debugging. Always add `Name` when spawning entities, even data-only ones.
```

#### 9. Per-Domain Test App Factories (add as subsection under Testing Patterns)

```markdown
### Per-domain test app factories

Each domain module defines a local `create_*_test_app()` inside its `#[cfg(test)]` block:

```rust
// gameplay/ai.rs — minimal app for AI tests
fn create_ai_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<RetargetTimer>();
    app.add_systems(Update, find_target);
    app
}

// gameplay/battlefield/mod.rs — full state-based app
fn create_battlefield_test_app() -> App {
    let mut app = crate::testing::create_base_test_app();
    app.add_plugins(plugin);
    crate::testing::transition_to_ingame(&mut app);
    app
}
```

**Convention**: use the shared `testing.rs` helpers as building blocks, then add only the specific plugin/systems under test. Register only what's needed — avoid loading unrelated plugins that might overwrite test data.
```

#### 10. Test Module Organization (add as subsection under Testing Patterns)

```markdown
### Test module organization

Tests live **inline** in the same file as production code, using `#[cfg(test)]`:

```rust
// At bottom of domain_module.rs:
#[cfg(test)]
mod tests {
    use super::*;
    // Pure function tests — no App, no Bevy systems
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    // App-based tests using create_*_test_app()
}
```

Split into `mod tests` (pure/unit) and `mod integration_tests` (App-based) when both kinds exist. Named test modules (e.g., `mod observer_tests`) are fine when testing a specific subsystem.

The top-level `tests/` directory is reserved for full-stack integration tests that use `auto_battle::plugin` directly (e.g., state transition tests in `tests/integration/state_transitions.rs`).
```

#### 11. Debug Toggle Pattern (add as subsection under Dev Tools section)

```markdown
### Debug toggle pattern

Debug overlays use a marker resource whose presence/absence controls visibility:

```rust
// 1. Define marker resource
#[derive(Resource)]
struct ShowWorldInspector;

// 2. Toggle system: insert/remove on keypress
fn toggle_world_inspector(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    existing: Option<Res<ShowWorldInspector>>,
) {
    if input.just_pressed(KeyCode::F4) {
        if existing.is_some() {
            commands.remove_resource::<ShowWorldInspector>();
        } else {
            commands.insert_resource(ShowWorldInspector);
        }
    }
}

// 3. Gate the overlay system on resource existence
app.add_systems(Update, render_inspector.run_if(resource_exists::<ShowWorldInspector>));
```

This pattern is used for both F3 (navmesh debug) and F4 (world inspector). The `run_if(resource_exists::<T>)` condition means the gated systems have zero cost when the toggle is off.
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes

#### Manual Verification:
- [ ] All 11 new sections are present and formatted consistently
- [ ] Code examples in new sections compile conceptually (match actual codebase patterns)
- [ ] File:line references are accurate
- [ ] No duplicate coverage with existing sections

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 3: Final Consistency Pass

### Overview
Re-read the entire ARCHITECTURE.md and cross-check every claim against the actual codebase. Fix any remaining inconsistencies.

### Changes Required:

#### 1. Full document read-through
- Read the completed ARCHITECTURE.md top to bottom
- For each factual claim (file paths, line numbers, constant values, API signatures), verify against the source

#### 2. Specific checks
- Every `src/` path in the module tree → `ls` to confirm it exists
- Every constant value → grep `lib.rs`, `battlefield/mod.rs`, `economy/mod.rs`
- Every `GameSet` variant mentioned → match against `lib.rs` enum
- Every test helper → match against `testing.rs`
- No stale "(future)" markers remain
- No references to deleted/renamed files
- Button pattern section removed or updated (was "future", now implemented)
- `gameplay_running()` description accurate

#### 3. Line count check
- Ensure the document hasn't grown unreasonably (target: ~900-1000 lines, up from 723)
- If too long, look for sections that can be condensed

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes (237 unit + 2 integration tests)

#### Manual Verification:
- [ ] Every module tree entry maps to a real file
- [ ] Every code example matches actual codebase patterns
- [ ] Document reads coherently top-to-bottom
- [ ] No contradictions between sections
- [ ] No stale references to old file names or removed features

---

## Testing Strategy

This is documentation-only, so no new tests are needed.

### Automated:
- `make check` — ensures no accidental code changes
- `make test` — ensures existing tests still pass

### Manual:
- Side-by-side comparison of each ARCHITECTURE.md section with actual source files
- Verify new pattern sections are accurate by reading the referenced source files

## Performance Considerations

None — documentation-only change.

## References

- Linear ticket: [GAM-51](https://linear.app/tayhu-games/issue/GAM-51/architecturemd-overhaul-architecture-review-phase-2)
- Blocking ticket: [GAM-50](https://linear.app/tayhu-games/issue/GAM-50) (Bug fixes & quick wins — DONE)
- Current ARCHITECTURE.md: `ARCHITECTURE.md` (723 lines)
- Previous overhaul: `thoughts/shared/plans/2026-02-13-foxtrot-guidelines-migration.md`
