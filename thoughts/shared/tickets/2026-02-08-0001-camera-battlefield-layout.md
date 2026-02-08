# Ticket 1: Camera & Battlefield Layout

**Delivers:** Battlefield coordinate system with zones, fortress placeholders, horizontal camera panning, and state architecture refactor

**Depends on:** None

## What to Implement

| What to implement | How to test |
|-------------------|-------------|
| State architecture refactor: SubStates + DespawnOnExit | Pausing no longer destroys InGame entities; no manual cleanup system |
| Battlefield coordinate constants (zone widths, total dimensions) | Constants defined and used consistently |
| Player fortress placeholder (far left, behind buildings) with `PlayerFortress` marker | Blue rectangle visible on far left of battlefield |
| Enemy fortress placeholder (far right) with `EnemyFortress` marker | Red rectangle visible on far right of battlefield |
| Building zone boundaries (6 columns x 8 rows area) | Building zone area clearly marked with distinct background |
| Combat zone (72 columns wide between zones) | Wide combat area visible between building zone and enemy fortress |
| Camera2d positioned for battlefield view | Full vertical view visible, battlefield fills screen height |
| Horizontal camera panning (keyboard A/D or arrow keys) | Can pan left/right to see full battlefield |
| Camera bounds clamping | Camera stops at battlefield edges, can't pan into void |

## Context

This is the foundation ticket. All subsequent tickets build on the coordinate system, zone layout, and state architecture defined here.

The battlefield layout is:
```
[player fort] [6 cols build zone] [72 cols combat zone] [enemy fort]
```
Total width: ~80 columns. The camera must pan horizontally since the full battlefield won't fit on screen at once.

**Current codebase state:** Skeleton Bevy 0.18 project with state machine (Loading/MainMenu/InGame/Paused), manual cleanup system (`CleanupXxx` markers), and 2D camera already spawned in `src/main.rs`.

**State architecture refactor (included in this ticket):**
- Remove `GameState::Paused` — becomes `InGameState::Paused` SubState
- Replace all manual `CleanupXxx` markers with Bevy's built-in `DespawnOnExit`
- Delete custom `cleanup_entities` system
- This fixes the known bug where pausing destroys InGame entities, and must be done before battlefield entities exist

Relevant files:
- `src/main.rs` — camera spawn, app setup
- `src/screens/in_game.rs` — InGame state enter/exit
- `src/screens/paused.rs` — Paused state (becomes SubState)
- `src/lib.rs` — GameState enum (add InGameState SubState)
- `src/components/cleanup.rs` — CleanupXxx markers (to be deleted)
- `src/systems/cleanup.rs` — cleanup_entities system (to be deleted)

## Done When

You can see the full battlefield layout with:
- Blue fortress rectangle on the far left
- Marked building zone area (6x8 grid area)
- Wide combat zone in the middle
- Red fortress rectangle on the far right
- Horizontal camera panning with A/D or arrow keys
- Camera stops at battlefield boundaries
- ESC pauses (overlay appears), ESC resumes — **battlefield persists through pause**
- Q from pause returns to MainMenu — all InGame entities cleaned up

## References

- Source: `thoughts/shared/research/2026-02-04-tano-style-game-research.md` (Section 2.1 Battlefield System, Section 7 Design Decisions)
- Bevy SubStates example: `~/.cargo/registry/src/.../bevy-0.18.0/examples/state/sub_states.rs`
