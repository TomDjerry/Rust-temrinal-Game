use super::*;

impl SideContract {
    pub(super) fn target(&self) -> u32 {
        match self.objective {
            ContractObjective::KillMonsters { target } => target,
            ContractObjective::CollectItem { target, .. } => target,
        }
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
    use super::super::*;

    fn build_test_game(seed: u64) -> Game {
        let data = GameData::load("assets").expect("assets");
        let config = GameConfig {
            seed: Some(seed),
            width: 40,
            height: 22,
        };
        Game::new(config, seed, data).expect("game")
    }

    #[test]
    fn side_contract_target_should_match_objective() {
        let kill_contract = SideContract {
            name: "测试击杀".to_string(),
            objective: ContractObjective::KillMonsters { target: 3 },
            progress: 0,
            reward_item_id: "battle_tonic".to_string(),
            reward_qty: 1,
            completed: false,
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
}
