# 2026-03-05 UI Upgrade (Ratatui Panels + Popups) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade rendering to ratatui split panels with inventory/help popups while keeping current gameplay and key behavior unchanged.

**Architecture:** Keep game mechanics in `src/game/mod.rs`; move rendering/layout concerns to `src/game/ui/mod.rs` with explicit UI mode routing.

**Tech Stack:** Rust 2024, ratatui, crossterm, anyhow.

---

### Task 1: UI behavior tests first (TDD red)

**Files:**
- Create: `src/game/ui/mod.rs` (test section first)

**Step 1: Write failing tests**
- Test adaptive sidebar ratio for width buckets.
- Test popup mode transitions (`Normal <-> Inventory/Help`).

**Step 2: Run tests to confirm failure**
Run: `cargo test`
Expected: FAIL before implementations are complete.

### Task 2: Implement ratatui rendering layer

**Files:**
- Modify: `Cargo.toml`
- Create: `src/game/ui/mod.rs`
- Modify: `src/game/mod.rs`

**Step 1: Add `ratatui` dependency**

**Step 2: Implement UI helpers and rendering**
- Root split with adaptive ratio.
- Right sidebar: status / quest / log.
- Map panel rendering with fog/FOV behavior preserved.
- Centered popup for inventory/help.

**Step 3: Wire game input by UI mode**
- `i` toggle inventory popup
- `?` toggle help popup
- `Esc` closes popup; if no popup, exits
- Preserve existing gameplay keys in normal mode

### Task 3: Verify + docs update

**Files:**
- Modify: `docs/project-progress.md`

**Step 1: Verification**
Run: `cargo test && cargo build`
Expected: PASS

**Step 2: Runtime smoke check**
Run: `cargo run -- --seed 123`
Expected: split panels visible; popups togglable; gameplay unchanged.

**Step 3: Update progress document**
- Move UI panel/popup items from in-progress to done and refresh next TODOs.
