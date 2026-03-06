# Game Runtime Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor the runtime around `Game` into focused modules without changing gameplay, save semantics, or current UI behavior.

**Architecture:** Keep `Game` as the aggregate root in `src/game/mod.rs`, and move domain logic into sibling modules that extend `impl Game`. Prefer extraction over redesign. Preserve behavior through existing tests plus a few focused regression checks around the new module boundaries.

**Tech Stack:** Rust 2024, anyhow, crossterm, ratatui, rand, serde, serde_json, cargo test, cargo clippy.

---

### Task 1: Extract shared utilities and save/load logic

**Files:**
- Create: `src/game/util.rs`
- Create: `src/game/save.rs`
- Modify: `src/game/mod.rs`
- Test: `src/game/save.rs`

**Step 1: Write the failing test**
- Move or add a save roundtrip test in the save-focused module test area.

**Step 2: Run test to verify it fails**

Run: `cargo test save_load_roundtrip_should_restore_core_state -v`
Expected: FAIL because the moved test or extracted functions are not wired yet.

**Step 3: Write minimal implementation**
- Move `SaveState`, `save_to_file`, `load_from_file`, `to_save_state`, `from_save_state` into `src/game/save.rs`.
- Move duplicated `strip_bom` into `src/game/util.rs` and update both asset loading and save loading to use it.

**Step 4: Run targeted tests**

Run: `cargo test save_load_roundtrip_should_restore_core_state -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/game/mod.rs src/game/save.rs src/game/util.rs
git commit -m "refactor: extract save and util modules"
```

### Task 2: Extract contracts and quest progress helpers

**Files:**
- Create: `src/game/contracts.rs`
- Modify: `src/game/mod.rs`
- Test: `src/game/contracts.rs`

**Step 1: Write the failing test**
- Move or add focused tests for contract completion and required quest item rules.

**Step 2: Run test to verify it fails**

Run: `cargo test kill_contract_should_complete_and_grant_reward collect_contract_should_progress_on_pickup_and_grant_reward -v`
Expected: FAIL due to moved logic not yet reconnected.

**Step 3: Write minimal implementation**
- Move contract helpers, required quest item helpers, reward handling, and progress logging into `src/game/contracts.rs`.
- Keep public behavior unchanged.

**Step 4: Run targeted tests**

Run: `cargo test kill_contract_should_complete_and_grant_reward collect_contract_should_progress_on_pickup_and_grant_reward -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/game/mod.rs src/game/contracts.rs
git commit -m "refactor: extract contracts module"
```

### Task 3: Extract inventory, equipment, and item-effect behavior

**Files:**
- Create: `src/game/inventory.rs`
- Modify: `src/game/mod.rs`
- Test: `src/game/inventory.rs`

**Step 1: Write the failing test**
- Move or add focused tests for equip/use/drop/buff behavior.

**Step 2: Run test to verify it fails**

Run: `cargo test inventory_use_potion_should_consume_turn equipment_use_should_increase_effective_stats attack_buff_consumable_should_apply_and_expire -v`
Expected: FAIL until extracted methods are wired back.

**Step 3: Write minimal implementation**
- Move inventory entry selection, add/remove item helpers, equip/unequip helpers, buff ticking, effective stat helpers, item use, pickup and potion helpers into `src/game/inventory.rs`.

**Step 4: Run targeted tests**

Run: `cargo test inventory_use_potion_should_consume_turn equipment_use_should_increase_effective_stats attack_buff_consumable_should_apply_and_expire -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/game/mod.rs src/game/inventory.rs
git commit -m "refactor: extract inventory module"
```

### Task 4: Extract AI and action orchestration

**Files:**
- Create: `src/game/ai.rs`
- Create: `src/game/actions.rs`
- Modify: `src/game/mod.rs`
- Test: `src/game/ai.rs`

**Step 1: Write the failing test**
- Move or add focused tests for noise alert and flee behavior.

**Step 2: Run test to verify it fails**

Run: `cargo test monster_should_enter_alert_and_move_toward_noise low_hp_monster_should_flee_instead_of_attacking -v`
Expected: FAIL until the extracted AI methods are correctly connected.

**Step 3: Write minimal implementation**
- Move `apply_action`, inventory action dispatch, turn finishing helpers into `src/game/actions.rs`.
- Move `monster_turn`, flee decision, and AI state decay helpers into `src/game/ai.rs`.
- Replace the manual `Default` impl for `MonsterAiState` with derive-based default and collapse the nested `if` that Clippy flagged.

**Step 4: Run targeted tests**

Run: `cargo test monster_should_enter_alert_and_move_toward_noise low_hp_monster_should_flee_instead_of_attacking -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/game/mod.rs src/game/ai.rs src/game/actions.rs
git commit -m "refactor: extract action and ai modules"
```

### Task 5: Extract UI snapshot mapping and reorganize tests

**Files:**
- Create: `src/game/snapshot.rs`
- Modify: `src/game/mod.rs`
- Modify: `src/game/ui/mod.rs`
- Test: `src/game/snapshot.rs`

**Step 1: Write the failing test**
- Move or add a focused snapshot regression test for side contract and inventory view data.

**Step 2: Run test to verify it fails**

Run: `cargo test snapshot_should_include_side_contract_view -v`
Expected: FAIL until snapshot assembly is reconnected.

**Step 3: Write minimal implementation**
- Move `snapshot`, `map_rows`, `cell_view`, and inventory display mapping helpers into `src/game/snapshot.rs`.
- Keep `UiSnapshot` structure unchanged.
- Reorganize tests so they live closer to extracted modules and reduce `src/game/mod.rs` size.

**Step 4: Run targeted tests**

Run: `cargo test snapshot_should_include_side_contract_view -v`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/game/mod.rs src/game/snapshot.rs src/game/ui/mod.rs
git commit -m "refactor: extract snapshot module"
```

### Task 6: Final verification and project docs

**Files:**
- Modify: `docs/project-progress.md`
- Modify: `README.md`

**Step 1: Run the full verification suite**

Run: `cargo test`
Expected: all tests pass.

**Step 2: Run static checks**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS with no warnings.

**Step 3: Run build verification**

Run: `cargo build`
Expected: PASS.

**Step 4: Update docs**
- Record the module split and new maintenance status in `docs/project-progress.md`.
- Refresh `README.md` structure notes if needed.

**Step 5: Commit**

```bash
git add docs/project-progress.md README.md src/game
git commit -m "docs: record runtime refactor"
```

