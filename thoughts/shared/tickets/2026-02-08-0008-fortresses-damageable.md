# Ticket 8: Fortresses as Damageable Entities

**Delivers:** Both fortresses become real entities with health that units can attack

**Depends on:** 1, 5

## What to Implement

| What to implement | How to test |
|-------------------|-------------|
| Player fortress entity with Health component (~2000 HP) | Blue fortress has health, visible as entity |
| Enemy fortress entity with Health component (~2000 HP) | Red fortress has health, visible as entity |
| Fortress marker component | Fortresses distinguishable from other entities in queries |
| Fortress health bars (large, prominent) | Both fortresses show HP bars |
| Units target enemy fortress when no enemy units nearby | Player units walk to and attack enemy fort when no enemies remain |
| Enemies target player fortress when no player units nearby | Enemies attack player fort when path is clear |
| Fortress takes damage from unit attacks | Fortress HP visibly decreases when attacked |

## Context

Ticket 1 created fortress placeholders (colored rectangles). This ticket upgrades them to real game entities with health, making them attackable targets and enabling the victory/defeat conditions in Ticket 9.

**Fortress HP (from research):** ~2000 HP. This provides moderate buffer — a few leaked enemies are survivable, but 20+ units breaking through will destroy the fortress.

**Target priority update:** The unit AI from Ticket 4 already moves toward the enemy fortress when no enemies exist. This ticket makes that behavior actually deal damage by giving fortresses Health components and making them valid attack targets.

**Key distinction:** The player fortress is behind the building zone (far left). Enemies must pass through the combat zone AND the building zone to reach it. The enemy fortress is on the far right — player units must cross the full combat zone.

Relevant files:
- `src/screens/in_game.rs` — fortress entity spawning (upgrade from Ticket 1 placeholders)
- `src/components/mod.rs` — Fortress marker component
- Unit targeting system from Ticket 4 — ensure fortresses are valid targets
- Combat system from Ticket 5 — ensure units can damage fortresses
- Health bar system from Ticket 5 — render fortress health bars

## Done When

- Both fortresses are visible with large health bars showing ~2000 HP
- When no enemy units remain, player units walk to and attack the enemy fortress
- Enemy fortress HP decreases visibly as it takes damage
- When no player units remain, enemy units walk to and attack the player fortress
- Player fortress HP decreases visibly when attacked
- Fortresses can be reduced to 0 HP (sets up victory/defeat for Ticket 9)

## References

- Source: `thoughts/shared/research/2026-02-04-tano-style-game-research.md` (Section 2.1 Fortress Properties, Section 7 Fortress HP decision)
