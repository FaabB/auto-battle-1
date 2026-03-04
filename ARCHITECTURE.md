# Architecture

This document describes the conventions and patterns used in the auto-battle-1 codebase. It serves as the authoritative reference for how code should be organized.

**Reference projects**: [foxtrot](https://github.com/janhohenheim/foxtrot) (3D), [bevy_new_2d](https://github.com/TheBevyFlock/bevy_new_2d) (2D template). Both by the Bevy community, targeting Bevy 0.18.

---

## Plugin Architecture

All plugins use the **function plugin** pattern:

```rust
pub(super) fn plugin(app: &mut App) {
    app.register_type::<MyComponent>()
        .init_resource::<MyResource>();

    app.add_systems(OnEnter(GameState::InGame), setup_stuff)
        .add_systems(
            Update,
            do_stuff
                .in_set(GameSet::Input)
                .run_if(crate::gameplay_running),
        );
}
```

The top-level compositor in `lib.rs` is the only `pub fn plugin` -- all others are `pub(super) fn plugin` (or `pub fn plugin` when inside a `pub(crate)` parent module, per clippy `redundant_pub_crate`).

### Why function plugins

Both foxtrot and bevy_new_2d use function plugins exclusively. Zero boilerplate (no `struct FooPlugin;` + `impl Plugin`), composes naturally as tuples (`app.add_plugins((a::plugin, b::plugin))`), and the function name is always `plugin` -- uniform and predictable.

### Registration order inside a plugin

1. Type registrations (`register_type`)
2. Resources (`init_resource`)
3. Sub-plugins (`add_plugins`)
4. Observers (when needed)
5. Systems (`add_systems`)

This matches foxtrot's convention and keeps each plugin function readable and consistent.

---

## Module Structure

| Pattern | When to use |
|---------|-------------|
| Flat file (`foo.rs`) | Small module (<300 lines), no submodules |
| Directory (`foo/mod.rs`) | Module has submodules or will grow past 300 lines |

### Current modules

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

### When to create a subdirectory

From foxtrot's pattern:
- **1 file** -- flat file (`src/animation.rs`)
- **2-3 related files** -- subdirectory with `mod.rs` (`src/audio/mod.rs` + `perceptual.rs`)
- **4+ files or sub-features** -- nested subdirectories (`src/gameplay/player/pickup/`)

---

## App Assembly

`main.rs` is the **composition root** -- the only place plugins are assembled into the app.

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

`lib.rs` holds the top-level `pub fn plugin` compositor plus cross-cutting shared types (Z-layers, `GameSet`). No game logic in either file.

This matches both foxtrot and bevy_new_2d, where `main.rs` only assembles the app and all logic lives in domain plugins.

---

## Visibility Rules

| Scope | Visibility | Example |
|-------|-----------|---------|
| Plugin functions | `pub(super)` or `pub` | `pub(super) fn plugin(app: &mut App)` |
| Components, resources, constants used cross-module | `pub` | `pub struct Health { ... }` |
| Systems (called only by their own plugin) | private (no `pub`) | `fn setup_camera(...)` |
| Systems called by parent `mod.rs` | `pub(super)` | `pub(super) fn camera_pan(...)` |
| Component fields accessed cross-module | `pub` | `pub current: f32` |
| Top-level compositor | `pub` | `pub fn plugin(app: &mut App)` in `lib.rs` |
| Module declarations in `lib.rs` | `pub(crate)` | `pub(crate) mod gameplay;` |
| Sub-module declarations in compositors | `pub` | `pub mod battlefield;` (inside `pub(crate)` parent) |
| Private sub-modules | `mod` | `mod camera;` in `battlefield/mod.rs` |

**Key rule:** Items inside a `pub(crate)` module use `pub` (not `pub(crate)`) since the module boundary already constrains visibility. Clippy's `redundant_pub_crate` lint enforces this. This means `pub(super) fn plugin` becomes `pub fn plugin` when the parent module is `pub(crate)`.

Re-exports for integration tests: types that external `tests/` crate needs (e.g., `GameState`) are re-exported from `lib.rs` via `pub use screens::GameState;`.

---

## State Management

### Two orthogonal state axes

The codebase uses two independent state machines:

| State | Defined in | Purpose | Variants |
|-------|-----------|---------|----------|
| `GameState` | `screens/mod.rs` | Which screen is active | `Loading`, `MainMenu`, `InGame` |
| `Menu` | `menus/mod.rs` | Which menu overlay is shown | `None`, `Main`, `Pause`, `Victory`, `Defeat` |

Both use `#[states(scoped_entities)]` for automatic entity cleanup via `DespawnOnExit`.

This is the same pattern used by foxtrot and bevy_new_2d: a `Screen` state for which screen is active, and a `Menu` state for overlay menus. The two are orthogonal -- `Menu::Pause` appears while `GameState::InGame` is active, and `Menu::Main` appears while `GameState::MainMenu` is active.

### Pause mechanism (dual-layer)

Pausing uses two complementary mechanisms:

1. **`run_if` gate**: `gameplay_running()` checks `in_state(GameState::InGame).and(in_state(Menu::None))`. When any menu is open, gameplay systems simply don't run.
2. **Virtual time pause**: `OnExit(Menu::None)` calls `time.pause()`, `OnEnter(Menu::None)` calls `time.unpause()`. This freezes `Time<Virtual>` so physics, timers, and animations also stop.

Both layers are needed — `run_if` prevents system execution, virtual time prevents third-party plugins (physics, navmesh) from advancing.

### Importing states

```rust
use crate::screens::GameState;
use crate::menus::Menu;
```

### State-based `run_if` conditions

Gameplay systems that should only run when the game is active (not paused, not in a menu):

```rust
.run_if(crate::gameplay_running)
```

This is a shared helper in `lib.rs` that checks `in_state(GameState::InGame).and(in_state(Menu::None))`.

Menu-specific systems:

```rust
.run_if(in_state(Menu::Pause))
```

### Adding new states

If you need sub-states (e.g., within a loading screen), use `SubStates`:

```rust
#[derive(SubStates, Debug, Hash, PartialEq, Eq, Clone, Default)]
#[source(GameState = GameState::Loading)]
#[states(scoped_entities)]
pub enum LoadingScreen {
    #[default]
    Assets,
    Shaders,
    Level,
}
```

Register with `app.add_sub_state::<LoadingScreen>();`.

---

## Component Co-Location

Components live with their systems in domain plugins, never in a shared `components/` module.

- `Building`, `Occupied`, `GridCursor` -- `src/gameplay/building/mod.rs`
- `PlayerFortress`, `BuildSlot`, `GridIndex` -- `src/gameplay/battlefield/mod.rs`
- `UiCamera` -- `src/ui_camera.rs`

Cross-cutting components (e.g., `Health` used by units and fortresses) should live in the domain that defines the concept, with other modules importing via `use crate::gameplay::Health;`.

### When something is shared

If a component is used by multiple sibling modules, it lives in the common parent's `mod.rs`. If used across top-level modules, it lives in the domain that "owns" the concept.

### Component derives

All components derive `Debug, Reflect` with `#[reflect(Component)]`:

```rust
// Marker component (no fields) -- also gets Clone, Copy
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct PlayerFortress;

// Data component (with fields) -- gets Clone, not Copy (unless all fields are Copy and it makes semantic sense)
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct Building {
    pub building_type: BuildingType,
    pub grid_col: u16,
    pub grid_row: u16,
}
```

Every `Reflect` type must be registered with `app.register_type::<T>()` in the plugin.

### Resource derives

Resources that hold simple data should also derive `Reflect` with `#[reflect(Resource)]` for inspector support:

```rust
#[derive(Resource, Default, Debug, Reflect)]
#[reflect(Resource)]
pub struct HoveredCell(pub Option<(u16, u16)>);
```

Register with `app.register_type::<T>()` alongside components.

---

## Data-Driven Stats Lookup

Game entity stats use `const fn` match expressions instead of trait implementations:

```rust
// gameplay/building/mod.rs:83
pub const fn building_stats(building_type: BuildingType) -> BuildingStats {
    match building_type {
        BuildingType::Barracks => BuildingStats { hp: 300.0, cost: 100, ... },
        BuildingType::Farm => BuildingStats { hp: 150.0, cost: 50, ... },
    }
}
```

Same pattern in `gameplay/units/mod.rs:72` with `unit_stats()`. Benefits:
- **Compile-time evaluation** — stats are constants, no runtime overhead
- **Exhaustive matching** — adding a new variant forces updating all stat lookups
- **Single source of truth** — one function per entity category, no inheritance

Convenience delegates (e.g., `building_hp()`, `building_color()`) call through to the main stats function.

---

## Entity Archetypes

Each entity type has a canonical component bundle documented in `gameplay/mod.rs` and a single spawn function:

| Entity | Spawn Location | Key Components |
|--------|---------------|----------------|
| Unit | `units/mod.rs:spawn_unit()` | `Unit`, `UnitType`, `Team`, `Target`, `CurrentTarget`, `Health`, `CombatStats`, `Movement`, `AttackTimer`, `Mesh2d`, `RigidBody::Dynamic`, `Collider`, `PreferredVelocity`, `AvoidanceAgent`, `NavPath` |
| Building | `building/placement.rs` | `Building`, `BuildingType`, `Team`, `Target`, `Health`, `ProductionTimer`/`IncomeTimer`, `RigidBody::Static`, `Collider`, `NavObstacle` |
| Fortress | `battlefield/renderer.rs` | `PlayerFortress`/`EnemyFortress`, `Team`, `Target`, `CurrentTarget`, `Health`, `CombatStats`, `AttackTimer`, `RigidBody::Static`, `Collider`, `NavObstacle` |
| Projectile | `combat/attack.rs` | `Projectile`, `Team`, `Hitbox`, `Sensor`, `RigidBody::Kinematic`, `Collider`, `CollidingEntities` |

The doc comment at `gameplay/mod.rs:3-17` serves as the canonical archetype reference.

---

## Theme System

Shared UI styling lives in `src/theme/`:

- `theme/palette.rs` -- color constants + font size tokens
- `theme/interaction.rs` -- button hover/press feedback using observers
- `theme/widget.rs` -- reusable widget constructors (header, label, overlay, button)

Both foxtrot and bevy_new_2d have identical `theme/` structures with `palette.rs`, `widget.rs`, and `interaction.rs`.

### Widget constructors

Widget constructors return `impl Bundle` and are used by screen/menu plugins:

```rust
// In theme/widget.rs
pub fn header(text: impl Into<String>) -> impl Bundle { ... }
pub fn label(text: impl Into<String>) -> impl Bundle { ... }
pub fn overlay() -> impl Bundle { ... }
```

Override specific properties when the defaults don't fit:

```rust
commands.spawn((
    widget::header("Auto Battle"),
    TextFont { font_size: 72.0, ..default() },  // Override default 64px
));
```

---

## Global System Ordering

The `GameSet` enum in `lib.rs` defines execution order for all `Update` systems:

```
Input -> Production -> Ai -> Movement -> Combat -> Death -> Ui
```

Domain plugins register their systems in the appropriate set:

```rust
app.add_systems(
    Update,
    camera_pan
        .in_set(GameSet::Input)
        .run_if(crate::gameplay_running),
);
```

Sets are chained in `lib.rs::plugin` so ordering is guaranteed across all plugins.

### When to use `GameSet`

- **Gameplay systems during `InGame`**: Always use the appropriate `GameSet` variant
- **Menu/screen systems** (e.g., `handle_main_menu_input`): `GameSet` is optional since these run in states where the gameplay pipeline is not active
- **OnEnter systems**: Don't need `GameSet` (they run in their own schedule)

### Pausable systems

The `run_if(in_state(Menu::None))` condition acts as the pause gate. When any menu is open, gameplay systems don't run.

Both foxtrot and bevy_new_2d use a similar pattern -- foxtrot has `PausableSystems` as a dedicated `SystemSet`, bevy_new_2d uses `Pause(bool)` state. Our approach with `Menu::None` check is equivalent and simpler since we already have the `Menu` state.

### `chain()` vs `chain_ignore_deferred()`

In Bevy 0.18, `.chain()` auto-inserts `ApplyDeferred` between chained systems. Use `.chain_ignore_deferred()` when you **don't** want deferred commands to flush between systems.

| Method | Behavior | When to use |
|--------|----------|-------------|
| `.chain()` | Inserts `ApplyDeferred` between each pair | Systems that spawn entities needed by later systems in the chain |
| `.chain_ignore_deferred()` | Pure ordering, no flush | Systems that share queries but shouldn't see each other's spawns yet |

**Examples in this codebase:**

- `.chain()` in `battlefield/mod.rs:205` — `spawn_battlefield` then `setup_camera_for_battlefield` (camera needs battlefield entities)
- `.chain_ignore_deferred()` in `combat/attack.rs:192` — `attack` → `move_projectiles` → `handle_projectile_hits` (newly spawned projectiles shouldn't move until next frame)
- `.chain_ignore_deferred()` in `building/mod.rs:223` — `update_grid_cursor` → `handle_building_placement` (cursor position read, not entity spawns)
- `.chain_ignore_deferred()` in `units/mod.rs:241` — `unit_movement` → `rebuild_spatial_hash` → `compute_avoidance` (avoidance pipeline, no intermediate spawns)

---

## Observers

Observers (`add_observer`) react to component lifecycle events. Foxtrot uses 47+ observers for entity setup, cleanup, and UI reactions. All lifecycle event types (`Add`, `Remove`, `Insert`, `Replace`, `Despawn`) and the `On` system param are in `bevy::prelude::*` — no explicit import needed.

### Lifecycle events (Bevy 0.18)

| Event | When it fires | Component still on entity? |
|-------|--------------|---------------------------|
| `Add` | Component added to entity that didn't have it | Yes (just added) |
| `Insert` | Component added, even if already present (runs after `Add`) | Yes |
| `Replace` | Component about to be replaced or removed (runs before `Remove`) | Yes (old value) |
| `Remove` | Component removed and **not** replaced. Also fires during despawn | Yes (about to be removed) |
| `Despawn` | Entity is being despawned (runs after `Remove` for each component) | Varies |

**Ordering during despawn**: `Replace` → `Remove` → `Despawn`.

### Observer handler pattern

The first parameter is `On<Event, Component>`. `On` derefs to the event struct, so `.entity` gives the target entity:

```rust
// Add observer — entity setup (foxtrot's primary pattern)
app.add_observer(setup_player);

fn setup_player(add: On<Add, Player>, mut commands: Commands) {
    commands.entity(add.entity).insert(( /* physics, collider, etc */ ));
}

// Remove observer — cleanup when component is removed or entity despawned
app.add_observer(clear_build_slot);

fn clear_build_slot(
    remove: On<Remove, Building>,
    buildings: Query<&Building>,
    grid_index: Res<GridIndex>,
    mut commands: Commands,
) {
    let Ok(building) = buildings.get(remove.entity) else { return };
    // Building data is still queryable — Remove fires before actual removal
    if let Some(slot) = grid_index.get(building.grid_col, building.grid_row) {
        commands.entity(slot).remove::<Occupied>();
    }
}
```

**Key**: `event.entity` (via `Deref`) = the entity the event targets. `event.observer()` = the observer entity itself (rarely needed).

Handlers support full system params: `Query`, `Res`, `ResMut`, `Commands`, `Single`, etc.

### Registration order

Observers are registered in the plugin function between resources and systems:

```rust
pub(super) fn plugin(app: &mut App) {
    app.register_type::<MyComponent>();     // 1. Types
    app.init_resource::<MyResource>();       // 2. Resources
    app.add_observer(clear_build_slot);       // 3. Observers
    app.add_systems(Update, my_system);      // 4. Systems
}
```

### When to use observers vs systems

- **Observer (`On<Add, T>`)**: One-time entity setup triggered by component addition (replaces "spawn then configure next frame")
- **Observer (`On<Remove, T>`)**: Cleanup when a component is removed or entity despawned (e.g., clearing grid slots, re-enabling collision)
- **OnEnter system**: Setup that happens when entering a state
- **Update system**: Continuous per-frame logic

### Naming conventions (from foxtrot)

| Pattern | Use case | Example |
|---------|----------|---------|
| `setup_*` | Entity configuration on `Add` | `setup_player`, `setup_npc_agent` |
| `spawn_*` | Spawning child entities on `Add` | `spawn_view_model` |
| Verb phrase | Action on `Remove` or custom events | `clear_build_slot`, `enable_collision_with_no_longer_held_prop` |
| `on_add` | Generic `Add` handler | `on_add` (in `npc/mod.rs`) |

Parameter names match the action: `add: On<Add, T>`, `remove: On<Remove, T>`, `_on: On<Event>` (when unused).

### Hooks vs observers

Bevy has two lifecycle reaction mechanisms:
- **Hooks** (`Component::on_add()`, `on_remove()`) — low-level, run synchronously with limited `DeferredWorld` access. Defined per-component type. Use for structural invariants.
- **Observers** (`app.add_observer()`) — high-level, run with full system params (`Query`, `Res`, `Commands`). Multiple observers per event. Use for game logic.

We use observers (not hooks) for all lifecycle reactions.

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

### Cross-plugin `OnEnter` ordering

When multiple plugins register `OnEnter(GameState::InGame)` systems, use `SystemSet` markers to control ordering:

```rust
// battlefield/mod.rs:184 — defines a set
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BattlefieldSetup;

// building/mod.rs — orders after it
app.add_systems(
    OnEnter(GameState::InGame),
    spawn_grid_cursor.after(BattlefieldSetup),
);
```

Same pattern with `DeathCheck` in `combat/death.rs:10` — other systems can order `.before(DeathCheck)` instead of referencing private system functions.

**Rule**: if another plugin needs to order relative to your `OnEnter`/`Update` system, export a `SystemSet` marker instead of making the system `pub`.

---

## Dev Tools

The `src/dev_tools/` module is feature-gated on `dev`:

```toml
[features]
default = ["dev"]
dev = ["bevy/dynamic_linking", "vleue_navigator/debug-with-gizmos", "dep:bevy-inspector-egui"]
```

- `cargo run` includes dev tools (default features)
- `cargo run --release --no-default-features` excludes them

Debug spawners, inspector overlays, state logging, and other development-only tools belong here. Both foxtrot and bevy_new_2d gate dev tools the same way.

### Debug keybindings

| Key | Action | Details |
|-----|--------|---------|
| F3 | Toggle navmesh debug overlay | Shows red navmesh triangulation + yellow unit path lines + green/cyan avoidance vectors. Off by default. |
| F4 | Toggle world inspector | Shows bevy-inspector-egui entity/component browser. Off by default. |

### Debug toggle pattern

Debug overlays use a marker resource whose presence/absence controls visibility:

```rust
// 1. Define marker resource
#[derive(Resource)]
struct ShowWorldInspector;

// 2. Toggle system: insert/remove on keypress (dev_tools/mod.rs:41)
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

---

## Third-Party Plugin Isolation

Non-trivial third-party crates are wrapped in `src/third_party/` with one file per crate. The compositor re-exports the types that domain plugins need:

```rust
// third_party/mod.rs
pub use self::vleue_navigator::NavObstacle;
pub use avian::{CollisionLayer, solid_entity_layers, surface_distance};

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((avian::plugin, vleue_navigator::plugin));
}
```

Foxtrot uses the same pattern with 10+ third-party crates. Domain plugins import via `use crate::third_party::{CollisionLayer, NavObstacle, surface_distance};` — never from the third-party crate directly.

### Collision Layer System

The game uses avian2d collision layers for physics filtering and damage delivery.

#### `CollisionLayer` enum (`third_party/avian.rs`)

| Layer | Purpose | On which entities |
|-------|---------|-------------------|
| `Pushbox` | Physical presence — blocks movement | Units, buildings, fortresses |
| `Hitbox` | Attack collider — deals damage | Projectiles (future: melee swings) |
| `Hurtbox` | Damageable surface | Units, buildings, fortresses |

#### Entity collision setup

| Entity | Memberships | Filters |
|--------|-------------|---------|
| Unit / Building / Fortress | `[Pushbox, Hurtbox]` | `[Pushbox, Hitbox]` |
| Projectile | `[Hitbox]` | `[Hurtbox]` |

Pushbox entities push/block each other (Pushbox↔Pushbox). Projectile hitboxes overlap with target hurtboxes (Hitbox↔Hurtbox) without physical response (`Sensor`).

#### `surface_distance()` wrapper (`third_party/avian.rs`)

Game systems use `surface_distance(&collider1, pos1, &collider2, pos2)` for range checks — never `contact_query` directly. This abstracts the physics dependency to one file.

#### Two-tier testing

- **Tier 1** (primary): `MinimalPlugins` with `Collider` components on test entities. `surface_distance()` is a pure geometric function — no physics pipeline needed. `CollidingEntities` manually populated for hitbox tests.
- **Tier 2** (smoke): Integration tests with `PhysicsPlugins` to validate collision layer wiring. Used sparingly — avian2d's `FixedUpdate` pipeline is unreliable under `MinimalPlugins` (see GAM-29).

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

---

## Testing Patterns

### Test helpers (`src/testing.rs`)

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

### When to use which base app

- **`create_base_test_app()`** -- tests that need real input processing
- **`create_base_test_app_no_input()`** -- tests that manually inject `ButtonInput::press()` (skipping `InputPlugin` prevents `just_pressed` from being cleared in `PreUpdate`)

### Integration test pattern

```rust
#[test]
fn my_system_spawns_entities() {
    let mut app = create_base_test_app();
    app.add_systems(OnEnter(GameState::InGame), my_setup_system);

    transition_to_ingame(&mut app);

    assert_entity_count::<With<MyComponent>>(&mut app, 1);
}
```

Two `app.update()` calls are needed (inside `transition_to_ingame`): first triggers `OnEnter`, second applies deferred commands.

### Key testing lessons

- **`StatesPlugin` required** -- `MinimalPlugins` doesn't include it. Test helpers add it automatically.
- **Plugin before state transition** -- add all plugins BEFORE calling `transition_to_ingame`, or `OnEnter` fires before the plugin is registered.
- **Test systems in isolation** -- register only the system under test. Registering other systems (e.g., `update_grid_cursor`) may overwrite manually-set test data.
- **`InputPlugin` clears `just_pressed` in `PreUpdate`** -- use `create_base_test_app_no_input()` and manually `init_resource::<ButtonInput<T>>()` when you need `press()` to persist through `Update`.

### Coverage target

90% test coverage across the codebase. Every ticket should include tests that maintain or increase coverage.

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

---

## Naming Conventions

| Pattern | Convention | Example |
|---------|-----------|---------|
| Plugin function | `plugin` | `pub(super) fn plugin(app: &mut App)` |
| OnEnter systems | `setup_*` or `spawn_*` | `setup_camera`, `spawn_battlefield` |
| Update systems | verb phrase | `camera_pan`, `handle_building_placement` |
| Entity cleanup | `DespawnOnExit` | `DespawnOnExit(GameState::Loading)` |
| Z-layer constants | `Z_` prefix | `Z_BUILDING`, `Z_UNIT` |
| System sets | `GameSet::Variant` | `GameSet::Input`, `GameSet::Combat` |
| Widget constructors | Return `impl Bundle` | `fn button(text: ...) -> impl Bundle` |
| Observer (Add) | `setup_*`, `spawn_*`, `on_add` | `fn setup_player(add: On<Add, Player>)` |
| Observer (Remove) | verb phrase describing action | `fn clear_build_slot(remove: On<Remove, Building>)` |
| Module doc comments | `//!` at top of file | `//! Battlefield grid layout and rendering.` |

### `Name::new()` convention

All spawned entities include a `Name` component for inspector visibility:

| Pattern | When | Example |
|---------|------|---------|
| Static string | Singletons, unique entities | `Name::new("Player Fortress")` |
| `format!()` | Multiple instances | `Name::new(format!("{team:?} {}", unit_type.display_name()))` |
| Coordinate-based | Grid entities | `Name::new(format!("Build Slot ({col}, {row}"))` |

This makes the F4 world inspector useful for debugging. Always add `Name` when spawning entities, even data-only ones.

---

## System Parameter Patterns

### `Single<>` vs `Query<>`

`Single<D, F>` is for exactly-one-entity queries. If 0 or >1 entities match, the system is **silently skipped** (no panic).

| Use `Single<>` when | Use `Query<>` when |
|---------------------|-------------------|
| Exactly one entity expected (camera, window, fortress) | Zero or many entities (units, buildings, projectiles) |
| System should no-op if entity is missing | System must handle empty/multiple results explicitly |

**`Single` as graceful degradation** (`units/spawn.rs:83`):

```rust
fn tick_enemy_spawner(
    enemy_fortress: Single<&Transform, With<EnemyFortress>>,
    // If fortress is destroyed (despawned), system silently stops running
    // → no more enemy spawns. No explicit "is fortress alive?" check needed.
) { ... }
```

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

---

## Z-Layer Ordering

Defined in `lib.rs`, used across domain plugins:

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

---

## Cargo.toml Conventions

### Dependencies

Use `default-features = false` with explicit feature selection to improve compile times:

```toml
bevy = { version = "0.18", default-features = false, features = ["2d"] }
avian2d = { version = "0.5", default-features = false, features = ["2d", "parry-f32", "debug-plugin", "parallel"] }
vleue_navigator = { version = "0.15", default-features = false, features = ["avian2d"] }
rand = "0.9"
bevy-inspector-egui = { version = "0.36", optional = true }  # gated on `dev` feature
```

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

### Features

```toml
[features]
default = ["dev"]
dev = ["bevy/dynamic_linking", "vleue_navigator/debug-with-gizmos", "dep:bevy-inspector-egui"]
```

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

Standard Bevy recommendation: slight optimization for game code, full optimization for dependencies. Both reference projects use identical settings.

---

## Asset Loading Pattern

When asset preloading is needed, follow the foxtrot/bevy_new_2d `LoadResource` pattern:

```rust
// Define asset collection as Resource + Asset
#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct LevelAssets {
    #[dependency]
    pub music: Handle<AudioSource>,
}

impl FromWorld for LevelAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self { music: assets.load("audio/music/theme.ogg") }
    }
}

// In plugin:
app.load_resource::<LevelAssets>();
```

The `#[dependency]` attribute ensures all handles are fully loaded before the resource is inserted. The loading screen waits for `ResourceHandles::is_all_done()`.

Not yet needed in auto-battle-1 (no complex assets), but this is the pattern to follow when we add audio, sprites, etc.

---

## Spawning Patterns

### Spawn entities with all components at once

Don't split "state spawning" from "render spawning":

```rust
// Good: spawn everything together
commands.spawn((
    Name::new("Player Fortress"),
    PlayerFortress,
    Sprite::from_color(color, size),
    Transform::from_xyz(x, y, Z_ZONE),
    DespawnOnExit(GameState::InGame),
));

// Bad: spawn then insert later
let entity = commands.spawn((PlayerFortress, Transform::default())).id();
// ... later in another system ...
commands.entity(entity).insert(Sprite::from_color(color, size)); // Over-engineering
```

### Use `children!` for hierarchies

Both reference projects use the `children!` macro for parent-child spawning:

```rust
commands.spawn((
    widget::ui_root("Pause Menu"),
    DespawnOnExit(Menu::Pause),
    children![
        widget::header("Game paused"),
        widget::button("Continue", close_menu),
        widget::button("Quit to title", quit_to_title),
    ],
));
```

### Use `DespawnOnExit` for cleanup

Never write manual cleanup systems. Tag entities with `DespawnOnExit(State::Variant)` and the state machine handles it:

```rust
commands.spawn((
    Name::new("Loading Screen"),
    widget::header("Loading..."),
    DespawnOnExit(GameState::Loading),
));
```

---

## Error Handling

Foxtrot sets a global error handler to log instead of panic:

```rust
app.set_error_handler(bevy::ecs::error::error);
```

Consider adopting this when the game is mature enough that panicking on system errors is undesirable.
