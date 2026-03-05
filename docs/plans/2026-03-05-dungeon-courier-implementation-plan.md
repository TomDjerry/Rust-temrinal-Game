# Dungeon Courier Vertical Slice Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Deliver a deterministic playable terminal roguelike loop (map, combat, objective, AI chase, fog, data-driven defs) and preserve continuation context.

**Architecture:** Keep game runtime orchestration in `src/game/mod.rs`, isolate domain mechanics in map/combat/path/data modules, and drive dynamic definitions from `assets/*.json`.

**Tech Stack:** Rust 2024, crossterm, rand, serde/serde_json, anyhow.

---

### Task 1: Core skeleton and dependencies

**Files:**
- Modify: `Cargo.toml`
- Create: `src/game/mod.rs`
- Create: `src/game/data.rs`
- Create: `src/game/combat.rs`
- Create: `src/game/map/mod.rs`
- Create: `src/game/map/path.rs`
- Modify: `src/main.rs`

**Step 1: Add failing tests first**
- Add map connectivity, deterministic generation, BFS, and damage boundary tests.

**Step 2: Run tests to verify red state**
Run: `cargo test`
Expected: compile/runtime failures before implementation is complete.

**Step 3: Implement minimal production code**
- Implement map/tile model, dungeon generation, BFS, combat formula, data loading, and game loop.

**Step 4: Verify green state**
Run: `cargo test`
Expected: all unit tests pass.

**Step 5: Stabilize build**
Run: `cargo build`
Expected: binary builds successfully.

### Task 2: Data-driven assets

**Files:**
- Create: `assets/items.json`
- Create: `assets/monsters.json`

**Step 1: Define minimal item data**
- Include package quest item and healing potion consumable.

**Step 2: Define minimal monster roster**
- Include at least three monster templates.

**Step 3: Validate runtime loading**
Run: `cargo run -- --seed 123`
Expected: game starts, spawns entities, and responds to input.

### Task 3: Documentation and continuity

**Files:**
- Create: `docs/plans/2026-03-05-dungeon-courier-design.md`
- Create: `docs/project-progress.md`

**Step 1: Document implemented architecture and known gaps**
- Capture current feature scope and non-implemented modules.

**Step 2: Maintain progress tracker**
- Track implemented, in-progress, pending, and verification commands for resumable development.

**Step 3: Re-verify before completion claim**
Run: `cargo test && cargo build`
Expected: pass with fresh output evidence.
