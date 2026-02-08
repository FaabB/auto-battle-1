# Research: TANO-Style Auto-Battle Game

**Date**: 2026-02-04 (Updated: 2026-02-05)
**Status**: Finalized (Post-Review Update)
**Purpose**: Define architecture and implementation roadmap for a "There Are No Orcs" inspired game

---

## 1. Game Concept Summary

Based on research of [There Are No Orcs](https://store.steampowered.com/app/3480990/There_Are_No_Orcs/), our game will be a **tile-building auto-battler** where:

- Players place **buildings** on a grid (not units directly)
- Buildings **produce units automatically** over time
- Units **auto-battle** enemies without micromanagement
- Strategic depth comes from **building placement synergies** and **resource management**
- Combat continues while player can still build/upgrade

### Key Differentiators from Traditional Tower Defense
| Tower Defense | TANO-Style |
|---------------|------------|
| Towers attack directly | Buildings spawn units that fight |
| Static defense | Dynamic army that pushes forward |
| Enemies path through maze | Units meet enemies on battlefield |
| Round-based building | Continuous building during combat |

---

## 2. Core Systems Analysis

### 2.1 Battlefield System

```
┌───────────────────────────────────────────────────────────────────────────┐
│                              BATTLEFIELD                                  │
├───────┬──────────────────────┬────────────────────────────────────┬──────┤
│PLAYER │   PLAYER ZONE        │           COMBAT ZONE              │ENEMY │
│FORT   │   (Grid for          │           (Where units             │FORT  │
│       │    buildings)        │            fight)                  │      │
│       │                      │                                    │      │
│ [██]  │   [B][B][B][ ]       │       →Units→    ←Enemies←         │ [██] │
│ [██]  │   [B][ ][ ][ ]       │                                    │ [██] │
│       │   [ ][ ][ ][ ]       │                                    │      │
│       │   [ ][ ][ ][ ]       │                                    │      │
└───────┴──────────────────────┴────────────────────────────────────┴──────┘
 DEFEND        BUILD                      FIGHT                    DESTROY
```

**Layout:**
- Player fortress is on the far left (last line of defense, behind buildings)
- Buildings are the front line -- enemies can reach and destroy them
- Units spawn from buildings and move right into the combat zone
- Enemy fortress on far right (the objective)

**Victory/Defeat Conditions:**
- **Victory:** Destroy the enemy fortress
- **Defeat:** Enemy destroys the player fortress

**Fortress Properties:**
```
Fortress {
    team: Team,
    health: f32,         // Player: 2000, Enemy: 2000 (initial values)
    max_health: f32,
    position: Vec2,      // Fixed position (far left for player, far right for enemy)
}
```

**Components needed:**
- Grid system for building placement (player zone)
- Free-movement area for combat
- Player fortress (far left, behind buildings, last line of defense)
- Enemy fortress (far right, must destroy)
- Collision boundaries

### 2.2 Building System

Buildings are the core player interaction:

| Aspect | Description |
|--------|-------------|
| Placement | Grid-snapped, player zone only |
| Production | Spawns units at intervals |
| Cost | Gold/resources to place |
| Synergies | Bonuses from adjacent buildings |
| Upgrades | Improve production rate, unit stats |

**Building Properties:**
```
Building {
    building_type: BuildingType,
    grid_position: (i32, i32),
    production_timer: Timer,
    unit_type_produced: UnitType,
    production_rate: f32,
    level: u32,
    synergy_bonuses: Vec<SynergyBonus>,
}
```

### 2.3 Unit System

Units are spawned by buildings and fight automatically:

**Unit Properties:**
```
Unit {
    unit_type: UnitType,
    position: Vec2,
    health: f32,
    max_health: f32,
    damage: f32,
    attack_speed: f32,
    movement_speed: f32,
    attack_range: f32,
    team: Team (Player | Enemy),
}
```

**Unit Behaviors:**
1. Move toward nearest enemy (or enemy fortress if no enemies)
2. Stop when in attack range
3. Attack automatically
4. Die when health <= 0
5. Target priority: Nearby enemies > Enemy fortress

### 2.4 Enemy System

Enemies spawn from designated points and attack player units/buildings/fortress:

**Wave Properties:**
```
Wave {
    enemy_types: Vec<(EnemyType, count)>,
    spawn_interval: f32,
    difficulty_multiplier: f32,  // Mixed scaling: more enemies + tougher stats
}
```

**Wave Scaling Approach:** Mixed count + stat scaling. Early waves increase enemy count. Later waves also increase enemy HP/damage. This keeps the visual spectacle of growing armies while preventing performance degradation from pure count scaling.

### 2.5 Economy System

```
Economy {
    gold: u32,            // Starting gold: 200
    gold_per_second: f32,  // Passive income from Farms
    // Future: multiple resource types
}
```

**Gold Sources:**
- Passive income (gold-producing buildings: Farms at 3 gold/sec)
- Killing enemies (5 gold per kill)
- Wave completion bonuses

**Gold Sinks:**
- Building placement (Barracks: 100, Farm: 50)
- Building upgrades (post-prototype)
- (Future) Commander abilities

### 2.6 Synergy System

Adjacent building bonuses (TANO's core strategic depth):

```
Synergy {
    trigger: SynergyTrigger,      // e.g., "Adjacent to Barracks"
    effect: SynergyEffect,         // e.g., "+20% production speed"
}
```

**Example Synergies:**
- Barracks + Armory = Units spawn with +10% damage
- Farm + Farm = +25% gold production each
- Tower + Wall = Tower gains +50% range

---

## 3. Technical Architecture

### 3.1 State Machine (Extended)

Current states need expansion:

```
                    ┌──────────────┐
                    │   Loading    │
                    └──────┬───────┘
                           │
                    ┌──────▼───────┐
                    │   MainMenu   │
                    └──────┬───────┘
                           │
              ┌────────────▼────────────┐
              │      LevelSelect        │  (Future)
              └────────────┬────────────┘
                           │
    ┌──────────────────────▼──────────────────────┐
    │                   InGame                     │
    │  ┌─────────────────────────────────────┐    │
    │  │  Sub-states:                        │    │
    │  │  - Combat (waves active, default)  │    │
    │  │  - WaveComplete (rewards)          │    │
    │  │  - Victory / Defeat                │    │
    │  │  - Paused (sub-state of InGame)    │    │
    │  └─────────────────────────────────────┘    │
    └─────────────────────────────────────────────┘
```

**Note on Pause architecture:** Paused MUST be a sub-state of InGame (not a top-level state). ~~The current codebase has Paused as a top-level state, which triggers `OnExit(InGame)` cleanup and destroys all game entities. This must be reworked before gameplay systems are added.~~ **Resolved in Ticket 1:** `Paused` is now an `InGameState` SubState (using Bevy's `#[derive(SubStates)]`). Manual `CleanupXxx` markers replaced with built-in `DespawnOnExit`. Game systems use `run_if(in_state(InGameState::Playing))` to auto-pause.

### 3.2 ECS Component Overview

**Core Components:**
```rust
// Spatial
GridPosition { x: i32, y: i32 }
WorldPosition  // Bevy's Transform

// Buildings
Building { building_type, level }
Produces { unit_type, timer, rate }
Synergizable { bonuses: Vec<Synergy> }

// Units
Unit { unit_type }
Health { current, max }
Combat { damage, attack_speed, range }
Movement { speed }
Team { Player | Enemy }
Target { entity: Option<Entity> }

// Economy
GoldValue { amount }  // For enemies that drop gold

// Fortress
Fortress  // Marker component

// Economy
GoldReward { amount: u32 }  // Kill reward (5 gold per enemy)
```

### 3.3 System Groups

```rust
// Input Systems
- handle_building_placement
- handle_camera_controls (horizontal pan, no zoom)
- handle_pause_input

// Building Systems
- building_production_tick
- calculate_synergies
- spawn_unit_from_building

// Unit Systems
- unit_find_target
- unit_movement
- unit_attack
- unit_death

// Enemy Systems
- wave_spawner
- enemy_ai

// Economy Systems
- passive_gold_generation
- gold_on_kill

// Rendering Systems
- render_grid
- render_buildings
- render_units
- render_ui
```

### 3.4 Plugin Structure

```
GamePlugin
├── BattlefieldPlugin
│   ├── GridPlugin
│   └── CameraPlugin
├── BuildingPlugin
│   ├── PlacementPlugin
│   ├── ProductionPlugin
│   └── SynergyPlugin
├── UnitPlugin
│   ├── MovementPlugin
│   ├── CombatPlugin
│   └── AIPlugin
├── EnemyPlugin
│   └── WavePlugin
├── EconomyPlugin
└── UIPlugin
    ├── HUDPlugin
    └── BuildMenuPlugin
```

---

## 4. Rendering Strategy (Placeholder Phase)

Since we don't have art yet, use basic shapes:

| Entity | Shape | Color |
|--------|-------|-------|
| Player Building | Square | Blue variants |
| Enemy Spawner | Square | Red |
| Player Unit | Circle | Green |
| Enemy Unit | Circle | Red |
| Grid Cell | Outlined square | Gray |
| Gold | Small circle | Yellow |
| Health Bar | Rectangle | Green/Red |

**Implementation:** Use Bevy's built-in `Sprite` component with colored rectangles/circles. Avoid `bevy_prototype_lyon` as it adds CPU-side tessellation overhead that scales poorly with many entities. A 1x1 white pixel texture scaled to size is a common pattern for colored rectangles in Bevy.

---

## 5. Implementation Tickets

9 tickets total (Ticket 1-9). Each delivers a testable milestone.

---

### Ticket 1: Camera & Battlefield Layout
**Delivers:** Battlefield coordinate system, camera setup with horizontal panning

| What to implement | How to test |
|-------------------|-------------|
| Battlefield coordinate system (zones positioned) | Zones visible with debug colors |
| Player fortress position (far left) | Blue fortress rectangle on far left |
| Enemy fortress position (far right) | Red fortress rectangle on far right |
| Building zone boundaries (6x8 area) | Building zone area clearly marked |
| Combat zone (72 columns between zones) | Wide combat area visible |
| Camera2d positioned for battlefield view | Full vertical view, horizontal panning |
| Horizontal camera panning (keyboard/mouse) | Can pan left/right to see full battlefield |

**Done when:** You can see the full battlefield layout with fortress placeholders on both ends and can pan the camera horizontally.

---

### Ticket 2: Grid & Building Placement
**Delivers:** Visible grid, click to place buildings

| What to implement | How to test |
|-------------------|-------------|
| Grid data structure (6x8) | Grid lines visible on screen |
| Grid rendering (outlined squares) | Cells are clearly defined |
| Mouse-to-grid coordinate conversion | Hover highlights correct cell |
| Building component & types | - |
| Click to place building | Click cell → blue square appears |
| Placement validation | Can't place on occupied cell |

**Done when:** You can click around the grid and place blue squares that persist.

---

### Ticket 3: Unit Spawning
**Delivers:** Buildings produce units over time

| What to implement | How to test |
|-------------------|-------------|
| Unit component (type, team) | - |
| Production timer on buildings | - |
| Spawn unit at building position | Place building → units appear nearby |
| Render units as circles | Green circles spawn from buildings |
| Team coloring | Player units = green |

**Done when:** Place a building, wait a few seconds, see green circles spawning.

---

### Ticket 4: Movement & AI
**Delivers:** Units move toward enemies

| What to implement | How to test |
|-------------------|-------------|
| Unit movement system | Units drift across screen |
| Enemy spawner (temporary, for testing) | Press key → red circles appear on right |
| Target-finding (nearest enemy) | Units change direction toward enemies |
| Move toward target | Green moves right, red moves left |
| Stop at attack range | Units stop near each other |

**Done when:** Spawn enemies, watch your units walk toward them and stop when close.

---

### Ticket 5: Combat
**Delivers:** Units fight and die

| What to implement | How to test |
|-------------------|-------------|
| Health component | - |
| Attack system (damage on timer) | Units near each other lose health |
| Death/despawn at 0 HP | Units disappear when killed |
| Health bars above units | See red/green bars shrinking |
| Visual feedback (color flash) | Units flash when hit |

**Done when:** Units engage, health bars drop, losers disappear.

---

### Ticket 6: Economy
**Delivers:** Gold costs for buildings, earn gold from kills

| What to implement | How to test |
|-------------------|-------------|
| Gold resource | - |
| Gold UI display | See "Gold: 200" on screen |
| Building costs | Barracks costs 100 gold, Farm costs 50 gold |
| Deduct gold on placement | Gold decreases when building |
| Can't build without gold | Clicking with 0 gold does nothing |
| Gold from killing enemies | Kill enemy → gold increases |

**Done when:** Start with 200 gold, spend it on buildings, earn more by killing (5 gold per enemy).

---

### Ticket 7: Wave System
**Delivers:** Structured enemy waves with scaling difficulty

| What to implement | How to test |
|-------------------|-------------|
| Wave data structure | - |
| Wave spawner system | Enemies spawn automatically |
| Wave counter UI | "Wave 1/10" visible |
| Wave completion detection | Wave ends when all enemies dead |
| Difficulty scaling | Wave 2 has more/stronger enemies |

**Done when:** Waves auto-spawn, counter increments, later waves are harder.

---

### Ticket 8: Fortresses as Damageable Entities
**Delivers:** Fortresses with health that units can attack

| What to implement | How to test |
|-------------------|-------------|
| Player fortress entity (far left, behind buildings) | Large blue structure visible with ~2000 HP |
| Enemy fortress entity (far right) | Large red structure visible with ~2000 HP |
| Fortress health bars | Both show HP bars |
| Fortress marker component | - |
| Units target fortress when no enemies nearby | Units walk to and attack enemy fort |
| Enemies target player fortress when no player units | Enemies attack player fort |

**Done when:** Both fortresses visible with health bars. Units attack the enemy fortress when no enemies remain. Enemies attack player fortress when no player units block them.

---

### Ticket 9: Victory/Defeat & Game Loop
**Delivers:** Win/lose conditions, complete playable game

| What to implement | How to test |
|-------------------|-------------|
| Victory detection: enemy fortress HP <= 0 | "Victory" screen appears |
| Defeat detection: player fortress HP <= 0 | "Defeat" screen appears |
| Victory/Defeat sub-states | State transitions correctly |
| Victory/Defeat screen UI | Clear win/lose message displayed |
| Restart functionality | Can play again from game over screen |
| Full state cleanup on restart | No leftover entities from previous game |

**Done when:** Full game loop - build army, destroy enemy fort (win) or lose your fort (lose), restart cleanly.

---

### Future Tickets (Post-Prototype)

| Ticket | Focus |
|--------|-------|
| 10 | Synergy system (adjacent building bonuses) |
| 11 | Multiple building/unit types |
| 12 | Building upgrades |
| 13 | Save/Load system |
| 14 | Meta-progression |
| 15 | Multiple maps |
| 16 | Art & polish |

**Future Ideas (Details)**

**Synergy System**
- Adjacent building detection
- Synergy bonus calculation
- Visual synergy indicators

**Multiple Building/Unit Types**
- Rock-paper-scissors unit counters
- Specialized buildings
- Different unit stats and behaviors

**Save/Load System**
- Serialize game state
- Load saved games
- Auto-save

**Meta-Progression**
- Permanent upgrades
- Unlock new buildings/units
- Currency earned across runs

**Multiple Maps**
- Different grid layouts
- Terrain types
- Map-specific challenges

**Polish & Art**
- Replace shapes with sprites
- Add animations
- Sound effects and music
- Particle effects

---

## 6. Data-Driven Design Considerations

For easy balancing and iteration, externalize game data:

```
assets/
├── data/
│   ├── buildings.ron    # Building definitions
│   ├── units.ron        # Unit stats
│   ├── enemies.ron      # Enemy definitions
│   ├── waves.ron        # Wave compositions
│   └── synergies.ron    # Synergy rules
```

**Example building definition (RON format):**
```ron
(
    buildings: [
        (
            id: "barracks",
            name: "Barracks",
            cost: 100,
            size: (1, 1),
            produces: "soldier",
            production_time: 3.0,
            health: 500,
        ),
        (
            id: "farm",
            name: "Farm",
            cost: 50,
            size: (1, 1),
            gold_per_second: 3,
            health: 200,
        ),
    ]
)
```

---

## 7. Design Decisions (All Resolved)

| Question | Decision | Rationale |
|----------|----------|-----------|
| **Building grid size** | 6x8 (medium) | Room for strategy without overwhelming |
| **Camera** | Horizontal pan, no zoom | Required for 72-column combat zone |
| **Building destruction** | Yes, buildings can be destroyed | Buildings are the front line (fortress is behind them) |
| **Game start** | Start with 200 gold | Allows Barracks + Farm or 2 Barracks. Multiple opening strategies. |
| **Unit limits** | No limit | Unlimited chaos, monitor performance later |
| **Combat zone width** | 12x building zone (72 columns) | Wide battlefield, requires camera panning, strategic pacing |
| **Building death** | Units keep fighting | Only future production stops |
| **Game flow** | Continuous | No prep phase, enemies attack from start |
| **Fortress position** | Behind buildings (far left) | Buildings are the front line, fortress is last defense |
| **Starting gold** | 200 | Comfortable opening, multiple strategies viable |
| **Kill reward** | 5 gold per enemy | Low reward; economy buildings dominate income |
| **Fortress HP** | ~2000 | Moderate buffer; a few leaked enemies survivable, 20+ units break through |
| **Farm income** | 3 gold/sec | ~17 second payoff. Balanced against military investment. |

### Battlefield Dimensions (Calculated)

Based on decisions:
- Building zone: 6 columns x 8 rows
- Combat zone: 72 columns wide (12x building zone width)
- Total battlefield width: ~80 columns (fortress + buildings + combat + enemy fortress)

```
[fort] [6 cols build] [72 cols combat] [enemy fort]
```

---

## 8. Recommended Bevy Ecosystem Crates

| Crate | Purpose | When Needed |
|-------|---------|-------------|
| `bevy_common_assets` | RON asset loading via `AssetServer` | Ticket 1 or when data-driven design is set up |
| `bevy_spatial` | KD-tree spatial queries for target-finding | Post-prototype (when O(n^2) becomes bottleneck) |
| `bevy_asset_loader` | Declarative asset loading with state transitions | Ticket 1 (simplifies Loading state) |
| `leafwing-input-manager` | Structured input handling | When input complexity grows (Ticket 2+) |

---

## 9. Known Prototype Limitations

These are acknowledged tradeoffs accepted for the prototype scope (Tickets 1-9):

| Limitation | Impact | Resolution |
|------------|--------|------------|
| **Only 2 building types** (Barracks, Farm) | Shallow strategy; game will be "solved" quickly | Ticket 11: Multiple building/unit types |
| **No synergies** | Building placement is functionally irrelevant (all positions equivalent) | Ticket 10: Synergy system |
| **No building upgrades** | Once grid is full, player agency drops to zero | Ticket 12: Building upgrades |
| **No building selling** | Placement mistakes are permanent | Post-prototype ticket |
| **One unit type** | Combat is a pure numbers game, optimal Farm:Barracks ratio will be found quickly | Ticket 11: Multiple unit types |
| **Potential stalemates** | If production matches wave strength, neither side advances | Wave scaling should eventually break stalemates |
| **Late-game gold sink drought** | Once 48-cell grid fills, gold accumulates with nothing to spend on | Building upgrades (post-prototype) |

---

## 10. References

- [There Are No Orcs - Steam](https://store.steampowered.com/app/3480990/There_Are_No_Orcs/)
- [TANO Review - Indie Sagas](https://indiesagas.com/there-are-no-orcs-review/)
- [TANO Strategy Guide - MinorGames](https://minorgames.com/posts/there-are-no-orcs-forget-the-lore-embrace-the-swarm-strategy-2097)

---

## 11. Next Steps

1. ~~Review this research document~~ Done
2. ~~Resolve design decisions~~ Done
3. ~~Multi-angle review (game design, tech, scope, balance)~~ Done (2026-02-05)
4. **Begin implementation with `/create_plan` for Ticket 1**

### Recommended Implementation Order

| Ticket | Focus | Dependency |
|--------|-------|------------|
| 1 | Camera & Battlefield Layout | None |
| 2 | Grid & Building Placement | 1 |
| 3 | Unit Spawning | 2 |
| 4 | Unit Movement & AI | 3 |
| 5 | Combat System | 4 |
| 6 | Economy | 2 |
| 7 | Wave System | 4, 5, 6 |
| 8 | Fortresses as Damageable Entities | 1, 5 |
| 9 | Victory/Defeat & Game Loop | 8, 7 |

**Note:** Ticket 6 (Economy) can be worked in parallel with Tickets 3-5 since it primarily depends on Ticket 2. Ticket 8 (Fortress entities) can begin after Tickets 1+5 since fortresses are positioned entities with health.


