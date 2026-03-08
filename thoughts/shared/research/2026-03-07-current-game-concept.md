# Game Concept: Auto-Battle Prototype (Current Game)

**Date**: 2026-03-07
**Status**: Living Document
**Purpose**: Design direction for the auto-battle-1 prototype. Standalone game, not the final product.

---

## Overview

A **PvE auto-battler grind game** with deck building and a skill tree. The player builds a collection of building cards, assembles decks, and fights through escalating battles to earn rewards and grow stronger.

This is a standalone prototype — a testing ground for core mechanics (especially the auto-battler combat and skill tree) that will inform the final game (The Lich's Dominion). It is not intended to evolve into the final game.

**Genre**: Auto-battler + Deck builder + Grind RPG
**Theme**: Generic fantasy (not tied to the lich narrative)
**Perspective**: High top-down (~35-degree angle), pixel art
**Multiplayer**: None. Purely PvE.

---

## Core Loop

```
Battle (auto-battler combat)
  |
  v
Rewards (card shards, skill points, currency)
  |
  v
Hub (deck building, skill tree, card tiering)
  |
  v
Battle select (choose next fight, set difficulty)
  |
  v
Battle ...
```

The game is a **grind RPG**. Battles are repeatable. The player replays fights at increasing difficulty for better and exclusive rewards. There is story-like progression that unlocks new battles, but the core engagement is farming and optimizing.

---

## Battle System

### Battlefield

- **2D open field** — large, scrollable area with free camera scroll + zoom
- **Factorio-style grid** for building placement (64x64 tile base)
- **Multiple enemy strongholds** scattered across the battlefield
- **Points of Interest (POIs)** scattered across the field
- **Player's stronghold** to defend
- **Expanding build territory** — the player starts with a build zone near their stronghold. Capturing POIs or destroying enemy strongholds unlocks new build zones across the map

### Combat Flow

- Buildings are placed in **real-time** during combat (no pause)
- Buildings produce units automatically over time
- Units fight autonomously — no micromanagement
- **1000+ units** on screen simultaneously (swarm scale)
- **Rally points** — the player can assign POIs or strongholds as rally targets for individual production buildings. Newly spawned units move toward their building's assigned rally point. Rally points can be changed mid-battle
- Player redirects focus **mid-battle** — choosing which strongholds or POIs to prioritize by reassigning rally points as the situation develops

### Wave Format

Varies by battle:
- Some battles have **continuous enemy streams**
- Others use **distinct waves with breathing room**
- Part of the flexible objective system

### Win/Loss Conditions

**Flexible objectives** — different battles have different conditions:
- Destroy specific enemy strongholds
- Hold POIs for a duration
- Survive for a set time
- Combinations of the above

**Escalating pressure** — enemy waves/spawns increase in power over time. Enemies actively attack the player's base and buildings. The longer the player waits, the harder it gets — this naturally punishes turtling and rewards aggressive play. Faster completion yields better rewards.

**On loss**: no rewards. Clean retry, no cost. The only loss is time spent.

**Battle duration**: varies widely — ~2-5 minutes for easy content, up to ~20 minutes for hard content. Duration scales with difficulty.

### Points of Interest

POIs provide battlefield advantages when captured or destroyed:

**Capture mechanic**: Unit presence. Having more friendly units than enemies near a POI gradually captures it (like Domination in shooters). Contested = no capture progress. Once captured, a POI stays under player control unless enemies recapture it.

POIs also serve as **rally point targets** — buildings can be assigned to send their produced units toward a specific POI.

| POI Type | Effect |
|----------|--------|
| Resource generation | Holding gives ongoing gold income during battle |
| Unit buffs | Holding buffs nearby units (damage, speed, defense) |
| Strategic effects | Reveal map, slow enemies, spawn allied units, block paths |
| Loot on capture | One-time reward on destruction (card shard, bonus, etc.) |

### In-Battle Economy

- **Gold** is the in-battle currency. Used to build buildings (via the shop).
- **Mana** regenerates passively over time during battle. Regen rate can be boosted by skill tree passives or specific buildings. Spent on **active skill abilities** during combat.
- Gold and mana are battle-scoped — they don't persist between fights.

---

## Card & Deck System

### Cards = Buildings

Each card represents a specific building type. The building it produces is fixed — a Barracks card always spawns a Barracks that produces swordsmen.

**10-20 building card types** at launch, each with a distinct unit and role.

**Building synergies** — adjacent buildings can grant bonuses to each other (e.g., Barracks next to Archery Range boosts attack speed). Synergies make grid placement strategically meaningful beyond just territory control.

### Deck Construction

- The player builds a **deck before each battle** from their card collection
- **Minimum 5 distinct card types** required, no maximum deck size
- The deck is a **weighted probability pool** — it is NOT a draw pile
- More copies of a card in the deck = higher probability it appears in the shop
- Cards are never "removed" from the deck when they appear
- **Copy limit per card type** — max copies allowed scales inversely with tier (e.g., Tier 1: max 5 copies, Tier 3: max 2 copies). Prevents mono-deck strategies while giving low-tier cards a probability-weighting niche

### Shop & Reroll (In-Battle)

The shop/reroll system:
- **4 card slots** shown at once, drawn from the deck's probability pool
- Player can reroll for new options
- Playing a card from the shop costs **gold** and places a building on the grid. **Higher-tier cards cost more gold** — creates a tradeoff between cheaper weaker buildings early vs expensive powerful ones later
- Active abilities are **point-targeted** — the player clicks a location on the battlefield to cast (AoE spells, targeted buffs, etc.)
- The building is then independent of the card system — if enemies destroy it, the card is unaffected

### Card Acquisition

**Battle rewards** are the primary source (v1). Three additional channels kept open for future:
- Shop / trade system
- Crafting / fusion
- Events

### Card Tiering

Cards can be upgraded to higher tiers:
- **Requires**: duplicate cards + meta currency
- **Tier count is flexible per card type** — simple buildings may cap at 3 tiers, complex ones may go to 5
- **Effect**: Both stat improvements AND new abilities at tier thresholds
  - Higher stats (more HP, faster production, stronger units)
  - New building abilities at certain tiers (e.g., tier 2 Archery Range gains fire arrows)
- **In-battle cost scales with tier** — higher-tier buildings cost more gold to place, balancing power with economy

### Card Shards & Gacha

- Battles reward **generic card shards** (not per-card-type). Shards can be spent on any card
- **10 shards** to assemble a complete card of your choice
- Duplicate complete cards are used for tiering up. **Scaling cost**: 2 duplicates for the first tier-up, then 3, then 5, etc.
- Higher difficulty battles reward more shards and exclusive card types

---

## Skill Tree System

The same skill tree system planned for the final game. This prototype is the testing ground for the mechanic.

### How It Works

- The skill tree has a **single root node** as the starting point
- **Tree branches** are unlockable pieces that contain multiple skill nodes and can split into multiple paths
- Each branch ends with **hook nodes** where new branches can be connected
- Branches are **purchased** with skill points earned from battle rewards
- The tree grows outward as the player buys and attaches new branches
- **Free respec** — branches can be freely reassigned between battles. The tree is a loadout, not a permanent commitment

### Constraint Design

- The tree is **unbounded** — it can grow indefinitely as the player acquires more branches
- **Skill points** are the bottleneck, not tree space — the player must choose which branches to invest in
- This creates meaningful choices about which paths to prioritize and expand

### Skill Types

- **Passive bonuses** — army-wide stat boosts, production bonuses, economic advantages. **Always active** once unlocked (no ongoing cost)
- **Active abilities** — usable during battle via an ability bar (cooldown-based). **Point-targeted** — player clicks a battlefield location to cast. Spells, buffs, tactical interventions. Cost mana to cast

### Progression Pace

~10-30 new branches over 10 hours of play. The tree grows noticeably each session and is a core part of the engagement loop.

---

## Meta Progression

### Between-Battles Hub

A **menu/UI screen** with tabs:
- **Deck Builder** — manage card collection, build decks, tier up cards
- **Skill Tree** — browse tree, purchase branches, plan builds
- **Battle Select** — choose next fight, set difficulty

No spatial hub — clean and functional.

### Battle Select

- **5-8 distinct battles** at launch, presented as a single list sorted by difficulty
- Story-like progression unlocks new battles
- All battles are **repeatable**
- Player can **increase difficulty** on any battle for better rewards

### Difficulty Scaling Rewards

- Higher difficulty = **more shards, more currency** (better farming)
- Higher difficulty = **exclusive drops** (certain cards and skill tree branches only available at hard+ tiers)
- Both tracks incentivize pushing harder content

### Persistent Rewards

Earned from completing battles:
- **Card shards** (generic) — for assembling and tiering cards
- **Skill points** — for purchasing skill tree branches
- **Meta currency** — for tiering up cards (combined with duplicate cards)

---

## Art Direction

### Style

- **Pixel art**, high top-down (~35-degree angle)
- **64x64 tile base** for grid logic. Building sprites may extend slightly taller (e.g., 64x80) to show roof volume. **16x16 units**, **96-128px strongholds**
- Same art style as the final game (The Lich's Dominion) — shared visual language

### Two Visual Layers

**Sprite Canvas Sizes:**

| Element | Canvas Size | Notes |
|---------|-------------|-------|
| Grid tile | 64x64 px | One building = one tile |
| Buildings | 64x64 px | Standard buildings fill one grid cell |
| Units | 16x16 px | 4:1 ratio vs buildings |
| POIs | 48x48 px | Between unit and building size, stands out as landmark |
| Strongholds | 96x96 or 128x128 px | Largest sprites on the field |

**Buildings (the readable layer):**
- **64x64 pixel art sprites** — one building per grid cell
- High detail: banners, rune carvings, crop rows, flame animations. 6-8 colors per sprite
- Color-coded by function (production = warm, military = steel/red, economy = gold/green)
- Factorio-clean grid placement with subtle tile lines

**Units (the swarm layer):**
- **16x16 pixel art sprites** — 4x smaller than buildings, creating a clear visual hierarchy
- Recognizable fantasy silhouettes: swordsmen with shields, archers with bows, mages with staves, cavalry with mounts
- At 1000+ count, units still form **colored streams and masses** when zoomed out, but are individually readable when zoomed in
- Type distinguished by **silhouette + color + movement pattern**: melee clusters march, ranged spreads pause to fire, mages glide with staff glow
- Death effects are small golden sparkles — satisfying at scale
- Full Bevy entities per unit (not instanced/particle rendering)

### Palette

- Dark terrain backgrounds (deep greens, browns, greys)
- Bright, saturated units (player team vs enemy team clearly color-coded)
- Building highlights for readability
- Bright contrasting spell/ability effects visible even in swarm chaos

### Battlefield Variety

Different battles use different terrain tilesets and color palettes to keep the visual experience fresh and signal difficulty changes. **Terrain is purely cosmetic** — no movement or combat effects. Gameplay terrain effects may be added later.

### Art Style: Arcane Siege

Full palette and image generation prompt in `2026-03-08-art-style-proposals.json`.

Colorful fantasy + Factorio grid. Varied biomes from lush meadows to corrupted wastelands, with difficulty signaled by terrain mood. Charming stone-and-magic buildings with warm rune glows. Units at 16-20px — large enough to read as distinct fantasy characters (swordsmen, archers, mages) while still forming swarm rivers at scale. Green player units (#22DD66). Warm parchment-and-wood UI. The mood is adventurous and inviting — think Warcraft meets Stardew Valley's warmth.

---

## Scope Summary

| System | In prototype | Deferred to final game |
|--------|-------------|----------------------|
| Auto-battler combat | Yes | Enhanced |
| Building grid (Factorio-style) | Yes | Yes |
| Building synergies (adjacency) | Yes | Yes |
| Card/deck system | Yes | Replaced by realm + unit pipeline |
| Skill tree | Yes (full system) | Yes (expanded with unit classes) |
| Shop/reroll | Yes (existing) | TBD |
| Meta progression (grind) | Yes | Yes (realm-based) |
| Action RPG (lich hero) | No | Yes |
| Realm management | No | Yes |
| Elemental mana types | No | Yes |
| Vessels / phylactery | No | Yes |
| Faction system | No | Yes |
| Crafting | No | Deferred |
| PvP | No | No (PvE only) |
| Story/narrative | No | TBD |

---

## Design Pillars

1. **Grind satisfaction** — Replaying battles at higher difficulty should feel rewarding, not repetitive. Better loot, exclusive drops, visible power growth.
2. **Swarm spectacle** — 1000+ units flowing across the battlefield is the visual hook. It should look and feel impressive.
3. **Deck craft** — Building a deck, weighting probabilities, and tiering cards is where strategic depth lives between battles.
4. **Skill tree mastery** — The skill tree is a unique mechanic. It should be satisfying to expand, plan builds, and optimize paths.
5. **Tactical reads** — Mid-battle decisions matter: which stronghold to push, which POI to contest, when to use active skills.
