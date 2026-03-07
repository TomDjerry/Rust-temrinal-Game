# Maintenance Refactor Implementation Plan

**Goal:** Reduce module size and improve maintainability without changing gameplay or save behavior.

**Scope:**
- Split `src/game/contracts.rs` into generation and runtime-focused submodules.
- Split `src/game/inventory.rs` into operations, equipment, and buffs submodules.
- Replace contract generation magic numbers with named constants.
- Replace the equipment bonus tuple with a named struct instead of introducing a trait.

**Non-Goals:**
- No main-loop behavior changes.
- No contract rule changes.
- No save format changes.
- No new abstraction trait unless a second implementation appears.

### Task 1: Split `contracts`
- Add `src/game/contracts/contract_generation.rs`
- Add `src/game/contracts/contract_runtime.rs`
- Keep tests in `src/game/contracts.rs`
- Verify with targeted contract and snapshot tests

### Task 2: Split `inventory`
- Add `src/game/inventory/inventory_operations.rs`
- Add `src/game/inventory/equipment.rs`
- Add `src/game/inventory/buffs.rs`
- Keep tests in `src/game/inventory.rs`
- Verify with targeted inventory and combat-adjacent tests

### Task 3: Clean constants and return types
- Introduce named constants for generated contract time limits
- Introduce `EquipmentBonuses` as a named internal struct
- Keep external APIs unchanged

### Task 4: Full verification
- Run `cargo fmt`
- Run `cargo test`
- Run `cargo clippy --all-targets --all-features -- -D warnings`
- Run `cargo build`

### Task 5: Commit and push
- Commit only refactor-related files
- Push after verification passes
