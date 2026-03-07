# Contract Constraints Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add reusable side-contract constraints so `time_limit` and `stealth` contracts can succeed, fail, render in the UI, and serialize safely without changing the main victory loop.

**Architecture:** Extend `SideContract` with a constraint layer and explicit failed state, keep state transitions centralized in `src/game/contracts.rs`, and expose UI-ready status through `src/game/snapshot.rs`. Hook runtime updates through existing turn advancement and AI alert transitions so behavior lands with minimal churn outside the contract domain.

**Tech Stack:** Rust 2024, `serde`, `ratatui`, `cargo test`, `cargo clippy`, `cargo fmt`

---

### Task 1: Add contract constraint data model

**Files:**
- Modify: `src/game/mod.rs`
- Modify: `src/game/contracts.rs`
- Modify: `src/game/save.rs`
- Test: `src/game/contracts.rs`

**Step 1: Write the failing test**

Add tests in `src/game/contracts.rs` that construct a `SideContract` with:
- a `TimeLimit` constraint
- a `Stealth` constraint
- `failed` defaulting to `false`
- `failure_reason` defaulting to `None`

Assert that helper methods report active constraints and default state correctly.

**Step 2: Run test to verify it fails**

Run: `cargo test contracts::tests::side_contract_defaults_should_support_constraints -v`

Expected: FAIL because constraint fields / helpers do not exist yet.

**Step 3: Write minimal implementation**

Implement in `src/game/mod.rs`:
- `ContractConstraint`
- `ContractConstraintKind` or equivalent helper structure only if needed
- `failed` and `failure_reason` on `SideContract`
- `#[serde(default)]` on newly added fields for save compatibility

Implement in `src/game/contracts.rs`:
- small helper methods to inspect active constraints and terminal state

Keep the model minimal; do not add generalized event buses or extra enums unless directly used.

**Step 4: Run test to verify it passes**

Run: `cargo test contracts::tests::side_contract_defaults_should_support_constraints -v`

Expected: PASS

**Step 5: Commit**

Run:
`git add src/game/mod.rs src/game/contracts.rs src/game/save.rs`

`git commit -m "feat: add contract constraint model"`

### Task 2: Add time-limit failure logic

**Files:**
- Modify: `src/game/contracts.rs`
- Modify: `src/game/actions.rs`
- Test: `src/game/contracts.rs`

**Step 1: Write the failing test**

Add tests in `src/game/contracts.rs` for:
- a timed contract that succeeds before timeout
- a timed contract that fails after timeout
- a failed timed contract that no longer grants rewards

Prefer constructing a game, injecting a contract with `start_turn` and `max_turns`, then advancing turns through existing action flow.

**Step 2: Run test to verify it fails**

Run: `cargo test contracts::tests::time_limited_contract_should_fail_after_deadline -v`

Expected: FAIL because deadline ticking is not implemented.

**Step 3: Write minimal implementation**

Implement in `src/game/contracts.rs`:
- `tick_contract_constraints(current_turn)`
- `fail_side_contract(reason)`
- `can_progress_side_contract()`

Wire it from `src/game/actions.rs` after player turn advancement, reusing existing turn flow.

Log one clear failure line when timeout happens.

**Step 4: Run test to verify it passes**

Run: `cargo test contracts::tests::time_limited_contract_should_fail_after_deadline -v`

Expected: PASS

**Step 5: Commit**

Run:
`git add src/game/contracts.rs src/game/actions.rs`

`git commit -m "feat: add timed contract failures"`

### Task 3: Add stealth failure logic

**Files:**
- Modify: `src/game/contracts.rs`
- Modify: `src/game/ai.rs`
- Test: `src/game/contracts.rs`
- Test: `src/game/ai.rs`

**Step 1: Write the failing test**

Add tests covering:
- stealth contract fails when a monster enters `Alert`
- failed stealth contract no longer progresses

Prefer one contract-level test and one AI integration test.

**Step 2: Run test to verify it fails**

Run: `cargo test stealth_contract_should_fail_when_monster_enters_alert -v`

Expected: FAIL because AI does not notify the contract system yet.

**Step 3: Write minimal implementation**

Implement in `src/game/contracts.rs`:
- `on_contract_alert_triggered()`

Wire it in `src/game/ai.rs` at the point where a monster transitions into `MonsterAiState::Alert`.

Ensure the transition is only reported once for the actual state change, not every alert tick.

**Step 4: Run test to verify it passes**

Run: `cargo test stealth_contract_should_fail_when_monster_enters_alert -v`

Expected: PASS

**Step 5: Commit**

Run:
`git add src/game/contracts.rs src/game/ai.rs`

`git commit -m "feat: add stealth contract failures"`

### Task 4: Allow collect contracts to use constraints

**Files:**
- Modify: `src/game/contracts.rs`
- Modify: `src/game/inventory.rs`
- Test: `src/game/contracts.rs`

**Step 1: Write the failing test**

Add tests for:
- `CollectItem` contract with `TimeLimit`
- `CollectItem` contract with `Stealth`
- `CollectItem` contract with both constraints

Verify:
- success path still grants reward
- failed contract does not complete even if item is collected later

**Step 2: Run test to verify it fails**

Run: `cargo test collect_contract_with_constraints_should_only_reward_on_success -v`

Expected: FAIL because progress and reward gating still ignore failure state.

**Step 3: Write minimal implementation**

Adjust `src/game/contracts.rs` so progress/reward paths all guard on `can_progress_side_contract()`.

Only touch `src/game/inventory.rs` if pickup flow needs a direct helper call to keep contract progress logic centralized.

**Step 4: Run test to verify it passes**

Run: `cargo test collect_contract_with_constraints_should_only_reward_on_success -v`

Expected: PASS

**Step 5: Commit**

Run:
`git add src/game/contracts.rs src/game/inventory.rs`

`git commit -m "feat: gate collect contracts by constraints"`

### Task 5: Expose constraint status in snapshots

**Files:**
- Modify: `src/game/snapshot.rs`
- Modify: `src/game/ui/mod.rs`
- Test: `src/game/snapshot.rs`
- Test: `src/game/ui/mod.rs`

**Step 1: Write the failing test**

Add snapshot tests for:
- active timed contract shows remaining turns
- active stealth contract shows `未暴露`
- failed contract shows failure status and message

Add UI test if needed to ensure panel lines include constraint status.

**Step 2: Run test to verify it fails**

Run: `cargo test snapshot_should_include_contract_constraint_status -v`

Expected: FAIL because current snapshot view does not expose those fields.

**Step 3: Write minimal implementation**

Extend `SideContractView` to include:
- `status_text`
- `constraint_lines`
- optional `failure_reason`

Render these lines in the existing side-contract panel without redesigning the whole sidebar.

**Step 4: Run test to verify it passes**

Run: `cargo test snapshot_should_include_contract_constraint_status -v`

Expected: PASS

**Step 5: Commit**

Run:
`git add src/game/snapshot.rs src/game/ui/mod.rs`

`git commit -m "feat: show contract constraint status"`

### Task 6: Generate constrained contracts

**Files:**
- Modify: `src/game/contracts.rs`
- Test: `src/game/contracts.rs`

**Step 1: Write the failing test**

Add tests ensuring generated side contracts can include:
- timed collect contract
- stealth collect contract
- dual-constraint collect contract

Avoid asserting exact RNG sequence beyond what is needed; prefer seeding and checking allowed shapes.

**Step 2: Run test to verify it fails**

Run: `cargo test generated_side_contract_should_include_supported_constraints -v`

Expected: FAIL because generation does not assign new constraints yet.

**Step 3: Write minimal implementation**

Update existing side-contract generation to occasionally attach:
- `TimeLimit`
- `Stealth`
- both together

Keep scope intentionally small and limited to collect contracts in this batch.

**Step 4: Run test to verify it passes**

Run: `cargo test generated_side_contract_should_include_supported_constraints -v`

Expected: PASS

**Step 5: Commit**

Run:
`git add src/game/contracts.rs`

`git commit -m "feat: generate constrained contracts"`

### Task 7: Full verification and docs update

**Files:**
- Modify: `README.md`
- Modify: `docs/project-progress.md`
- Modify: `docs/plans/2026-03-07-contract-constraints-design.md` (only if implementation diverged)

**Step 1: Update docs**

Document:
- new contract types
- failure semantics
- any new UI status wording

**Step 2: Run formatting and verification**

Run:
- `cargo fmt`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo build`

Expected:
- all commands succeed
- all tests pass

**Step 3: Inspect git diff**

Run: `git diff --stat`

Expected: only contract-constraint feature files and doc updates are present.

**Step 4: Commit**

Run:
`git add README.md docs/project-progress.md src/game/actions.rs src/game/ai.rs src/game/contracts.rs src/game/inventory.rs src/game/mod.rs src/game/save.rs src/game/snapshot.rs src/game/ui/mod.rs`

`git commit -m "feat: add advanced side contract constraints"`

**Step 5: Push**

Run:
`git push`
