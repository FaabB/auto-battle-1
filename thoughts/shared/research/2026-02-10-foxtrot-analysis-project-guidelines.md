# Project Organization Guidelines
## Derived from Foxtrot (Bevy 0.18 Reference Project) Analysis

**Date**: 2026-02-10
**Source**: https://github.com/janhohenheim/foxtrot — Jan Hohenheim's 3D reference project for Bevy 0.18
**Purpose**: Establish scalable project organization patterns for auto-battle-1

---

## 1. Plugin Architecture: Function Plugins, Not Struct Plugins

### Pattern
Foxtrot uses **function plugins** (`fn plugin(app: &mut App)`) instead of struct plugins (`impl Plugin for MyPlugin`). Every module exposes a `pub(super) fn plugin(app: &mut App)` function.

```rust
// gameplay/mod.rs
pub(super) fn plugin(app: &mut App) {
    app.add_plugins((
        animation::plugin,
        crosshair::plugin,
        npc::plugin,
        player::plugin,
        level::plugin,
    ));
}
```

### Why
- Zero boilerplate — no `struct FooPlugin;` + `impl Plugin` needed
- Composes naturally as tuples: `app.add_plugins((a::plugin, b::plugin))`
- The function name is always `plugin` — uniform and predictable

### Guideline for auto-battle-1
- **Adopt function plugins** for all new domain modules
- Parent `mod.rs` composes children with `app.add_plugins((child1::plugin, child2::plugin))`
- Visibility is `pub(super)` by default — only the parent calls the plugin

---

## 2. Module & Directory Hierarchy

### Pattern: Domain Directories with mod.rs
Each top-level domain gets its own directory with a `mod.rs`:

```
src/
├── main.rs              # App assembly only
├── animation.rs         # Cross-cutting animation state machine
├── asset_tracking.rs    # Cross-cutting asset loading
├── audio/
│   ├── mod.rs           # Audio plugin + pools
│   └── perceptual.rs    # Volume math
├── gameplay/
│   ├── mod.rs           # Composes sub-plugins
│   ├── animation.rs     # Gameplay-specific animation helpers
│   ├── level.rs         # Level spawning + LevelAssets
│   ├── crosshair/       # Feature with multiple files → subdirectory
│   │   ├── mod.rs
│   │   └── assets.rs
│   ├── npc/
│   │   ├── mod.rs       # Npc component + setup
│   │   ├── ai.rs        # AI systems
│   │   ├── animation.rs # NPC-specific animation
│   │   ├── assets.rs
│   │   └── sound.rs
│   └── player/
│       ├── mod.rs       # Player component + setup
│       ├── camera.rs
│       ├── input.rs
│       ├── animation.rs
│       ├── assets.rs
│       ├── movement_sound.rs
│       ├── dialogue/    # Sub-feature → deeper nesting
│       │   ├── mod.rs
│       │   └── ui.rs
│       └── pickup/
│           ├── mod.rs
│           ├── collision.rs
│           ├── ui.rs
│           └── sound.rs
├── menus/
│   ├── mod.rs           # Menu state enum + plugin composition
│   ├── main.rs
│   ├── pause.rs
│   ├── settings.rs
│   └── credits.rs
├── screens/
│   ├── mod.rs           # Screen state enum + plugin composition
│   ├── splash.rs
│   ├── title.rs
│   ├── gameplay.rs
│   └── loading/
│       ├── mod.rs       # LoadingScreen SubState
│       ├── preload_assets.rs
│       ├── shader_compilation.rs
│       └── spawn_level.rs
├── theme/
│   ├── mod.rs           # prelude + plugin
│   ├── palette.rs       # Color constants
│   ├── widget.rs        # Reusable UI widget constructors
│   └── interaction.rs   # Button hover/press behavior
├── third_party/
│   ├── mod.rs           # All third-party plugin setup
│   ├── avian3d.rs       # Physics setup + CollisionLayer enum
│   ├── bevy_enhanced_input.rs
│   └── ...              # One file per third-party crate
└── dev_tools/
    ├── mod.rs           # Only included with `#[cfg(feature = "dev")]`
    ├── debug_ui.rs
    ├── input.rs
    └── validate_preloading.rs
```

### When to Create a Subdirectory
- **1 file** → flat file (`src/animation.rs`)
- **2-3 related files** → subdirectory with mod.rs (`src/audio/mod.rs` + `perceptual.rs`)
- **4+ files or sub-features** → nested subdirectories (`src/gameplay/player/pickup/`)

### Guideline for auto-battle-1
```
src/
├── main.rs              # App assembly (DefaultPlugins + our plugins)
├── lib.rs               # Shared types: GameState, InGameState, Z-layers
├── testing.rs           # Test helpers (#[cfg(test)])
├── prelude.rs           # Selective re-exports (keep minimal)
├── battlefield/         # Grid, camera, rendering
│   ├── mod.rs
│   ├── camera.rs
│   └── renderer.rs
├── building/            # Building placement, grid cursor (currently building.rs)
│   ├── mod.rs           # → promote to directory when it grows
│   └── placement.rs
├── combat/              # Future: ticket 5
│   ├── mod.rs
│   └── damage.rs
├── economy/             # Future: ticket 6
│   └── mod.rs
├── units/               # Future: tickets 3-4
│   ├── mod.rs
│   ├── spawning.rs
│   ├── movement.rs
│   └── ai.rs
├── screens/             # State-driven screens
│   ├── mod.rs
│   ├── loading.rs
│   ├── main_menu.rs
│   └── in_game.rs
├── theme/               # UI widgets, colors, interaction
│   ├── mod.rs
│   ├── palette.rs
│   └── widget.rs
└── dev_tools/           # Debug overlays, inspector (feature-gated)
    └── mod.rs
```

---

## 3. State Management

### Pattern: States Own Their Module
State enums live in the `mod.rs` of the module that manages them:

```rust
// screens/mod.rs — Screen state lives here
#[derive(States, Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
#[states(scoped_entities)]  // <-- enables DespawnOnExit
pub(crate) enum Screen {
    #[default]
    Splash,
    Title,
    Loading,
    Gameplay,
}

pub(super) fn plugin(app: &mut App) {
    app.init_state::<Screen>();
    // ...
}
```

```rust
// menus/mod.rs — Menu state lives here
#[derive(States, Debug, Hash, PartialEq, Eq, Clone, Default)]
#[states(scoped_entities)]
pub(crate) enum Menu {
    #[default]
    None,
    Main,
    Credits,
    Settings,
    Pause,
}
```

```rust
// screens/loading/mod.rs — SubState lives with its parent concept
#[derive(SubStates, Debug, Hash, PartialEq, Eq, Clone, Default)]
#[source(Screen = Screen::Loading)]
#[states(scoped_entities)]
pub(crate) enum LoadingScreen {
    #[default]
    Assets,
    Shaders,
    Level,
}
```

### Key Insight: `#[states(scoped_entities)]`
This attribute enables `DespawnOnExit(MyState::Variant)` for automatic entity cleanup. Every state enum should have it.

### Guideline for auto-battle-1
- **Already good**: `GameState` in `lib.rs`, `InGameState` as SubState
- **Action**: Add `#[states(scoped_entities)]` to both state enums (if not already present)
- **Future**: When adding menus, create `src/menus/mod.rs` with a `Menu` state enum

---

## 4. Visibility: `pub(crate)` by Default

### Pattern
Foxtrot uses strict visibility throughout:

| Visibility | Used for |
|-----------|----------|
| `pub(super)` | Plugin functions — only the parent `mod.rs` calls them |
| `pub(crate)` | Components, resources, constants shared across modules |
| `pub(crate)` | Module declarations in `mod.rs` (when other modules need access) |
| private | Internal systems, helper functions, file-local components |

```rust
// gameplay/player/mod.rs
pub(crate) mod camera;     // Other modules (menus/settings) need camera resources
pub(crate) mod input;      // Other modules need BlocksInput
mod animation;             // Only used within player/

pub(super) fn plugin(app: &mut App) { ... }  // Only gameplay/mod.rs calls this

pub(crate) struct Player;  // Other modules need to query for Player
pub(crate) const PLAYER_RADIUS: f32 = 0.5;  // Used by NPC module too
const PLAYER_HEIGHT: f32 = 1.8;  // Only used locally
```

### Guideline for auto-battle-1
- **Switch from `pub` to `pub(crate)`** for all components, resources, and constants
- **Use `pub(super)` for plugin functions**
- **Keep systems private** unless they're registered from another module (rare)
- The `lib.rs` module declarations can stay `pub` since they serve as the crate's API for integration tests

---

## 5. Component Co-Location: Components Live With Their Systems

### Pattern
Components are defined in the same file as the systems that primarily use them. There is no shared `components/` module.

```rust
// gameplay/level.rs — Level component + LevelAssets resource + spawn_level system
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub(crate) struct Level;

#[derive(Resource, Asset, Clone, TypePath)]
pub(crate) struct LevelAssets { ... }

pub(crate) fn spawn_level(mut commands: Commands, level_assets: Res<LevelAssets>) { ... }
```

```rust
// gameplay/player/camera.rs — Camera components + camera resources + camera systems
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
#[require(Transform, Visibility)]
pub(crate) struct PlayerCamera;

#[derive(Resource, Reflect, Debug, Deref, DerefMut)]
#[reflect(Resource)]
pub(crate) struct WorldModelFov(pub(crate) f32);
```

### When Something Is Shared
If a component is used by multiple sibling modules, it lives in the common parent's `mod.rs`:
- `Npc` struct is in `gameplay/npc/mod.rs` — used by `npc/ai.rs`, `npc/animation.rs`, `npc/sound.rs`
- `Player` struct is in `gameplay/player/mod.rs` — used by `player/camera.rs`, `player/input.rs`, etc.

### Guideline for auto-battle-1
- **Already following this**: `Building`, `Occupied`, `GridCursor` live in `building.rs`
- **Rule**: If only one module uses a component, it lives in that module's file
- **Rule**: If sibling sub-modules share a component, it lives in the parent `mod.rs`
- **Never create** a catch-all `components.rs` or `components/mod.rs`

---

## 6. Third-Party Plugin Isolation

### Pattern
All third-party crate configuration lives in `src/third_party/`, one file per crate:

```rust
// third_party/mod.rs
pub(super) fn plugin(app: &mut App) {
    app.add_plugins((
        avian3d::plugin,
        bevy_enhanced_input::plugin,
        bevy_landmass::plugin,
        // ...
    ));
}
```

```rust
// third_party/avian3d.rs — Physics setup isolated here
pub(super) fn plugin(app: &mut App) {
    app.add_plugins(PhysicsPlugins::default())
        .add_observer(enable_interpolation);
}

#[derive(Debug, PhysicsLayer, Default)]
pub(crate) enum CollisionLayer {
    #[default]
    Default,
    Prop,
    Character,
}
```

### Why
- Third-party boilerplate doesn't pollute domain code
- Easy to swap or remove a crate — changes are localized
- Crate-specific types (like `CollisionLayer`) have a clear home

### Guideline for auto-battle-1
- Not needed yet (no complex third-party plugins beyond Bevy itself)
- **Create `src/third_party/` when** we add our first non-trivial third-party crate (physics, UI framework, etc.)

---

## 7. Theme & UI Widget System

### Pattern
Reusable UI is centralized in `src/theme/`:

```rust
// theme/mod.rs — Small prelude for UI convenience
pub(crate) mod prelude {
    pub(crate) use super::{interaction::InteractionPalette, palette as ui_palette, widget};
}

// theme/palette.rs — All color constants
pub(crate) const HEADER_TEXT: Color = Color::WHITE;
pub(crate) const BUTTON_BACKGROUND: Color = Color::srgb(0.2, 0.2, 0.3);
// ...

// theme/widget.rs — Reusable widget constructors returning `impl Bundle`
pub(crate) fn ui_root(name: impl Into<Cow<'static, str>>) -> impl Bundle { ... }
pub(crate) fn header(text: impl Into<String>) -> impl Bundle { ... }
pub(crate) fn button<E, B, M, I>(text: impl Into<String>, action: I) -> impl Bundle { ... }
```

### Usage in Screens
```rust
// menus/settings.rs
use crate::theme::{palette::SCREEN_BACKGROUND, prelude::*};

fn spawn_settings_menu(mut commands: Commands) {
    commands.spawn((
        widget::ui_root("Settings Screen"),
        DespawnOnExit(Menu::Settings),
        children![
            widget::header("Settings"),
            widget::button("Back", go_back_on_click),
        ],
    ));
}
```

### Guideline for auto-battle-1
- **Create `src/theme/` now** — we already have main menu and pause screen UI
- Start with: `palette.rs` (colors), `widget.rs` (button/label helpers)
- Add `interaction.rs` later when we need hover/press feedback

---

## 8. System Organization Within Plugins

### Pattern: One `plugin()` Function Registers Everything

```rust
pub(super) fn plugin(app: &mut App) {
    // 1. State initialization
    app.init_state::<Menu>();

    // 2. Resource initialization
    app.init_resource::<CameraSensitivity>();

    // 3. Sub-plugin composition
    app.add_plugins((camera::plugin, input::plugin));

    // 4. Observers (react to component Add/Remove)
    app.add_observer(setup_player);

    // 5. Systems with scheduling
    app.add_systems(OnEnter(Screen::Title), open_main_menu);
    app.add_systems(
        Update,
        (system_a, system_b)
            .chain()
            .run_if(in_state(Screen::Gameplay)),
    );

    // 6. Asset preloading
    app.load_asset::<Gltf>(Player::model_path());
}
```

### Observers vs Systems
Foxtrot uses **observers** (`add_observer`) for one-time setup triggered by component addition:
```rust
app.add_observer(setup_player);  // Runs when Player component is added

fn setup_player(add: On<Add, Player>, mut commands: Commands) {
    commands.entity(add.entity).insert(( /* physics, collider, etc */ ));
}
```

This replaces the old pattern of "spawn entity, then run system next frame to configure it."

### SystemSets for Ordering
Global system ordering uses `SystemSet` enums:
```rust
#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum PostPhysicsAppSystems {
    TickTimers,
    ChangeUi,
    PlaySounds,
    PlayAnimations,
    Update,
}

// In main.rs:
app.configure_sets(Update, (
    PostPhysicsAppSystems::TickTimers,
    PostPhysicsAppSystems::ChangeUi,
    // ...
).chain());
```

### Guideline for auto-battle-1
- **Adopt the registration order** above: states → resources → sub-plugins → observers → systems → assets
- **Use observers** for entity setup instead of "spawn then configure" chains
- **Define SystemSets** when we have ordering needs across plugins (combat before UI, etc.)

---

## 9. App Assembly: main.rs as the Composition Root

### Pattern
`main.rs` is the **only place** where plugins are assembled into the app. It's the composition root.

```rust
fn main() -> AppExit {
    let mut app = App::new();

    // 1. Error handler
    app.set_error_handler(error);

    // 2. DefaultPlugins with overrides
    app.add_plugins(DefaultPlugins.set(WindowPlugin { ... }).set(ImagePlugin { ... }));

    // 3. Global state + system sets
    app.init_state::<Pause>();
    app.configure_sets(Update, (Set1, Set2, Set3).chain());

    // 4. Third-party plugins
    app.add_plugins(third_party::plugin);

    // 5. Our plugins (order matters for dependencies)
    app.add_plugins((
        asset_tracking::plugin,
        screens::plugin,
        menus::plugin,
        theme::plugin,
        audio::plugin,
        // ...
    ));

    // 6. Plugins that depend on registrations from above
    app.add_plugins((gameplay::plugin, shader_compilation::plugin));

    app.run()
}
```

### Guideline for auto-battle-1
- **main.rs** should only contain `fn main()` and app assembly
- **lib.rs** holds shared types (states, z-layers) and module declarations
- No game logic in either file — delegate everything to domain plugins

---

## 10. Feature Flags for Dev Tools

### Pattern
```toml
[features]
default = ["dev_native"]
dev = ["bevy/dynamic_linking", "bevy/bevy_dev_tools", "dep:bevy-inspector-egui"]
dev_native = ["dev", "native", "bevy/file_watcher"]
release = []
```

```rust
// main.rs
#[cfg(feature = "dev")]
mod dev_tools;

// In app assembly:
#[cfg(feature = "dev")]
app.add_plugins(dev_tools::plugin);
```

### Guideline for auto-battle-1
- **Create `src/dev_tools/mod.rs`** feature-gated on `dev`
- Put debug overlays, state logging, inspector setup there
- Add `dev` feature to Cargo.toml that enables `bevy/dynamic_linking` + debug tools

---

## 11. Cargo.toml Best Practices

### Pattern
```toml
# Explicit Bevy features (no default-features)
bevy = { version = "0.18", default-features = false, features = ["bevy_state", ...] }

# Clippy lints relaxed for Bevy patterns
[lints.clippy]
too_many_arguments = "allow"   # Systems have many params
type_complexity = "allow"       # Query types are complex

# Dev profile optimizations
[profile.dev]
opt-level = 1
[profile.dev.package."*"]
opt-level = 3
```

### Guideline for auto-battle-1
- Add the `[lints.clippy]` section for `too_many_arguments` and `type_complexity`
- Add dev profile optimizations if not already present
- Consider explicit Bevy features for faster compilation (only what we use)

---

## 12. Naming Conventions

### Files
| Scope | Convention | Example |
|-------|-----------|---------|
| Domain plugin | Directory + mod.rs | `src/combat/mod.rs` |
| Sub-feature | Named .rs file | `src/combat/damage.rs` |
| Cross-cutting utility | Top-level .rs file | `src/animation.rs` |
| Asset definition | `assets.rs` inside domain dir | `src/units/assets.rs` |

### Code
| Item | Convention | Example |
|------|-----------|---------|
| Plugin function | `pub(super) fn plugin` | Always `plugin`, never `build` |
| Setup system (OnEnter) | `spawn_*` or `setup_*` | `spawn_battlefield`, `setup_player` |
| Teardown (OnExit) | Prefer `DespawnOnExit` | Don't write manual cleanup |
| Marker component | Unit struct | `struct Player;` |
| Data component | Named fields | `struct Health { current: f32, max: f32 }` |
| Observer target | `on_add_*` or match the event | `fn on_add(add: On<Add, Npc>)` |
| Widget constructors | Return `impl Bundle` | `fn button(text: ...) -> impl Bundle` |

---

## 13. Summary: Key Differences from Current auto-battle-1

| Current | Recommended Change |
|---------|-------------------|
| Struct plugins (`impl Plugin for X`) | Function plugins (`fn plugin(app: &mut App)`) |
| `pub` visibility everywhere | `pub(crate)` default, `pub(super)` for plugins |
| States in `lib.rs` | States in the `mod.rs` of the module that manages them |
| No theme system | Create `src/theme/` for shared UI |
| No dev_tools separation | Feature-gate debug tools in `src/dev_tools/` |
| `prelude.rs` with wildcard exports | Minimal prelude or eliminate entirely |
| N/A | `third_party/` pattern ready when needed |

---

## 14. Migration Priority

These changes can be adopted incrementally, one ticket at a time:

1. **Immediate** (next ticket): Switch to function plugins, tighten visibility
2. **Soon** (within 2-3 tickets): Create `theme/` module, promote `building.rs` to `building/`
3. **When needed**: Create `third_party/`, `dev_tools/`, SystemSets
4. **Ongoing**: Follow co-location rule, naming conventions, `#[states(scoped_entities)]`
