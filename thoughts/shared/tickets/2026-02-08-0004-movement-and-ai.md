# Ticket 4: Movement & AI — DONE

**Delivers:** Units move toward enemies, temporary enemy spawner for testing

**Depends on:** 3

## What to Implement

| What to implement | How to test |
|-------------------|-------------|
| Unit movement system (move toward target) | Units drift across the screen |
| Target-finding system (nearest enemy) | Units orient toward closest enemy |
| Default behavior: move toward enemy fortress if no enemies | Player units walk right when no enemies present |
| Temporary enemy spawner (debug key to spawn test enemies) | Press a key → red circles appear on the right side |
| Enemy units move left toward player | Red circles move leftward |
| Units stop when within attack range of target | Units stop near each other, don't overlap |
| Target component (tracks current target entity) | Units switch targets when current target dies |

## Context

This ticket adds the core unit AI loop: find target → move toward target → stop at attack range. The actual damage dealing comes in Ticket 5; this ticket just handles movement and targeting.

**Unit behavior priority:**
1. Find nearest enemy unit → move toward it
2. If no enemy units exist → move toward enemy fortress
3. Stop when within attack range

**Targeting throttle:** Units without a target evaluate every frame for instant reaction. Units with a valid target re-evaluate every 10 frames (`RETARGET_INTERVAL_FRAMES`) to balance responsiveness with performance. Targets that are despawned trigger immediate re-evaluation.

**Temporary enemy spawner:** Since the wave system comes in Ticket 7, we need a debug mechanism to spawn test enemies. A simple keypress (e.g., E key) that spawns a few red enemy circles on the right side of the combat zone is sufficient. This will be replaced by the wave system later.

Enemy units follow the same AI but mirrored — they target player units first, then the player fortress.

Relevant files:
- `src/gameplay/units/mod.rs` — Target component (co-located with unit domain)
- New systems in `src/gameplay/units/ai.rs` or `src/gameplay/units/movement.rs`: `unit_find_target`, `unit_movement`
- AI systems use `.in_set(GameSet::Ai)`, movement systems use `.in_set(GameSet::Movement)`
- Debug enemy spawner lives in `src/dev_tools/` (feature-gated on `dev`), not in `screens/in_game.rs`

## Done When

- Player units spawn from Barracks and walk rightward into the combat zone
- Press debug key → enemy (red) units appear on the right
- Enemy units walk leftward
- When units from opposing teams get close, they stop (within attack range)
- If all enemies die, player units resume walking toward enemy fortress position
- Units smoothly update targets when their current target is destroyed

## Status

**DONE** — 2026-02-14. All automated checks pass (71 tests), manual verification confirmed.

## References

- Source: `thoughts/shared/research/2026-02-04-tano-style-game-research.md` (Section 2.3 Unit Behaviors, Section 2.4 Enemy System)
