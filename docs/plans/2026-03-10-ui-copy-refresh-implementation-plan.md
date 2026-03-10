# UI Copy Refresh Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refresh the in-game UI copy to a clearer modern Chinese game-UI style without changing behavior, layout, or controls.

**Architecture:** Limit changes to the presentation layer in `src/game/ui/mod.rs`. Update static labels and help text, then adjust the small set of UI tests that assert exact rendered strings.

**Tech Stack:** Rust 2024, ratatui, crossterm, cargo fmt, cargo test, cargo build.

---

### Task 1: Write failing UI text tests first

**Files:**
- Modify: `src/game/ui/mod.rs`
- Test: `src/game/ui/mod.rs`

**Step 1: Write the failing tests**
- Update the side-contract text assertions to the approved Chinese copy.
- Update the inventory line assertions to the approved Chinese equipped/action labels.
- Add or adjust assertions for renamed section titles if a pure helper is available.

**Step 2: Run tests to verify they fail**

Run: `cargo test side_contract_panel_lines_should_include_details -v`
Expected: FAIL because the approved wording is not implemented yet.

Run: `cargo test side_contract_panel_lines_should_include_failure_reason -v`
Expected: FAIL because the approved wording is not implemented yet.

Run: `cargo test inventory_popup_lines_should_mark_equipped_items_in_primary_row -v`
Expected: FAIL because the approved wording is not implemented yet.

**Step 3: Implement the minimal text updates**
- Replace UI titles and static labels in `src/game/ui/mod.rs`.
- Update help popup text to the approved wording.
- Update log popup footer text to the approved wording.
- Keep layout, grouping, styling, and key handling unchanged.

**Step 4: Run the focused tests to verify they pass**

Run: `cargo test side_contract_panel_lines_should_include_details -v`
Expected: PASS

Run: `cargo test side_contract_panel_lines_should_include_failure_reason -v`
Expected: PASS

Run: `cargo test inventory_popup_lines_should_mark_equipped_items_in_primary_row -v`
Expected: PASS

**Step 5: Commit**

```bash
git add src/game/ui/mod.rs
git commit -m "feat: polish ui copy for readability"
```

### Task 2: Verify no behavior regression

**Files:**
- Modify: `src/game/ui/mod.rs`
- Test: `src/game/ui/mod.rs`
- Test: `src/game/snapshot.rs`

**Step 1: Run focused UI regression tests**

Run: `cargo test inventory_popup_lines_should_group_items_by_category_order -v`
Expected: PASS

Run: `cargo test inventory_popup_lines_should_highlight_selected_actionable_item -v`
Expected: PASS

Run: `cargo test map_cell_style_should_color_visible_player_and_monster_differently -v`
Expected: PASS

Run: `cargo test map_cell_style_should_keep_hidden_and_explored_cells_muted -v`
Expected: PASS

**Step 2: Run broader inventory/snapshot regressions**

Run: `cargo test inventory_item_view_should_group_items_by_slot_or_usage -v`
Expected: PASS

Run: `cargo test inventory_item_view_should_expose_action_label_for_current_state -v`
Expected: PASS

Run: `cargo test snapshot_cell_view_should_classify_visible_entities_and_tiles -v`
Expected: PASS

**Step 3: Commit**

```bash
git add src/game/ui/mod.rs src/game/snapshot.rs
git commit -m "test: confirm ui copy refresh keeps behavior stable"
```

### Task 3: Final verification and docs sync

**Files:**
- Modify: `docs/project-progress.md`

**Step 1: Update project progress notes**
- Add a done item for UI copy refresh.
- Update the latest verification section if this work is part of the current batch.

**Step 2: Run formatting and full verification**

Run: `cargo fmt`
Expected: PASS

Run: `cargo test`
Expected: PASS

Run: `cargo build`
Expected: PASS

**Step 3: Manual smoke check**

Run: `cargo run -- --seed 123`
Expected:
- Sidebar titles and status wording match the approved modern Chinese copy.
- Inventory popup labels are easier to read and consistent with the rest of the UI.
- Help and log popups use the approved concise wording.

**Step 4: Commit**

```bash
git add docs/project-progress.md src/game/ui/mod.rs
git commit -m "docs: record ui copy refresh"
```
