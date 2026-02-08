# Bevy 0.18 Autobattler Project Setup - Implementation Plan

## Overview

Set up a Rust + Bevy 0.18 project foundation for an autobattler game inspired by "There Are No Orcs". This plan covers project structure, build configuration, linting, testing infrastructure, and basic game shell—without implementing actual gameplay.

## Current State Analysis

- Empty project directory with only `CLAUDE.md`, `.gitignore`, and `thoughts/` structure
- No Rust/Cargo project initialized
- No existing code or configuration

## Desired End State

A fully configured Bevy 0.18 project with:
- Modular plugin-based architecture
- Fast compile times (dynamic linking + LLD/Mold)
- Comprehensive linting (Clippy + rustfmt)
- Unit and integration test infrastructure
- Basic game state machine (Loading → MainMenu → InGame → Paused)
- 2D pixel-art ready camera and rendering
- Placeholder screens for each state

### Verification Criteria:
- `cargo build` compiles without errors
- `cargo test` runs and passes
- `cargo clippy` reports no warnings
- `cargo fmt --check` passes
- Game launches and displays main menu

## What We're NOT Doing

- Game mechanics implementation (combat, units, buildings)
- Asset creation (sprites, audio, fonts)
- Networking or multiplayer
- Save/load system
- CI/CD pipelines
- Actual UI design or gameplay

## Research Summary

### Reference Game: "There Are No Orcs"
| Aspect | Details |
|--------|---------|
| **Core Loop** | Place buildings → Generate units → Auto-combat |
| **Visual Style** | 2D pixel graphics, top-down view |
| **Systems Needed** | Tile grid, unit spawning, auto-combat, progression |

### Bevy 0.18 Key Features
- Feature collections: `2d`, `3d`, `ui` for targeted compilation
- Built-in `PanCamera` for 2D navigation
- Improved UI with directional navigation
- `StateScoped` components for automatic cleanup

---

## Phase 1: Project Initialization

### Overview
Initialize Rust project with Cargo and configure Bevy 0.18 dependency with optimized build settings.

### Changes Required:

#### 1. Initialize Cargo Project
**Command**: `cargo init`

Creates basic Rust project structure.

#### 2. Configure Cargo.toml
**File**: `Cargo.toml`

```toml
[package]
name = "auto-battle"
version = "0.1.0"
edition = "2024"
rust-version = "1.85"

[dependencies]
bevy = { version = "0.18", default-features = false, features = ["2d"] }

[dev-dependencies]
pretty_assertions = "1.4"

[lints.rust]
unsafe_code = "forbid"

[lints.clippy]
all = "warn"
pedantic = "warn"
nursery = "warn"
# Allow common patterns in game dev
needless_pass_by_value = "allow"  # Bevy systems take ownership
too_many_arguments = "allow"      # Bevy queries can have many params
type_complexity = "allow"         # Bevy queries can be complex

# Development profile - fast compile, slow runtime
[profile.dev]
opt-level = 1

# Optimize dependencies even in dev
[profile.dev.package."*"]
opt-level = 3

# Release profile - slow compile, fast runtime
[profile.release]
lto = "thin"
codegen-units = 1
```

#### 3. Configure Fast Builds
**File**: `.cargo/config.toml`

```toml
# Fast compilation configuration for Bevy development
# Reference: https://bevyengine.org/learn/quick-start/getting-started/setup/

[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

[target.aarch64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

[target.x86_64-apple-darwin]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

[target.aarch64-apple-darwin]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

[target.x86_64-pc-windows-msvc]
linker = "rust-lld.exe"

# Enable dynamic linking for faster iterative builds
# Run with: cargo run --features bevy/dynamic_linking
```

#### 4. Configure Rustfmt
**File**: `rustfmt.toml`

```toml
edition = "2024"
max_width = 100
tab_spaces = 4
hard_tabs = false
reorder_imports = true
imports_granularity = "Module"
group_imports = "StdExternalCrate"
use_field_init_shorthand = true
use_try_shorthand = true
```

#### 5. Configure Clippy
**File**: `clippy.toml`

```toml
# Clippy configuration
msrv = "1.85"
cognitive-complexity-threshold = 25
```

### Success Criteria:

#### Automated Verification:
- [x] Project initializes: `cargo init` succeeds
- [x] Dependencies resolve: `cargo fetch` succeeds
- [x] Project compiles: `cargo build` succeeds
- [x] Formatting check passes: `cargo fmt --check`

#### Manual Verification:
- [ ] Verify `Cargo.toml` contains all specified sections
- [ ] Verify `.cargo/config.toml` exists with linker config

**Pause Point**: Confirm Phase 1 complete before proceeding.

---

## Phase 2: Project Structure & Plugin Architecture

### Overview
Create modular directory structure following Bevy best practices with plugin-based organization.

### Changes Required:

#### 1. Create Directory Structure
**Commands**:
```bash
mkdir -p src/{game,screens,ui,components,systems,resources}
mkdir -p assets/{sprites,audio,fonts}
touch assets/.gitkeep
```

#### 2. Create Prelude Module
**File**: `src/prelude.rs`

```rust
//! Common imports for the entire crate.

pub use bevy::prelude::*;

// Re-export game modules
pub use crate::components::*;
pub use crate::resources::*;
pub use crate::GameState;
```

#### 3. Create Components Module
**File**: `src/components/mod.rs`

```rust
//! Game components.

mod cleanup;

pub use cleanup::*;
```

**File**: `src/components/cleanup.rs`

```rust
//! Cleanup marker components for state-scoped entity management.

use bevy::prelude::*;

/// Marker for entities that should be cleaned up when leaving the loading state.
#[derive(Component)]
pub struct CleanupLoading;

/// Marker for entities that should be cleaned up when leaving the main menu.
#[derive(Component)]
pub struct CleanupMainMenu;

/// Marker for entities that should be cleaned up when leaving the game.
#[derive(Component)]
pub struct CleanupInGame;

/// Marker for entities that should be cleaned up when leaving pause.
#[derive(Component)]
pub struct CleanupPaused;
```

#### 4. Create Resources Module
**File**: `src/resources/mod.rs`

```rust
//! Game resources.

// Resources will be added as needed
```

#### 5. Create Systems Module
**File**: `src/systems/mod.rs`

```rust
//! Game systems.

mod cleanup;

pub use cleanup::*;
```

**File**: `src/systems/cleanup.rs`

```rust
//! Generic cleanup systems for state transitions.

use bevy::prelude::*;

/// Despawns all entities with the specified cleanup component.
pub fn cleanup_entities<T: Component>(
    mut commands: Commands,
    query: Query<Entity, With<T>>,
) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}
```

#### 6. Create Game Plugin
**File**: `src/game/mod.rs`

```rust
//! Core game plugin and state management.

use bevy::prelude::*;

/// Main game plugin that sets up core systems and states.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<crate::GameState>();
    }
}
```

#### 7. Create Screens Module
**File**: `src/screens/mod.rs`

```rust
//! Screen plugins for each game state.

mod loading;
mod main_menu;
mod in_game;
mod paused;

pub use loading::LoadingScreenPlugin;
pub use main_menu::MainMenuPlugin;
pub use in_game::InGamePlugin;
pub use paused::PausedPlugin;
```

### Success Criteria:

#### Automated Verification:
- [x] All files created successfully
- [x] `cargo check` passes with new module structure
- [x] `cargo clippy` reports no errors

#### Manual Verification:
- [x] Directory structure matches specification
- [x] Module hierarchy is correctly linked

**Pause Point**: Confirm Phase 2 complete before proceeding.

---

## Phase 3: Game States & Screen Plugins

### Overview
Implement game state machine and placeholder screen plugins with proper enter/exit handling.

### Changes Required:

#### 1. Define Game States
**File**: `src/lib.rs`

```rust
//! Auto-battle game library.

pub mod components;
pub mod game;
pub mod prelude;
pub mod resources;
pub mod screens;
pub mod systems;

use bevy::prelude::*;

/// Primary game states.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    /// Initial loading state.
    #[default]
    Loading,
    /// Main menu state.
    MainMenu,
    /// Active gameplay state.
    InGame,
    /// Paused state (overlay on InGame).
    Paused,
}
```

#### 2. Loading Screen Plugin
**File**: `src/screens/loading.rs`

```rust
//! Loading screen plugin.

use bevy::prelude::*;
use crate::{GameState, components::CleanupLoading, systems::cleanup_entities};

/// Plugin for the loading screen.
pub struct LoadingScreenPlugin;

impl Plugin for LoadingScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Loading), setup_loading_screen)
            .add_systems(
                Update,
                check_loading_complete.run_if(in_state(GameState::Loading)),
            )
            .add_systems(
                OnExit(GameState::Loading),
                cleanup_entities::<CleanupLoading>,
            );
    }
}

fn setup_loading_screen(mut commands: Commands) {
    commands.spawn((
        Text::new("Loading..."),
        TextFont {
            font_size: 48.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0),
            ..default()
        },
        CleanupLoading,
    ));
}

fn check_loading_complete(mut next_state: ResMut<NextState<GameState>>) {
    // For now, immediately transition to main menu
    // In the future, this will wait for assets to load
    next_state.set(GameState::MainMenu);
}
```

#### 3. Main Menu Plugin
**File**: `src/screens/main_menu.rs`

```rust
//! Main menu plugin.

use bevy::prelude::*;
use crate::{GameState, components::CleanupMainMenu, systems::cleanup_entities};

/// Plugin for the main menu.
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MainMenu), setup_main_menu)
            .add_systems(
                Update,
                handle_main_menu_input.run_if(in_state(GameState::MainMenu)),
            )
            .add_systems(
                OnExit(GameState::MainMenu),
                cleanup_entities::<CleanupMainMenu>,
            );
    }
}

fn setup_main_menu(mut commands: Commands) {
    // Title
    commands.spawn((
        Text::new("Auto Battle"),
        TextFont {
            font_size: 72.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(30.0),
            ..default()
        },
        CleanupMainMenu,
    ));

    // Start prompt
    commands.spawn((
        Text::new("Press SPACE to Start"),
        TextFont {
            font_size: 32.0,
            ..default()
        },
        TextColor(Color::srgb(0.7, 0.7, 0.7)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(60.0),
            ..default()
        },
        CleanupMainMenu,
    ));
}

fn handle_main_menu_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        next_state.set(GameState::InGame);
    }
}
```

#### 4. In-Game Plugin
**File**: `src/screens/in_game.rs`

```rust
//! In-game plugin.

use bevy::prelude::*;
use crate::{GameState, components::CleanupInGame, systems::cleanup_entities};

/// Plugin for the main gameplay.
pub struct InGamePlugin;

impl Plugin for InGamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), setup_game)
            .add_systems(
                Update,
                handle_game_input.run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                OnExit(GameState::InGame),
                cleanup_entities::<CleanupInGame>,
            );
    }
}

fn setup_game(mut commands: Commands) {
    commands.spawn((
        Text::new("Game Running - Press ESC to Pause"),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(10.0),
            ..default()
        },
        CleanupInGame,
    ));
}

fn handle_game_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(GameState::Paused);
    }
}
```

#### 5. Paused Plugin
**File**: `src/screens/paused.rs`

```rust
//! Pause menu plugin.

use bevy::prelude::*;
use crate::{GameState, components::CleanupPaused, systems::cleanup_entities};

/// Plugin for the pause menu.
pub struct PausedPlugin;

impl Plugin for PausedPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Paused), setup_pause_menu)
            .add_systems(
                Update,
                handle_pause_input.run_if(in_state(GameState::Paused)),
            )
            .add_systems(
                OnExit(GameState::Paused),
                cleanup_entities::<CleanupPaused>,
            );
    }
}

fn setup_pause_menu(mut commands: Commands) {
    // Semi-transparent overlay
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        CleanupPaused,
    ));

    // Pause text
    commands.spawn((
        Text::new("PAUSED"),
        TextFont {
            font_size: 64.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(40.0),
            ..default()
        },
        CleanupPaused,
    ));

    // Resume prompt
    commands.spawn((
        Text::new("Press ESC to Resume | Q to Quit"),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::srgb(0.7, 0.7, 0.7)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(55.0),
            ..default()
        },
        CleanupPaused,
    ));
}

fn handle_pause_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(GameState::InGame);
    }
    if keyboard.just_pressed(KeyCode::KeyQ) {
        next_state.set(GameState::MainMenu);
    }
}
```

### Success Criteria:

#### Automated Verification:
- [x] `cargo check` passes
- [x] `cargo clippy` reports no warnings
- [x] `cargo fmt --check` passes

#### Manual Verification:
- [x] All screen plugins compile correctly
- [x] State transitions are properly defined

**Pause Point**: Confirm Phase 3 complete before proceeding.

---

## Phase 4: Main Entry Point & Camera Setup

### Overview
Create main.rs with app configuration, 2D camera, and pixel-perfect rendering setup.

### Changes Required:

#### 1. Main Entry Point
**File**: `src/main.rs`

```rust
//! Auto-battle game entry point.

use auto_battle::prelude::*;
use auto_battle::game::GamePlugin;
use auto_battle::screens::{
    InGamePlugin, LoadingScreenPlugin, MainMenuPlugin, PausedPlugin,
};

fn main() {
    App::new()
        // Bevy default plugins with pixel-art configuration
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Auto Battle".to_string(),
                        resolution: (1920.0, 1080.0).into(),
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()), // Pixel-perfect rendering
        )
        // Game plugins
        .add_plugins((
            GamePlugin,
            LoadingScreenPlugin,
            MainMenuPlugin,
            InGamePlugin,
            PausedPlugin,
        ))
        // Startup systems
        .add_systems(Startup, setup_camera)
        .run();
}

/// Sets up the 2D camera with orthographic projection.
fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        OrthographicProjection {
            near: -1000.0,
            far: 1000.0,
            scale: 1.0,
            ..OrthographicProjection::default_2d()
        },
    ));
}
```

#### 2. Update .gitignore
**File**: `.gitignore` (append)

```gitignore
# Rust
/target/
Cargo.lock

# IDE
.idea/
.vscode/
*.swp
*.swo

# OS
.DS_Store
Thumbs.db

# Assets (keep placeholder)
assets/*
!assets/.gitkeep
```

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` succeeds
- [ ] `cargo run` launches without errors
- [x] `cargo clippy` reports no warnings
- [x] `cargo fmt --check` passes

#### Manual Verification:
- [ ] Window opens at 1920x1080 with title "Auto Battle"
- [ ] Loading screen briefly appears
- [ ] Main menu displays "Auto Battle" title
- [ ] Pressing SPACE transitions to in-game
- [ ] Pressing ESC in-game shows pause menu
- [ ] Pressing ESC in pause returns to game
- [ ] Pressing Q in pause returns to main menu

**Pause Point**: Confirm Phase 4 complete before proceeding.

---

## Phase 5: Testing Infrastructure

### Overview
Set up unit testing and integration testing infrastructure for Bevy systems.

### Changes Required:

#### 1. Create Test Utilities Module
**File**: `src/testing.rs`

```rust
//! Testing utilities for Bevy systems.

#![cfg(test)]

use bevy::prelude::*;
use bevy::time::TimePlugin;

/// Creates a minimal app for testing with essential plugins.
pub fn create_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(TimePlugin);
    app
}

/// Creates a test app with state support.
pub fn create_test_app_with_state<S: States>() -> App {
    let mut app = create_test_app();
    app.init_state::<S>();
    app
}

/// Helper to advance the app by one frame.
pub fn tick(app: &mut App) {
    app.update();
}

/// Helper to advance the app by multiple frames.
pub fn tick_multiple(app: &mut App, count: usize) {
    for _ in 0..count {
        app.update();
    }
}
```

#### 2. Add Test Module to lib.rs
**File**: `src/lib.rs` (add at end)

```rust
#[cfg(test)]
pub mod testing;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn game_state_default_is_loading() {
        assert_eq!(GameState::default(), GameState::Loading);
    }

    #[test]
    fn game_states_are_distinct() {
        assert_ne!(GameState::Loading, GameState::MainMenu);
        assert_ne!(GameState::MainMenu, GameState::InGame);
        assert_ne!(GameState::InGame, GameState::Paused);
    }
}
```

#### 3. Create Systems Tests
**File**: `src/systems/cleanup.rs` (add tests at end)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;
    use crate::components::CleanupLoading;

    #[test]
    fn cleanup_removes_marked_entities() {
        let mut app = create_test_app();

        // Spawn entities with cleanup marker
        let entity_with_marker = app.world_mut().spawn(CleanupLoading).id();
        let entity_without_marker = app.world_mut().spawn_empty().id();

        // Add and run cleanup system
        app.add_systems(Update, cleanup_entities::<CleanupLoading>);
        tick(&mut app);

        // Verify marked entity is despawned
        assert!(app.world().get_entity(entity_with_marker).is_err());
        // Verify unmarked entity still exists
        assert!(app.world().get_entity(entity_without_marker).is_ok());
    }

    #[test]
    fn cleanup_handles_empty_query() {
        let mut app = create_test_app();
        app.add_systems(Update, cleanup_entities::<CleanupLoading>);

        // Should not panic with no matching entities
        tick(&mut app);
    }
}
```

#### 4. Create Integration Tests Directory
**File**: `tests/integration/mod.rs`

```rust
//! Integration tests for the auto-battle game.

mod state_transitions;
```

**File**: `tests/integration/state_transitions.rs`

```rust
//! Tests for game state transitions.

use auto_battle::prelude::*;
use auto_battle::game::GamePlugin;
use auto_battle::GameState;
use bevy::input::InputPlugin;
use pretty_assertions::assert_eq;

fn create_game_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(InputPlugin);
    app.add_plugins(GamePlugin);
    app
}

#[test]
fn game_initializes_in_loading_state() {
    let app = create_game_app();
    let state = app.world().resource::<State<GameState>>();
    assert_eq!(*state.get(), GameState::Loading);
}

#[test]
fn can_transition_between_states() {
    let mut app = create_game_app();

    // Transition to MainMenu
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::MainMenu);
    app.update();

    let state = app.world().resource::<State<GameState>>();
    assert_eq!(*state.get(), GameState::MainMenu);
}
```

**File**: `tests/integration.rs`

```rust
//! Integration test runner.

mod integration;
```

#### 5. Create E2E Tests Placeholder
**File**: `tests/e2e/mod.rs`

```rust
//! End-to-end tests for full game scenarios.
//!
//! These tests verify complete gameplay flows and require
//! headless rendering or screenshot comparison.

// E2E tests will be added as gameplay features are implemented.
// For now, this module serves as a placeholder for future tests.

#[test]
fn placeholder_e2e_test() {
    // TODO: Implement full game flow tests
    assert!(true);
}
```

**File**: `tests/e2e.rs`

```rust
//! E2E test runner.

mod e2e;
```

### Success Criteria:

#### Automated Verification:
- [x] `cargo test` runs all tests successfully
- [x] `cargo test --lib` runs unit tests
- [x] `cargo test --test integration` runs integration tests
- [x] `cargo test --test e2e` runs e2e tests
- [x] All tests pass

#### Manual Verification:
- [x] Test output shows correct test organization
- [x] No test failures or panics

**Pause Point**: Confirm Phase 5 complete before proceeding.

---

## Phase 6: Development Tooling & Makefile

### Overview
Create Makefile with common development commands and configure IDE settings.

### Changes Required:

#### 1. Create Makefile
**File**: `Makefile`

```makefile
.PHONY: all build run check test lint fmt clean dev

# Default target
all: check

# Build the project
build:
	cargo build

# Run the game
run:
	cargo run

# Run with dynamic linking (faster iteration)
dev:
	cargo run --features bevy/dynamic_linking

# Run all checks (lint + test)
check: lint test

# Run tests
test:
	cargo test

# Run unit tests only
test-unit:
	cargo test --lib

# Run integration tests only
test-integration:
	cargo test --test integration

# Run e2e tests only
test-e2e:
	cargo test --test e2e

# Run linting
lint: fmt-check clippy

# Check formatting
fmt-check:
	cargo fmt --check

# Apply formatting
fmt:
	cargo fmt

# Run clippy
clippy:
	cargo clippy -- -D warnings

# Clean build artifacts
clean:
	cargo clean

# Build release version
release:
	cargo build --release

# Run release version
run-release:
	cargo run --release
```

#### 2. Create VS Code Settings (Optional)
**File**: `.vscode/settings.json`

```json
{
    "editor.formatOnSave": true,
    "rust-analyzer.checkOnSave.command": "clippy",
    "rust-analyzer.cargo.features": ["bevy/dynamic_linking"],
    "rust-analyzer.procMacro.enable": true,
    "[rust]": {
        "editor.defaultFormatter": "rust-lang.rust-analyzer"
    }
}
```

**File**: `.vscode/extensions.json`

```json
{
    "recommendations": [
        "rust-lang.rust-analyzer",
        "tamasfe.even-better-toml",
        "serayuzgur.crates",
        "vadimcn.vscode-lldb"
    ]
}
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` runs successfully
- [x] `make test` runs all tests
- [x] `make lint` checks formatting and clippy
- [x] `make dev` runs with dynamic linking (not tested headless)

#### Manual Verification:
- [x] VS Code recognizes Rust project (if using VS Code)
- [x] rust-analyzer provides completions

**Pause Point**: Confirm Phase 6 complete before proceeding.

---

## Testing Strategy

### Unit Tests
- Test individual components in isolation
- Test system functions with mock world state
- Test state transitions
- Location: `src/**/*.rs` (inline `#[cfg(test)]` modules)

### Integration Tests
- Test plugin interactions
- Test multi-system workflows
- Test state machine behavior
- Location: `tests/integration/`

### E2E Tests
- Test full game scenarios (future)
- Screenshot comparison (future)
- Performance benchmarks (future)
- Location: `tests/e2e/`

### Running Tests
```bash
make test           # Run all tests
make test-unit      # Run unit tests only
make test-integration  # Run integration tests only
make test-e2e       # Run e2e tests only
```

---

## Performance Considerations

- Dynamic linking enabled for development (`make dev`)
- LLD/Mold linker configured for faster link times
- Release builds use LTO and single codegen unit
- Bevy `2d` feature collection minimizes compile scope

---

## References

- [Bevy 0.18 Release Notes](https://bevy.org/news/bevy-0-18/)
- [Bevy Quick Start Guide](https://bevy.org/learn/quick-start/getting-started/setup/)
- [Bevy Best Practices](https://github.com/tbillington/bevy_best_practices)
- [Unofficial Bevy Cheat Book](https://bevy-cheatbook.github.io/)
- [Bevy Testing Example](https://github.com/bevyengine/bevy/blob/main/tests/how_to_test_systems.rs)
- [TDD in Bevy](https://edgardocarreras.com/blog/tdd-in-rust-game-engine-bevy/)
- [Clippy Configuration](https://doc.rust-lang.org/clippy/configuration.html)
- [Rustfmt Configuration](https://github.com/rust-lang/rustfmt/blob/main/Configurations.md)
- "There Are No Orcs" [Steam Page](https://store.steampowered.com/app/3480990/There_Are_No_Orcs/)
