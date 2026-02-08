# Ticket 9: Victory/Defeat & Game Loop

**Delivers:** Win/lose conditions, end screens, restart functionality — a complete playable game

**Depends on:** 7, 8

## What to Implement

| What to implement | How to test |
|-------------------|-------------|
| Victory detection (enemy fortress HP <= 0) | Destroying enemy fortress triggers victory |
| Defeat detection (player fortress HP <= 0) | Losing player fortress triggers defeat |
| Victory sub-state of InGame | Game transitions to Victory state cleanly |
| Defeat sub-state of InGame | Game transitions to Defeat state cleanly |
| Victory screen UI ("Victory!" message) | Clear win message displayed on screen |
| Defeat screen UI ("Defeat!" message) | Clear lose message displayed on screen |
| Restart button/key from end screen | Can start a new game from victory/defeat screen |
| Full state cleanup on restart | No leftover entities, resources reset, gold back to 200 |
| Return to main menu option | Can go back to main menu from end screen |

## Context

This is the capstone ticket — after this, the prototype is a complete playable game loop: start → build → fight waves → destroy enemy fort (win) or lose your fort (lose) → restart.

**Victory/Defeat as sub-states:** These should be sub-states of InGame (similar to the Paused fix in Ticket 2) so the battlefield remains visible behind the end screen overlay. The game should freeze (no more unit movement, production, or waves) but the scene stays visible for dramatic effect.

**Cleanup on restart:** When restarting, ALL game entities must be despawned and resources reset:
- All units, buildings, fortress entities
- Gold reset to 200
- Wave counter reset to 1
- All timers reset
- Camera position reset

**Alternative victory condition:** If all 10 waves are defeated and no more enemies remain, the player wins even if the enemy fortress isn't at 0 HP. (Optional — discuss during implementation if this makes sense for the prototype.)

Relevant files:
- `src/lib.rs` — Victory/Defeat sub-states
- `src/screens/in_game.rs` — detection systems, state transitions
- New: victory/defeat screen UI
- Cleanup system from `src/systems/cleanup.rs` — ensure full entity cleanup
- All game systems — need run conditions to freeze during Victory/Defeat

## Done When

- Destroying the enemy fortress shows "Victory!" screen
- Losing the player fortress shows "Defeat!" screen
- Both screens overlay the battlefield (game visible behind)
- Game is frozen during end screen (no movement, no spawning)
- Can restart from either screen → fresh game with 200 gold, wave 1
- Can return to main menu from either screen
- No leftover entities from previous game after restart
- Full game loop is playable from start to finish

## References

- Source: `thoughts/shared/research/2026-02-04-tano-style-game-research.md` (Section 2.1 Victory/Defeat Conditions, Ticket 9 spec)
