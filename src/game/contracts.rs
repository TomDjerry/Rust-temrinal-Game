use super::*;

impl SideContract {
    pub(super) fn target(&self) -> u32 {
        match self.objective {
            ContractObjective::KillMonsters { target } => target,
            ContractObjective::CollectItem { target, .. } => target,
        }
    }

    pub(super) fn has_time_limit(&self) -> bool {
        self.constraints
            .iter()
            .any(|constraint| matches!(constraint, ContractConstraint::TimeLimit { .. }))
    }

    pub(super) fn has_stealth_requirement(&self) -> bool {
        self.constraints
            .iter()
            .any(|constraint| matches!(constraint, ContractConstraint::Stealth { .. }))
    }

    pub(super) fn is_terminal(&self) -> bool {
        self.completed || self.failed
    }

    pub(super) fn remaining_turns(&self, current_turn: u32) -> Option<i32> {
        self.constraints
            .iter()
            .find_map(|constraint| match constraint {
                ContractConstraint::TimeLimit {
                    start_turn,
                    max_turns,
                } => Some(*max_turns as i32 - current_turn.saturating_sub(*start_turn) as i32),
                ContractConstraint::Stealth { .. } => None,
            })
    }
}

impl Game {
    fn generated_collect_contract_constraints(&mut self) -> Vec<ContractConstraint> {
        match self.rng.random_range(0..3) {
            0 => vec![ContractConstraint::TimeLimit {
                start_turn: self.turn,
                max_turns: 8,
            }],
            1 => vec![ContractConstraint::Stealth { exposed: false }],
            _ => vec![
                ContractConstraint::TimeLimit {
                    start_turn: self.turn,
                    max_turns: 10,
                },
                ContractConstraint::Stealth { exposed: false },
            ],
        }
    }

    pub(super) fn ensure_side_contract(&mut self, announce: bool) {
        if self.side_contract.is_some() {
            return;
        }

        let contract = if self.rng.random_bool(0.5) {
            SideContract {
                name: "清剿威胁".to_string(),
                objective: ContractObjective::KillMonsters { target: 3 },
                progress: 0,
                reward_item_id: "battle_tonic".to_string(),
                reward_qty: 1,
                completed: false,
                constraints: Vec::new(),
                failed: false,
                failure_reason: None,
            }
        } else {
            SideContract {
                name: "药剂补给".to_string(),
                objective: ContractObjective::CollectItem {
                    item_id: "healing_potion".to_string(),
                    target: 2,
                },
                progress: 0,
                reward_item_id: "iron_skin_tonic".to_string(),
                reward_qty: 1,
                completed: false,
                constraints: self.generated_collect_contract_constraints(),
                failed: false,
                failure_reason: None,
            }
        };

        if announce {
            self.push_log(format!("新增支线合约: {}", contract.name));
        }
        self.side_contract = Some(contract);
    }

    pub(super) fn required_quest_item_ids(&self) -> Vec<String> {
        let mut ids = self
            .data
            .item_defs
            .values()
            .filter_map(|def| match def.effect {
                ItemEffectDef::QuestItem {
                    required_for_delivery: true,
                } => Some(def.id.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        ids.sort();
        ids
    }

    pub(super) fn required_quest_progress(&self) -> (usize, usize) {
        let required = self.required_quest_item_ids();
        let collected = required
            .iter()
            .filter(|id| self.player.has_item(id))
            .count();
        (collected, required.len())
    }

    pub(super) fn collected_required_quest_item_count(&self) -> usize {
        self.required_quest_progress().0
    }

    pub(super) fn has_all_required_quest_items(&self) -> bool {
        let (collected, total) = self.required_quest_progress();
        collected == total
    }

    pub(super) fn side_contract_progress_line(&self) -> Option<String> {
        self.side_contract.as_ref().map(|contract| {
            if contract.completed {
                format!("支线合约 {}: 已完成", contract.name)
            } else {
                format!(
                    "支线合约 {}: {}/{}",
                    contract.name,
                    contract.progress,
                    contract.target()
                )
            }
        })
    }

    pub(super) fn side_contract_view(&self) -> Option<SideContractView> {
        self.side_contract.as_ref().map(|contract| {
            let target = contract.target();
            let progress = contract.progress.min(target);
            let status_text = if contract.failed {
                "已失败".to_string()
            } else if contract.completed {
                "已完成".to_string()
            } else {
                "进行中".to_string()
            };
            let mut constraint_lines = Vec::new();
            if contract.has_time_limit()
                && let Some(remaining) = contract.remaining_turns(self.turn)
            {
                if remaining < 0 {
                    constraint_lines.push("剩余: 已超时".to_string());
                } else {
                    constraint_lines.push(format!("剩余: {remaining} 回合"));
                }
            }
            if contract.has_stealth_requirement()
                && let Some(exposed) =
                    contract
                        .constraints
                        .iter()
                        .find_map(|constraint| match constraint {
                            ContractConstraint::Stealth { exposed } => Some(*exposed),
                            ContractConstraint::TimeLimit { .. } => None,
                        })
            {
                constraint_lines.push(if exposed {
                    "潜行: 已失败".to_string()
                } else {
                    "潜行: 未暴露".to_string()
                });
            }
            SideContractView {
                name: contract.name.clone(),
                objective: self.side_contract_objective_text(contract),
                progress_text: format!("{progress}/{target}"),
                reward_text: self.side_contract_reward_text(contract),
                completed: contract.completed,
                status_text,
                constraint_lines,
                failure_reason: contract.failure_reason.clone(),
            }
        })
    }

    fn side_contract_objective_text(&self, contract: &SideContract) -> String {
        match &contract.objective {
            ContractObjective::KillMonsters { .. } => "击杀怪物".to_string(),
            ContractObjective::CollectItem { item_id, .. } => {
                let item_name = self
                    .data
                    .item_defs
                    .get(item_id)
                    .map(|item| item.name.as_str())
                    .unwrap_or(item_id.as_str());
                format!("收集 {item_name}")
            }
        }
    }

    fn side_contract_reward_text(&self, contract: &SideContract) -> String {
        let reward_name = self
            .data
            .item_defs
            .get(&contract.reward_item_id)
            .map(|item| item.name.as_str())
            .unwrap_or(contract.reward_item_id.as_str());
        format!("{reward_name} x{}", contract.reward_qty)
    }

    pub(super) fn on_monster_killed_for_contract(&mut self) {
        let mut progress_log: Option<String> = None;
        if let Some(contract) = &mut self.side_contract
            && !contract.is_terminal()
            && matches!(contract.objective, ContractObjective::KillMonsters { .. })
        {
            contract.progress = contract.progress.saturating_add(1);
            progress_log = Some(format!(
                "支线合约 {}: {}/{}",
                contract.name,
                contract.progress,
                contract.target()
            ));
        }
        if let Some(line) = progress_log {
            self.push_log(line);
        }
        if self
            .side_contract
            .as_ref()
            .is_some_and(|contract| !contract.is_terminal())
        {
            self.try_complete_side_contract();
        }
    }

    pub(super) fn on_item_collected_for_contract(&mut self, item_id: &str, qty: u32) {
        if qty == 0 {
            return;
        }
        let mut progress_log: Option<String> = None;
        if let Some(contract) = &mut self.side_contract {
            if contract.is_terminal() {
                return;
            }
            if let ContractObjective::CollectItem {
                item_id: target_item_id,
                target: _,
            } = &contract.objective
                && target_item_id == item_id
            {
                contract.progress = contract.progress.saturating_add(qty);
                progress_log = Some(format!(
                    "支线合约 {}: {}/{}",
                    contract.name,
                    contract.progress,
                    contract.target()
                ));
            }
        }
        if let Some(line) = progress_log {
            self.push_log(line);
            self.try_complete_side_contract();
        }
    }

    pub(super) fn try_complete_side_contract(&mut self) {
        let mut reward: Option<(String, u32, String)> = None;
        if let Some(contract) = &mut self.side_contract {
            if contract.is_terminal() {
                return;
            }
            let target = contract.target();
            if contract.progress >= target {
                contract.progress = target;
                contract.completed = true;
                reward = Some((
                    contract.reward_item_id.clone(),
                    contract.reward_qty,
                    contract.name.clone(),
                ));
            }
        }

        let Some((reward_item_id, reward_qty, contract_name)) = reward else {
            return;
        };
        let added = self.add_item_to_inventory(&reward_item_id, reward_qty);
        if added > 0 {
            let reward_name = self
                .data
                .item_defs
                .get(&reward_item_id)
                .map(|item| item.name.clone())
                .unwrap_or(reward_item_id);
            self.push_log(format!(
                "支线合约完成: {contract_name}，获得 {reward_name} x{added}"
            ));
        } else {
            self.push_log(format!("支线合约完成: {contract_name}，但奖励未能放入背包"));
        }
    }

    pub(super) fn can_progress_side_contract(&self) -> bool {
        self.side_contract
            .as_ref()
            .is_some_and(|contract| !contract.is_terminal())
    }

    pub(super) fn on_contract_alert_triggered(&mut self) {
        let mut should_fail = false;
        if let Some(contract) = &mut self.side_contract {
            if contract.is_terminal() {
                return;
            }
            for constraint in &mut contract.constraints {
                if let ContractConstraint::Stealth { exposed } = constraint
                    && !*exposed
                {
                    *exposed = true;
                    should_fail = true;
                }
            }
        }
        if should_fail {
            self.fail_side_contract("stealth failed: alerted");
        }
    }

    pub(super) fn fail_side_contract(&mut self, reason: impl Into<String>) {
        let reason = reason.into();
        let mut contract_name = None;
        if let Some(contract) = &mut self.side_contract {
            if contract.is_terminal() {
                return;
            }
            contract.failed = true;
            contract.failure_reason = Some(reason.clone());
            contract_name = Some(contract.name.clone());
        }
        if let Some(contract_name) = contract_name {
            self.push_log(format!("contract failed: {contract_name} - {reason}"));
        }
    }

    pub(super) fn tick_contract_constraints(&mut self) {
        let should_fail = self.side_contract.as_ref().is_some_and(|contract| {
            !contract.is_terminal()
                && contract
                    .remaining_turns(self.turn)
                    .is_some_and(|remaining| remaining < 0)
        });

        if should_fail {
            self.fail_side_contract("time limit exceeded");
        }
    }

    pub(super) fn missing_required_quest_item_names(&self) -> Vec<String> {
        self.required_quest_item_ids()
            .into_iter()
            .filter(|id| !self.player.has_item(id))
            .map(|id| {
                self.data
                    .item_defs
                    .get(&id)
                    .map(|def| def.name.clone())
                    .unwrap_or(id)
            })
            .collect()
    }

    pub(super) fn log_required_quest_progress(&mut self) {
        let (collected, total) = self.required_quest_progress();
        self.push_log(format!("必需任务物进度: {collected}/{total}"));
        if let Some(line) = self.side_contract_progress_line() {
            self.push_log(line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::build_test_game;
    use super::super::*;

    #[test]
    fn side_contract_target_should_match_objective() {
        let kill_contract = SideContract {
            name: "测试击杀".to_string(),
            objective: ContractObjective::KillMonsters { target: 3 },
            progress: 0,
            reward_item_id: "battle_tonic".to_string(),
            reward_qty: 1,
            completed: false,
            constraints: Vec::new(),
            failed: false,
            failure_reason: None,
        };
        let collect_contract = SideContract {
            name: "测试收集".to_string(),
            objective: ContractObjective::CollectItem {
                item_id: "healing_potion".to_string(),
                target: 2,
            },
            progress: 0,
            reward_item_id: "iron_skin_tonic".to_string(),
            reward_qty: 1,
            completed: false,
            constraints: Vec::new(),
            failed: false,
            failure_reason: None,
        };

        assert_eq!(kill_contract.target(), 3);
        assert_eq!(collect_contract.target(), 2);
    }

    #[test]
    fn required_quest_progress_should_count_only_required_items() {
        let mut game = build_test_game(51);
        game.monsters.clear();

        assert_eq!(game.required_quest_progress(), (0, 1));

        let _ = game.add_item_to_inventory("courier_badge", 1);
        assert_eq!(game.required_quest_progress(), (0, 1));

        let _ = game.add_item_to_inventory("delivery_note", 1);
        assert_eq!(game.required_quest_progress(), (1, 1));
    }

    #[test]
    fn side_contract_defaults_should_support_constraints() {
        let timed = SideContract {
            name: "timed test".to_string(),
            objective: ContractObjective::CollectItem {
                item_id: "healing_potion".to_string(),
                target: 2,
            },
            progress: 0,
            reward_item_id: "iron_skin_tonic".to_string(),
            reward_qty: 1,
            completed: false,
            constraints: vec![ContractConstraint::TimeLimit {
                start_turn: 3,
                max_turns: 8,
            }],
            failed: false,
            failure_reason: None,
        };
        let stealth = SideContract {
            name: "stealth test".to_string(),
            objective: ContractObjective::CollectItem {
                item_id: "healing_potion".to_string(),
                target: 1,
            },
            progress: 0,
            reward_item_id: "battle_tonic".to_string(),
            reward_qty: 1,
            completed: false,
            constraints: vec![ContractConstraint::Stealth { exposed: false }],
            failed: false,
            failure_reason: None,
        };

        assert!(timed.has_time_limit());
        assert!(!timed.has_stealth_requirement());
        assert!(stealth.has_stealth_requirement());
        assert!(!stealth.has_time_limit());
        assert!(!timed.is_terminal());
        assert_eq!(timed.failure_reason.as_deref(), None);
    }

    #[test]
    fn time_limited_contract_should_fail_after_deadline() {
        let mut game = build_test_game(81);
        game.monsters.clear();
        game.side_contract = Some(SideContract {
            name: "timed collect".to_string(),
            objective: ContractObjective::CollectItem {
                item_id: "healing_potion".to_string(),
                target: 1,
            },
            progress: 0,
            reward_item_id: "iron_skin_tonic".to_string(),
            reward_qty: 1,
            completed: false,
            constraints: vec![ContractConstraint::TimeLimit {
                start_turn: game.turn,
                max_turns: 1,
            }],
            failed: false,
            failure_reason: None,
        });

        game.apply_action(Action::Wait);
        assert!(!game.side_contract.as_ref().expect("contract").failed);

        game.apply_action(Action::Wait);

        let contract = game.side_contract.as_ref().expect("contract");
        assert!(contract.failed);
        assert!(!contract.completed);
        assert_eq!(
            contract.failure_reason.as_deref(),
            Some("time limit exceeded")
        );
    }

    #[test]
    fn time_limited_contract_should_still_reward_before_deadline() {
        let mut game = build_test_game(82);
        game.monsters.clear();
        game.side_contract = Some(SideContract {
            name: "timed collect".to_string(),
            objective: ContractObjective::CollectItem {
                item_id: "healing_potion".to_string(),
                target: 1,
            },
            progress: 0,
            reward_item_id: "iron_skin_tonic".to_string(),
            reward_qty: 1,
            completed: false,
            constraints: vec![ContractConstraint::TimeLimit {
                start_turn: game.turn,
                max_turns: 1,
            }],
            failed: false,
            failure_reason: None,
        });

        game.apply_action(Action::Wait);
        game.on_item_collected_for_contract("healing_potion", 1);

        let contract = game.side_contract.as_ref().expect("contract");
        assert!(contract.completed);
        assert!(!contract.failed);
        assert_eq!(game.player.item_count("iron_skin_tonic"), 1);
    }

    #[test]
    fn failed_time_limited_contract_should_stop_progress_and_reward() {
        let mut game = build_test_game(83);
        game.monsters.clear();
        game.side_contract = Some(SideContract {
            name: "timed collect".to_string(),
            objective: ContractObjective::CollectItem {
                item_id: "healing_potion".to_string(),
                target: 1,
            },
            progress: 0,
            reward_item_id: "iron_skin_tonic".to_string(),
            reward_qty: 1,
            completed: false,
            constraints: vec![ContractConstraint::TimeLimit {
                start_turn: game.turn,
                max_turns: 0,
            }],
            failed: false,
            failure_reason: None,
        });

        game.apply_action(Action::Wait);
        game.on_item_collected_for_contract("healing_potion", 1);

        let contract = game.side_contract.as_ref().expect("contract");
        assert!(contract.failed);
        assert!(!contract.completed);
        assert_eq!(contract.progress, 0);
        assert_eq!(game.player.item_count("iron_skin_tonic"), 0);
    }

    #[test]
    fn stealth_contract_should_fail_when_alert_triggered() {
        let mut game = build_test_game(84);
        game.monsters.clear();
        game.side_contract = Some(SideContract {
            name: "stealth collect".to_string(),
            objective: ContractObjective::CollectItem {
                item_id: "healing_potion".to_string(),
                target: 1,
            },
            progress: 0,
            reward_item_id: "battle_tonic".to_string(),
            reward_qty: 1,
            completed: false,
            constraints: vec![ContractConstraint::Stealth { exposed: false }],
            failed: false,
            failure_reason: None,
        });

        game.on_contract_alert_triggered();

        let contract = game.side_contract.as_ref().expect("contract");
        assert!(contract.failed);
        assert_eq!(
            contract.failure_reason.as_deref(),
            Some("stealth failed: alerted")
        );
        assert!(matches!(
            contract.constraints.first(),
            Some(ContractConstraint::Stealth { exposed: true })
        ));
    }

    #[test]
    fn generated_side_contract_should_include_supported_constraints() {
        let mut game = build_test_game(85);
        game.monsters.clear();

        let mut saw_time_limit = false;
        let mut saw_stealth = false;
        let mut saw_dual = false;

        for _ in 0..256 {
            game.side_contract = None;
            game.ensure_side_contract(false);

            let contract = game.side_contract.as_ref().expect("contract");
            if !matches!(contract.objective, ContractObjective::CollectItem { .. }) {
                continue;
            }

            let has_time_limit = contract.has_time_limit();
            let has_stealth = contract.has_stealth_requirement();
            saw_time_limit |= has_time_limit;
            saw_stealth |= has_stealth;
            saw_dual |= has_time_limit && has_stealth;

            if saw_time_limit && saw_stealth && saw_dual {
                break;
            }
        }

        assert!(
            saw_time_limit,
            "expected generated collect contract with time limit"
        );
        assert!(
            saw_stealth,
            "expected generated collect contract with stealth"
        );
        assert!(
            saw_dual,
            "expected generated collect contract with both constraints"
        );
    }

    #[test]
    fn victory_should_require_required_quest_items() {
        let mut game = build_test_game(21);
        game.monsters.clear();
        game.player.pos = game.exit_pos;
        let _ = game.add_item_to_inventory("package", 1);

        game.check_victory();
        assert!(!game.won);

        let _ = game.add_item_to_inventory("delivery_note", 1);
        game.check_victory();
        assert!(game.won);
    }

    #[test]
    fn map_should_spawn_required_quest_item() {
        let game = build_test_game(22);

        assert!(
            game.ground_items
                .iter()
                .any(|item| item.item_id == "delivery_note")
        );
    }

    #[test]
    fn reaching_exit_without_required_items_should_log_missing_progress() {
        let mut game = build_test_game(27);
        game.monsters.clear();
        let _ = game.add_item_to_inventory("package", 1);
        game.player.pos = game.exit_pos;

        game.check_victory();

        assert!(!game.won);
        let last_log = game.log.back().cloned().unwrap_or_default();
        assert!(last_log.contains("0/1"));
    }

    #[test]
    fn picking_required_item_should_log_quest_progress() {
        let mut game = build_test_game(28);
        game.monsters.clear();
        game.ground_items.push(GroundItem {
            item_id: "delivery_note".to_string(),
            pos: game.player.pos,
        });

        let picked = game.try_pickup();

        assert!(picked);
        assert!(game.log.iter().any(|line| line.contains("1/1")));
    }

    #[test]
    fn kill_contract_should_complete_and_grant_reward() {
        let mut game = build_test_game(29);
        game.monsters.clear();
        game.side_contract = Some(SideContract {
            name: "kill test".to_string(),
            objective: ContractObjective::KillMonsters { target: 1 },
            progress: 0,
            reward_item_id: "battle_tonic".to_string(),
            reward_qty: 1,
            completed: false,
            constraints: Vec::new(),
            failed: false,
            failure_reason: None,
        });

        game.on_monster_killed_for_contract();

        let contract = game.side_contract.as_ref().expect("contract");
        assert!(contract.completed);
        assert_eq!(contract.progress, 1);
        assert_eq!(game.player.item_count("battle_tonic"), 1);
    }

    #[test]
    fn collect_contract_should_progress_on_pickup_and_grant_reward() {
        let mut game = build_test_game(30);
        game.monsters.clear();
        game.side_contract = Some(SideContract {
            name: "collect test".to_string(),
            objective: ContractObjective::CollectItem {
                item_id: "healing_potion".to_string(),
                target: 1,
            },
            progress: 0,
            reward_item_id: "iron_skin_tonic".to_string(),
            reward_qty: 1,
            completed: false,
            constraints: Vec::new(),
            failed: false,
            failure_reason: None,
        });
        game.ground_items.push(GroundItem {
            item_id: "healing_potion".to_string(),
            pos: game.player.pos,
        });

        let picked = game.try_pickup();
        assert!(picked);

        let contract = game.side_contract.as_ref().expect("contract");
        assert!(contract.completed);
        assert_eq!(contract.progress, 1);
        assert_eq!(game.player.item_count("iron_skin_tonic"), 1);
    }
}
