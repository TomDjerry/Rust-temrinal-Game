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
                format!(
                    "\u{652F}\u{7EBF}\u{59D4}\u{6258} {}\u{FF1A}\u{5DF2}\u{5B8C}\u{6210}",
                    contract.name
                )
            } else if contract.failed {
                format!(
                    "\u{652F}\u{7EBF}\u{59D4}\u{6258} {}\u{FF1A}\u{5DF2}\u{5931}\u{8D25}",
                    contract.name
                )
            } else {
                format!(
                    "\u{652F}\u{7EBF}\u{59D4}\u{6258} {}\u{FF1A}{}/{}",
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
                "\u{5DF2}\u{5931}\u{8D25}".to_string()
            } else if contract.completed {
                "\u{5DF2}\u{5B8C}\u{6210}".to_string()
            } else {
                "\u{8FDB}\u{884C}\u{4E2D}".to_string()
            };
            let mut constraint_lines = Vec::new();
            if contract.has_time_limit()
                && let Some(remaining) = contract.remaining_turns(self.turn)
            {
                if remaining < 0 {
                    constraint_lines
                        .push("\u{5269}\u{4F59}\u{FF1A}\u{5DF2}\u{8D85}\u{65F6}".to_string());
                } else {
                    constraint_lines.push(format!(
                        "\u{5269}\u{4F59}\u{FF1A}{remaining} \u{56DE}\u{5408}"
                    ));
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
                    "\u{6F5C}\u{884C}\u{FF1A}\u{5DF2}\u{66B4}\u{9732}".to_string()
                } else {
                    "\u{6F5C}\u{884C}\u{FF1A}\u{672A}\u{66B4}\u{9732}".to_string()
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
            ContractObjective::KillMonsters { .. } => {
                "\u{51FB}\u{6740}\u{602A}\u{7269}".to_string()
            }
            ContractObjective::CollectItem { item_id, .. } => {
                let item_name = self
                    .data
                    .item_defs
                    .get(item_id)
                    .map(|item| item.name.as_str())
                    .unwrap_or(item_id.as_str());
                format!("\u{6536}\u{96C6} {item_name}")
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
                "\u{652F}\u{7EBF}\u{59D4}\u{6258} {}\u{FF1A}{}/{}",
                contract.name,
                contract.progress,
                contract.target()
            ));
        }
        if let Some(line) = progress_log {
            self.push_log(line);
            self.try_complete_side_contract();
        }
    }

    pub(in crate::game) fn on_item_collected_for_contract(&mut self, item_id: &str, qty: u32) {
        let mut progress_log: Option<String> = None;
        if let Some(contract) = &mut self.side_contract
            && !contract.is_terminal()
            && let ContractObjective::CollectItem {
                item_id: target_item_id,
                ..
            } = &contract.objective
            && target_item_id == item_id
        {
            contract.progress = contract.progress.saturating_add(qty);
            progress_log = Some(format!(
                "\u{652F}\u{7EBF}\u{59D4}\u{6258} {}\u{FF1A}{}/{}",
                contract.name,
                contract.progress,
                contract.target()
            ));
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
                "\u{652F}\u{7EBF}\u{59D4}\u{6258}\u{5B8C}\u{6210}\u{FF1A}{contract_name}\u{FF0C}\u{83B7}\u{5F97} {reward_name} x{added}"
            ));
        } else {
            self.push_log(format!(
                "\u{652F}\u{7EBF}\u{59D4}\u{6258}\u{5B8C}\u{6210}\u{FF1A}{contract_name}\u{FF0C}\u{4F46}\u{5956}\u{52B1}\u{672A}\u{80FD}\u{653E}\u{5165}\u{80CC}\u{5305}"
            ));
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
            self.fail_side_contract(
                "\u{6F5C}\u{884C}\u{5931}\u{8D25}\u{FF1A}\u{654C}\u{4EBA}\u{8FDB}\u{5165}\u{8B66}\u{6212}",
            );
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
            self.push_log(format!(
                "\u{652F}\u{7EBF}\u{59D4}\u{6258}\u{5931}\u{8D25}\u{FF1A}{contract_name}\u{FF08}{reason}\u{FF09}"
            ));
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
            self.fail_side_contract("\u{8D85}\u{8FC7}\u{56DE}\u{5408}\u{9650}\u{5236}");
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
        self.push_log(format!(
            "\u{5FC5}\u{9700}\u{59D4}\u{6258}\u{7269}\u{8FDB}\u{5EA6}\u{FF1A}{collected}/{total}"
        ));
        if let Some(line) = self.side_contract_progress_line() {
            self.push_log(line);
        }
    }
}
