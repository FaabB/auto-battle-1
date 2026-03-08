# Art Style Guide

This document defines the visual direction for auto-battle-1. It is the authoritative reference for all art assets — equivalent to `ARCHITECTURE.md` for code.

**Style name**: Arcane Siege
**Origin**: `thoughts/shared/research/2026-03-08-art-style-proposals.json`

---

## Overview

Colorful fantasy pixel art with rich, varied battlefields. High-readability high top-down (~35-degree angle) view optimized for 1,000+ unit swarms. Buildings are charming stone-and-magic anchors with visible roofs and front walls, while units (16x16) form vibrant team-colored rivers with readable faces and weapons. The mood is adventurous and inviting — think Warcraft meets Stardew Valley's warmth.

---

## Rendering Rules

| Rule | Specification |
|------|---------------|
| Perspective | High top-down (~35-degree angle) |
| Shadows | Global 2px offset (bottom-right) drop shadow at 30% opacity for all buildings and units |
| Visual anchoring | Every building has a 4-8px footprint layer (dirt, stone, cracked earth) that blends into terrain |
| Readability threshold | Units never exceed 25% of the brightness of spell effects. Spells use additive blending; units use alpha blending |
| Sorting | Strict Y-sorting based on entity's bottom-most pixel (feet) for depth and minimal occlusion |

---

## Color Palette

### Background (Meadow biome default)

| Role | Hex | Description |
|------|-----|-------------|
| Primary | `#1E3020` | Darkest ground shade |
| Secondary | `#2A4A2C` | Mid ground shade |
| Accent | `#3B5E3A` | Highlight ground shade |

### Team Colors

| Role | Hex | Description |
|------|-----|-------------|
| Player primary | `#22DD66` | Player units, banners |
| Player secondary | `#44FFAA` | Player highlights |
| Player accent | `#88FFDD` | Player glow effects |
| Enemy primary | `#DD2233` | Enemy units, banners |
| Enemy secondary | `#FF4455` | Enemy highlights |
| Enemy accent | `#FF8866` | Enemy glow effects |

### VFX

| Role | Hex | Description |
|------|-----|-------------|
| Holy spell | `#FFEE55` | Yellow-gold spell effects |
| Arcane spell | `#DD88FF` | Purple spell effects |
| Fire spell | `#FF5500` | Orange fire effects |
| Death particles | `#FFEE55` | Golden mana sparkles on death |

### UI

| Role | Hex | Description |
|------|-----|-------------|
| Background | `#2C2418` | Panel background |
| Text | `#F5EDE0` | Warm off-white text |
| Highlight | `#E8C840` | Gold highlight |
| Border | `#8B7355` | Bronze border |
| Gold currency | `#FFB830` | Gold display |
| Mana currency | `#7B5FDD` | Mana display |

### Battlefield

| Role | Hex | Description |
|------|-----|-------------|
| POI neutral | `#A09880` | Uncaptured POI |
| POI captured | `#33BBFF` | Player-captured POI |
| Building military | `#5566AA` | Military building tint |
| Building economy | `#66884A` | Economy building tint |
| Building production | `#AA7744` | Production building tint |

### Biomes

Each biome has its own ground palette. Terrain is purely cosmetic — no gameplay effects.

| Biome | Primary | Secondary | Accent | Mood |
|-------|---------|-----------|--------|------|
| Meadow | `#1E3020` | `#2A4A2C` | `#3B5E3A` | Lush green grass, wildflowers, warm sunlight |
| Enchanted Forest | `#142818` | `#1E3E22` | `#2B5530` | Towering trees, dappled light, magical mushrooms, fireflies |
| Frozen Pass | `#1C2838` | `#283848` | `#3A5068` | Snow-covered ground, ice crystals, cold blue light, frosted rocks |
| Desert Ruins | `#2E2418` | `#443420` | `#5E4A30` | Warm sand, ancient stone ruins, heat shimmer, golden light |
| Corrupted Wastes | `#1A1020` | `#281830` | `#3D2548` | Dark purple terrain, pulsing veins, twisted flora — the hard mode biome |

---

## Sprite Specifications

### Buildings

| Property | Value |
|----------|-------|
| Canvas | 64x64 px (base footprint for grid logic) |
| Outline | 1px black |
| Colors | 6-8 colors per sprite |
| Footprint | 4-8px perimeter of ground-blending pixels (dirt for farms, cobblestone for barracks) |
| Animation | 2-3 frame idle loops. Production timer shown as small radial fill on sprite |
| Style | Warm, charming fantasy architecture. Dominant roof and foreshortened front wall |

**Damage states:**

| State | Visual |
|-------|--------|
| 100% HP | Clean, colorful, animated |
| 50% HP | Subtle cracks on stone, dust particles, animation speed slows 20% |
| 25% HP | Heavy cracks, small localized fire/smoke, rune lights flicker erratically |

### Units

| Property | Value |
|----------|-------|
| Canvas | 16x16 px, rendered at native size |
| Directions | 4-directional (up, down, left, right). Left/right mirrored |
| Proportions | Oversized heads for readability. Head/helmet = top 8-10px. Torso/legs = bottom 6-8px. Weapons stick out clearly |
| Elite variants | 18x18 px, 1px gold trim on silhouette, 1-frame shimmer overlay every 2s |

**Movement feel:**

| Unit type | Animation |
|-----------|-----------|
| Melee | 2-frame marching bob |
| Mages | Smooth glide |
| Cavalry | 3-frame gallop |

**Death effect:** 3-frame burst: Flash → Expand with team-colored sparkles and equipment fragments → Fade to golden mana particles.

### POIs

| Property | Value |
|----------|-------|
| Canvas | 48x48 px |
| Style | Silhouette-based: obelisk = buff, mine = resource, watchtower = strategic, chest = loot |

### Strongholds

| Property | Value |
|----------|-------|
| Canvas | 96-128 px |
| Player | Radiant core, warm stone, green banners |
| Enemy | Crimson core, dark obsidian, red banners |

### Terrain Tiles

| Property | Value |
|----------|-------|
| Canvas | 64x64 px |
| Style | Flat 2D square logic, 3-4 shades per biome. Ground stays subdued so units and buildings pop |

---

## UI Style

Warm parchment-and-wood fantasy UI. Feels like an adventurer's toolkit.

| Property | Value |
|----------|-------|
| Font | Pixel font (5x7 base grid) in warm off-white (`#F5EDE0`) |
| Panels | Warm dark-brown (`#2C2418`) with 1px bronze borders (`#8B7355`) and rounded corners |

---

## VFX Layering

Spells use a dedicated palette (purple/gold/orange) to ensure visibility on top of both green (player) and red (enemy) unit swarms. Spell effects use additive blending; units use alpha blending.

---

## Scene Reference Prompt

For generating full-scene mockups:

> High top-down 35-degree angle pixel art game mockup. A lush green meadow battlefield with a dark-green grass texture and a faint drop shadow under every object. On the left, a player stronghold (warm stone, blue glowing crystals, green banners, showing front wall and large roof). On the right, an enemy dark-stone fortress (red banners, pulsing crimson light). Between them, two massive swarms of tiny 16x16 pixel units (bright green vs bright red) collide in the center. Units show heads, faces, and weapons clearly. Where they meet, golden sparkles and bright purple magical bursts pop. Charming 64x64 base fantasy buildings (barracks, farms, forges) project slightly upward to show roofs. The bottom UI is a warm wood-and-parchment spellbook-style bar with four wooden card frames. 32x32 pixel style, crisp edges, no anti-aliasing, vibrant colors, dark background terrain.

---

## Asset Manifests

Per-asset generation specs (PixelLab prompts, parameters, versions) live in `manifest.json` files alongside the assets:

```
assets/sprites/
├── buildings/manifest.json
├── units/manifest.json
├── strongholds/manifest.json
├── terrain/meadow/manifest.json
└── vfx/manifest.json
```

See each manifest for individual asset descriptions and generation parameters.
