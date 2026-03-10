# UI Clarity Improvements Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Improve map readability with category-based colors and make inventory selection clearer by grouping items by slot/category and highlighting actionable entries.

**Architecture:** Keep all gameplay rules unchanged and implement the feature as a presentation-layer enhancement. Extend snapshot view models in `src/game/snapshot.rs`, then update rendering in `src/game/ui/mod.rs` to consume richer metadata for map cells and inventory rows.

**Tech Stack:** Rust 2024, ratatui, crossterm, anyhow, cargo test, cargo build.

---

### Task 1: Add failing tests for snapshot display metadata

**Files:**
- Modify: `src/game/snapshot.rs`

**Step 1: Write the failing tests**
- Add a test that verifies visible map cells classify player, monster, ground item, trap, and door into distinct display kinds.
- Add a test that verifies inventory items classify into `Weapon`, `Armor`, `Accessory`, `Consumable`, `Quest`, and `Other` based on item effect data.
- Add a test that verifies inventory action labels match current state, especially for equipped items and protected quest items.

Suggested test names:
- `snapshot_cell_view_should_classify_visible_entities_and_tiles`
- `inventory_item_view_should_group_items_by_slot_or_usage`
- `inventory_item_view_should_expose_action_label_for_current_state`

**Step 2: Run tests to verify they fail**

Run: `cargo test snapshot_cell_view_should_classify_visible_entities_and_tiles -v`
Expected: FAIL because map cells do not yet expose a display kind.

Run: `cargo test inventory_item_view_should_group_items_by_slot_or_usage -v`
Expected: FAIL because inventory item views do not yet expose grouping metadata.

Run: `cargo test inventory_item_view_should_expose_action_label_for_current_state -v`
Expected: FAIL because inventory item views do not yet expose action labels.

**Step 3: Implement the minimal snapshot model changes**
- Add a `MapCellKind` enum to the UI snapshot types.
- Add an `InventoryGroup` enum to the UI snapshot types.
- Extend `MapCell` with a `kind` field.
- Extend `InventoryItemView` with `group` and `action_label` fields.
- Update snapshot construction helpers in `src/game/snapshot.rs` to populate the new metadata.

**Step 4: Run the focused tests to verify they pass**

Run: `cargo test snapshot_cell_view_should_classify_visible_entities_and_tiles -v`
Expected: PASS

Run: `cargo test inventory_item_view_should_group_items_by_slot_or_usage -v`
Expected: PASS

Run: `cargo test inventory_item_view_should_expose_action_label_for_current_state -v`
Expected: PASS

**Step 5: Commit**

```bash
git add src/game/snapshot.rs src/game/ui/mod.rs
git commit -m "feat: enrich ui snapshot display metadata"
```

### Task 2: Add failing tests for map color styling

**Files:**
- Modify: `src/game/ui/mod.rs`

**Step 1: Write the failing tests**
- Add tests for a small helper that maps `MapCellKind + MapTone` to a `Style`.
- Cover visible player, visible monster, visible item, visible trap, visible door, visible wall/floor, explored cells, and hidden cells.

Suggested test names:
- `map_cell_style_should_color_visible_player_and_monster_differently`
- `map_cell_style_should_keep_hidden_and_explored_cells_muted`

**Step 2: Run tests to verify they fail**

Run: `cargo test map_cell_style_should_color_visible_player_and_monster_differently -v`
Expected: FAIL because there is no category-aware style helper yet.

Run: `cargo test map_cell_style_should_keep_hidden_and_explored_cells_muted -v`
Expected: FAIL because style mapping is still embedded in `build_map_lines` and only depends on tone.

**Step 3: Implement the minimal rendering change**
- Extract a helper such as `map_cell_style(cell: MapCell) -> Style`.
- Use `tone` as the first gate: hidden stays black, explored stays dark gray.
- Use `kind` to choose color for visible cells.
- Replace the inline `match` in `build_map_lines` with the helper.

**Step 4: Run the focused tests to verify they pass**

Run: `cargo test map_cell_style_should_color_visible_player_and_monster_differently -v`
Expected: PASS

Run: `cargo test map_cell_style_should_keep_hidden_and_explored_cells_muted -v`
Expected: PASS

**Step 5: Commit**

```bash
git add src/game/ui/mod.rs
git commit -m "feat: add category-based map colors"
```

### Task 3: Add failing tests for grouped inventory rendering

**Files:**
- Modify: `src/game/ui/mod.rs`

**Step 1: Write the failing tests**
- Extract the inventory popup body-building logic into a pure helper that returns `Vec<Line<'static>>` from `UiSnapshot`.
- Add a test that verifies group headers render in the expected order.
- Add a test that verifies selected entries include the pointer marker and the action label.
- Add a test that verifies equipped items show the equipped marker in the primary row.

Suggested test names:
- `inventory_popup_lines_should_group_items_by_category_order`
- `inventory_popup_lines_should_highlight_selected_actionable_item`
- `inventory_popup_lines_should_mark_equipped_items_in_primary_row`

**Step 2: Run tests to verify they fail**

Run: `cargo test inventory_popup_lines_should_group_items_by_category_order -v`
Expected: FAIL because popup rendering is still one flat loop.

Run: `cargo test inventory_popup_lines_should_highlight_selected_actionable_item -v`
Expected: FAIL because actionable labels are not yet rendered in the main line.

Run: `cargo test inventory_popup_lines_should_mark_equipped_items_in_primary_row -v`
Expected: FAIL because the grouped line builder does not exist yet.

**Step 3: Implement the grouped inventory popup**
- Add an inventory line builder helper used by `render_inventory_popup`.
- Group items into the fixed order: weapon, armor, accessory, consumable, quest, other.
- Skip empty groups.
- Render a header line before each non-empty group.
- Render selected entries with a stronger style and pointer marker.
- Render `action_label` in the main line so players can quickly see what the selected item can do.
- Keep the secondary `attr_desc` line as lighter supporting text.

**Step 4: Run the focused tests to verify they pass**

Run: `cargo test inventory_popup_lines_should_group_items_by_category_order -v`
Expected: PASS

Run: `cargo test inventory_popup_lines_should_highlight_selected_actionable_item -v`
Expected: PASS

Run: `cargo test inventory_popup_lines_should_mark_equipped_items_in_primary_row -v`
Expected: PASS

**Step 5: Commit**

```bash
git add src/game/ui/mod.rs
git commit -m "feat: group inventory popup entries by category"
```

### Task 4: Regression checks and docs update

**Files:**
- Modify: `docs/project-progress.md`

**Step 1: Run focused gameplay-safe regressions**

Run: `cargo test inventory_use_potion_should_consume_turn -v`
Expected: PASS

Run: `cargo test equipment_use_should_increase_effective_stats -v`
Expected: PASS

Run: `cargo test inventory_unequip_should_restore_effective_stats -v`
Expected: PASS

Run: `cargo test equipped_item_cannot_be_dropped -v`
Expected: PASS

**Step 2: Run the full verification suite**

Run: `cargo test`
Expected: PASS

Run: `cargo build`
Expected: PASS

**Step 3: Manual smoke check**

Run: `cargo run -- --seed 123`
Expected:
- Visible map shows distinct colors for player, monsters, items, traps, doors, and terrain.
- Inventory popup shows grouped headers instead of one flat list.
- The selected row is obviously highlighted and shows its current action label.
- Existing keybindings (`w/s`, `Enter`, `r`, `x`, `i`, `Esc`) still behave the same.

**Step 4: Update project progress documentation**
- Add a done item describing map category colors.
- Add a done item describing grouped inventory rendering and actionable highlight cues.

**Step 5: Commit**

```bash
git add docs/project-progress.md src/game/snapshot.rs src/game/ui/mod.rs
git commit -m "feat: improve map readability and inventory clarity"
```

