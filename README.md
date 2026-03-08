# Dungeon Courier (Rust Terminal Roguelike)

A turn-based terminal roguelike built with Rust, `crossterm`, and `ratatui`.

The current version focuses on a deterministic playable vertical slice with:
- procedural dungeon generation
- melee combat
- inventory and equipment systems
- quest delivery loop
- save/load
- AI state machine (patrol/alert/flee)

## Gameplay Goal

To win, you must:
1. Collect the package (`P`)
2. Collect all required quest items (for example `D`)
3. Reach the exit (`E`)

## Key Features

- Deterministic map generation by seed (`--seed`)
- Fog of war + explored memory
- Monster AI:
  - line-of-sight chase
  - noise-driven alert
  - low-HP flee behavior
- Inventory popup:
  - navigate with `w/s` or arrow keys
  - `Enter` use/equip
  - `r` unequip
  - `x` drop (quest items are protected)
- Equipment with stat modifiers:
  - ATK / DEF
  - CRIT / EVA
  - PEN / RES
- Consumables:
  - healing potion
  - temporary buff consumables with duration
- Environment interactions:
  - closed/open doors affect movement and vision
  - `c` closes an adjacent open door
  - one-shot traps deal fixed damage and create alert noise
- Save/load:
  - `F2` quick save
  - `F3` quick load

## Controls

- `WASD` / Arrow keys: move
- `g`: pick up item on current tile
- `u`: use healing potion
- `.`: wait one turn
- `c`: close an adjacent open door
- `i`: open/close inventory
- `?`: open/close help
- `Enter`: use/equip selected inventory item
- `r`: unequip selected item
- `x`: drop selected item
- `F2`: save
- `F3`: load
- `Esc`: close popup / quit if no popup
- `q`: quit game

## Requirements

- Rust toolchain (edition 2024 compatible, Rust 1.85+ recommended)
- A terminal that supports alternate screen and raw input

## Run

```bash
cargo run -- --seed 123
```

Optional map size:

```bash
cargo run -- --seed 123 --width 60 --height 26
```

## Build and Test

```bash
cargo fmt
cargo test
cargo build
```

## Save File

The quick save file is stored at:

```text
saves/save1.json
```

## Project Structure

```text
src/
  main.rs
  game/
    mod.rs          # core types + top-level orchestration
    actions.rs      # action dispatch and turn flow
    ai.rs           # monster AI and state decay
    data.rs         # item/monster data definitions and loading
    contracts.rs    # side contracts and required quest progress
    combat.rs       # damage formula
    inventory.rs    # inventory, equipment, buffs, pickup/use
    save.rs         # save/load serialization helpers
    snapshot.rs     # UI snapshot and view-model mapping
    util.rs         # shared small helpers
    map/
      mod.rs        # map model, generation, FOV, LOS
      path.rs       # BFS pathfinding
    ui/
      mod.rs        # ratatui rendering and popup UI
assets/
  items.json
  monsters.json
docs/
  project-progress.md
  plans/
```

## Current Status

Core loop is playable and tested.  
Latest verified status is tracked in `docs/project-progress.md`.

The runtime was also refactored into smaller modules so future features can land with lower risk.

Side contracts now support `Kill/Collect` objectives plus advanced `time-limit` and `stealth` constraints, including failure states and sidebar status details.

Environment interactions now include open/close doors plus one-shot traps that feed into the existing noise-driven alert pipeline.

## Roadmap (Next)

- Environment interaction follow-ups (locks, hidden traps, richer noise propagation)
- Ranged combat and skill systems
- More automated regression tests
