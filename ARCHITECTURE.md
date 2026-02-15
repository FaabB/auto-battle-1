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
│   ├── mod.rs           # Menu enum (None, Main, Pause, Victory, Defeat)
│   ├── main_menu.rs     # Main menu UI and input
│   ├── pause.rs         # Pause menu UI and input
│   └── endgame.rs       # Victory/Defeat overlay UI and input
├── gameplay/            # Cross-cutting components + compositor for domain plugins
│   ├── mod.rs           # Team, Health, Target components + composes sub-plugins
│   ├── endgame.rs       # Victory/defeat detection (fortress health checks)
│   ├── battlefield/     # Grid layout, zones, camera panning, rendering
│   │   ├── mod.rs       # Components, constants, GridIndex, plugin
│   │   ├── camera.rs    # Camera setup and panning
│   │   └── renderer.rs  # Battlefield sprite spawning
│   ├── building/        # Placement systems, grid cursor, building components
│   │   ├── mod.rs       # Components (Building, Occupied, GridCursor), plugin
│   │   ├── placement.rs # Cursor tracking and placement systems
│   │   └── production.rs# Barracks unit spawning on timer
│   ├── combat/          # Attack, death, health bars
│   │   ├── mod.rs       # Compositor + re-exports (AttackTimer, DeathCheck, HealthBarConfig)
│   │   ├── attack.rs    # Projectile spawning and damage
│   │   ├── death.rs     # DeathCheck SystemSet + despawn dead entities
│   │   └── health_bar.rs# Health bar spawning and updates
│   ├── economy/         # Gold, shop, income, UI
│   │   ├── mod.rs       # Gold resource, building costs, compositor
│   │   ├── income.rs    # Farm income + kill rewards
│   │   ├── shop.rs      # Shop logic (cards, reroll, selection)
│   │   ├── shop_ui.rs   # Shop panel UI (card buttons, reroll button)
│   │   └── ui.rs        # Gold HUD display
│   └── units/           # Unit components, AI, movement, spawning
│       ├── mod.rs       # Unit, CombatStats, Movement, CurrentTarget + plugin
│       ├── ai.rs        # Target finding and retargeting
│       ├── movement.rs  # Unit movement toward targets
│       └── spawn.rs     # Enemy spawning with ramping difficulty
├── theme/               # Shared color palette and UI widget constructors
│   ├── mod.rs           # Plugin (empty for now, ready for interaction.rs)
│   ├── palette.rs       # Color constants
│   └── widget.rs        # Reusable widget constructors (header, label, overlay)
└── dev_tools/           # Debug-only tools (feature-gated on `dev`)
    └── mod.rs           # Stub, ready for debug overlays and inspector
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
fn main() -> AppExit {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin { ... })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(auto_battle::plugin)
        .run()
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
| `Menu` | `menus/mod.rs` | Which menu overlay is shown | `None`, `Main`, `Pause` |

Both use `#[states(scoped_entities)]` for automatic entity cleanup via `DespawnOnExit`.

This is the same pattern used by foxtrot and bevy_new_2d: a `Screen` state for which screen is active, and a `Menu` state for overlay menus. The two are orthogonal -- `Menu::Pause` appears while `GameState::InGame` is active, and `Menu::Main` appears while `GameState::MainMenu` is active.

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

## Theme System

Shared UI styling lives in `src/theme/`:

- `theme/palette.rs` -- color constants (`HEADER_TEXT`, `BODY_TEXT`, `OVERLAY_BACKGROUND`)
- `theme/widget.rs` -- reusable widget constructors (`header()`, `label()`, `overlay()`)
- (future) `theme/interaction.rs` -- button hover/press behavior using observers

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

### Button pattern (future)

When we add buttons, follow the foxtrot/bevy_new_2d pattern:

```rust
// In theme/widget.rs
pub fn button<E, B, M, I>(text: impl Into<String>, action: I) -> impl Bundle
where
    E: EntityEvent,
    B: Bundle,
    I: IntoObserverSystem<E, B, M>,
{ ... }

// Usage in menus:
widget::button("Continue", |_: On<Pointer<Click>>, mut next_menu: ResMut<NextState<Menu>>| {
    next_menu.set(Menu::None);
})
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

---

## Observers

Observers (`add_observer`) are used for one-time setup triggered by component addition:

```rust
app.add_observer(setup_player);

fn setup_player(add: On<Add, Player>, mut commands: Commands) {
    commands.entity(add.entity).insert(( /* physics, collider, etc */ ));
}
```

This replaces the pattern of "spawn entity, then run system next frame to configure it." Foxtrot uses observers extensively for entity setup (player, NPC, props).

### When to use observers vs systems

- **Observer**: One-time reactions to component Add/Remove events
- **OnEnter system**: Setup that happens when entering a state
- **Update system**: Continuous per-frame logic

We don't use observers yet but should adopt them when entity setup becomes complex (e.g., spawning units with physics components).

---

## Dev Tools

The `src/dev_tools/` module is feature-gated on `dev`:

```toml
[features]
default = ["dev"]
dev = ["bevy/dynamic_linking"]
```

- `cargo run` includes dev tools (default features)
- `cargo run --release --no-default-features` excludes them

Debug spawners, inspector overlays, state logging, and other development-only tools belong here. Both foxtrot and bevy_new_2d gate dev tools the same way, with foxtrot additionally enabling `bevy_dev_tools`, `bevy_ui_debug`, and `bevy-inspector-egui` under the `dev` feature.

---

## Third-Party Plugin Isolation

When adding non-trivial third-party crates (physics, UI framework, etc.), create `src/third_party/` with one file per crate:

```rust
// third_party/mod.rs
pub(super) fn plugin(app: &mut App) {
    app.add_plugins((avian3d::plugin, bevy_enhanced_input::plugin));
}

// third_party/avian3d.rs
pub(super) fn plugin(app: &mut App) {
    app.add_plugins(PhysicsPlugins::default());
}

pub enum CollisionLayer { Default, Character, Prop }
```

Foxtrot uses this pattern with 10+ third-party crates. Not needed yet for auto-battle-1 (no complex third-party deps beyond Bevy itself). Create when needed.

---

## Testing Patterns

### Test helpers (`src/testing.rs`)

| Helper | Description |
|--------|-------------|
| `create_test_app()` | Bare `MinimalPlugins` app |
| `create_base_test_app()` | States + InputPlugin + WindowPlugin + Camera2d |
| `create_base_test_app_no_input()` | States + WindowPlugin + Camera2d (no InputPlugin) |
| `transition_to_ingame(app)` | Sets `GameState::InGame` and runs two updates |
| `count_entities::<F>(app)` | Count entities matching a query filter |
| `assert_entity_count::<F>(app, n)` | Assert exactly N entities match a filter |

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
| Observer targets | `on_add_*` or match event | `fn on_add(add: On<Add, Npc>)` |
| Module doc comments | `//!` at top of file | `//! Battlefield grid layout and rendering.` |

---

## Z-Layer Ordering

Defined in `lib.rs`, used across domain plugins:

| Constant | Value | Purpose |
|----------|-------|---------|
| `Z_BACKGROUND` | -1.0 | Background fill |
| `Z_ZONE` | 0.0 | Fortress and zone sprites |
| `Z_GRID` | 1.0 | Build zone grid cells |
| `Z_GRID_CURSOR` | 2.0 | Hover highlight |
| `Z_BUILDING` | 3.0 | Placed buildings |
| `Z_UNIT` | 4.0 | Units (future) |
| `Z_HEALTH_BAR` | 5.0 | Health bars (future) |

---

## Cargo.toml Conventions

### Dependencies

Use `default-features = false` with explicit feature selection for Bevy to improve compile times:

```toml
bevy = { version = "0.18", default-features = false, features = ["2d"] }
```

### Lints

```toml
[lints.clippy]
too_many_arguments = "allow"   # Systems have many params (DI)
type_complexity = "allow"       # Query types are complex
```

Both foxtrot and bevy_new_2d allow these two lints. Our project additionally enables `pedantic` and `nursery` groups and forbids `unsafe_code`.

### Dev profiles

```toml
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3
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
