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
}

impl Game {
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
                constraints: Vec::new(),
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
            SideContractView {
                name: contract.name.clone(),
                objective: self.side_contract_objective_text(contract),
                progress_text: format!("{progress}/{target}"),
                reward_text: self.side_contract_reward_text(contract),
                completed: contract.completed,
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
            && !contract.completed
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
            .is_some_and(|contract| !contract.completed)
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
            if contract.completed {
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
            if contract.completed {
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
