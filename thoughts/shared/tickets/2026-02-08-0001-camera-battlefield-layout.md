# Ticket 1: Camera & Battlefield Layout

**Delivers:** Battlefield coordinate system with zones, fortress placeholders, and horizontal camera panning

**Depends on:** None

## What to Implement

| What to implement | How to test |
|-------------------|-------------|
| Battlefield coordinate constants (zone widths, total dimensions) | Constants defined and used consistently |
| Player fortress placeholder (far left, behind buildings) | Blue rectangle visible on far left of battlefield |
| Enemy fortress placeholder (far right) | Red rectangle visible on far right of battlefield |
| Building zone boundaries (6 columns x 8 rows area) | Building zone area clearly marked with distinct background |
| Combat zone (72 columns wide between zones) | Wide combat area visible between building zone and enemy fortress |
| Camera2d positioned for battlefield view | Full vertical view visible, battlefield fills screen height |
| Horizontal camera panning (keyboard A/D or arrow keys) | Can pan left/right to see full battlefield |
| Camera bounds clamping | Camera stops at battlefield edges, can't pan into void |

## Context

This is the foundation ticket. All subsequent tickets build on the coordinate system and zone layout defined here.

The battlefield layout is:
```
[player fort] [6 cols build zone] [72 cols combat zone] [enemy fort]
```
Total width: ~80 columns. The camera must pan horizontally since the full battlefield won't fit on screen at once.

**Current codebase state:** Skeleton Bevy 0.18 project with state machine (Loading/MainMenu/InGame/Paused), cleanup system, and 2D camera already spawned in `src/main.rs`. The camera setup will need to be adjusted for battlefield-appropriate positioning.

**Important:** The Paused state is currently a top-level state that triggers `OnExit(InGame)` cleanup. This architectural issue (documented in research) should be noted but does NOT need to be fixed in this ticket — it becomes critical when game entities exist that would be destroyed by the pause transition (Ticket 2+).

Relevant files:
- `src/main.rs` — camera spawn, app setup
- `src/screens/in_game.rs` — InGame state enter/exit
- `src/lib.rs` — GameState enum

## Done When

You can see the full battlefield layout with:
- Blue fortress rectangle on the far left
- Marked building zone area (6x8 grid area)
- Wide combat zone in the middle
- Red fortress rectangle on the far right
- Horizontal camera panning with A/D or arrow keys
- Camera stops at battlefield boundaries

## References

- Source: `thoughts/shared/research/2026-02-04-tano-style-game-research.md` (Section 2.1 Battlefield System, Section 7 Design Decisions)
