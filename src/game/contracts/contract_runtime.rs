use super::super::*;

impl SideContract {
    pub(in crate::game) fn target(&self) -> u32 {
        match self.objective {
            ContractObjective::KillMonsters { target } => target,
            ContractObjective::CollectItem { target, .. } => target,
        }
    }

    pub(in crate::game) fn has_time_limit(&self) -> bool {
        self.constraints
            .iter()
            .any(|constraint| matches!(constraint, ContractConstraint::TimeLimit { .. }))
    }

    pub(in crate::game) fn has_stealth_requirement(&self) -> bool {
        self.constraints
            .iter()
            .any(|constraint| matches!(constraint, ContractConstraint::Stealth { .. }))
    }

    pub(in crate::game) fn is_terminal(&self) -> bool {
        self.completed || self.failed
    }

    pub(in crate::game) fn remaining_turns(&self, current_turn: u32) -> Option<i32> {
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
    pub(in crate::game) fn required_quest_item_ids(&self) -> Vec<String> {
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

    pub(in crate::game) fn required_quest_progress(&self) -> (usize, usize) {
        let required = self.required_quest_item_ids();
        let collected = required
            .iter()
            .filter(|id| self.player.has_item(id))
            .count();
        (collected, required.len())
    }

    pub(in crate::game) fn collected_required_quest_item_count(&self) -> usize {
        self.required_quest_progress().0
    }

    pub(in crate::game) fn has_all_required_quest_items(&self) -> bool {
        let (collected, total) = self.required_quest_progress();
        collected == total
    }

    pub(in crate::game) fn side_contract_progress_line(&self) -> Option<String> {
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

    pub(in crate::game) fn side_contract_view(&self) -> Option<SideContractView> {
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

    pub(in crate::game) fn on_monster_killed_for_contract(&mut self) {
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

    pub(in crate::game) fn on_item_collected_for_contract(&mut self, item_id: &str, qty: u32) {
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

    pub(in crate::game) fn try_complete_side_contract(&mut self) {
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

    pub(in crate::game) fn can_progress_side_contract(&self) -> bool {
        self.side_contract
            .as_ref()
            .is_some_and(|contract| !contract.is_terminal())
    }

    pub(in crate::game) fn on_contract_alert_triggered(&mut self) {
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

    pub(in crate::game) fn fail_side_contract(&mut self, reason: impl Into<String>) {
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

    pub(in crate::game) fn tick_contract_constraints(&mut self) {
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

    pub(in crate::game) fn missing_required_quest_item_names(&self) -> Vec<String> {
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

    pub(in crate::game) fn log_required_quest_progress(&mut self) {
        let (collected, total) = self.required_quest_progress();
        self.push_log(format!("必需任务物进度: {collected}/{total}"));
        if let Some(line) = self.side_contract_progress_line() {
            self.push_log(line);
        }
    }
}
