# Dungeon Courier Design (Current Implementation)

## Scope
This iteration implements a playable vertical slice focused on M1+M2 and part of M3:
- deterministic dungeon generation with rooms and corridors
- player movement, collision, melee combat, and turn loop
- package pickup and exit-based victory condition
- basic monster AI with line-of-sight chase and BFS step selection
- FOV visibility + explored fog memory
- JSON-driven item and monster definitions

## Architecture
The runtime is split into:
- `game::mod`: game loop, input handling, state mutation, rendering
- `game::map`: map model, generation, visibility, and geometry utilities
- `game::map::path`: BFS pathfinding
- `game::combat`: deterministic combat formula + random variance
- `game::data`: loading external item/monster definitions

## Runtime Flow
1. Parse `--seed/--width/--height` from CLI.
2. Load assets from `assets/items.json` and `assets/monsters.json`.
3. Generate map from seed; place player/package/exit/spawns.
4. Start terminal raw loop and process key actions:
   - movement and melee
   - pickup and potion usage
   - wait/help/quit
5. After each consumed player turn:
   - execute monster turn
   - cleanup dead monsters
   - recompute FOV and explored tiles
   - evaluate win/lose status

## Data Model Decisions
- Tile model keeps movement/vision semantics in `TileType` + `Tile` methods.
- Package is represented as a quest item (`kind = quest_package`) in external data.
- Healing potions are consumable data entries (`kind = consumable`, `heal = N`).
- Monsters are definition-driven (`hp/atk/def/glyph`) and spawned from floor candidates.

## Test Strategy
Unit tests currently cover:
- generated map objective connectivity
- same-seed deterministic generation
- BFS route correctness and blocked-cell handling
- combat damage formula boundaries and min-damage guard

## Known Gaps
Not yet implemented in this iteration:
- save/load
- contract side quests and rewards
- advanced AI states (alert/flee/noise memory)
- inventory/equipment UI panels and richer item taxonomy
- ratatui multi-panel rendering
