use super::super::*;
use crate::game::map::path::bfs_distance;
use std::collections::HashSet;

const GENERATED_COLLECT_TIME_LIMIT_TURNS: u32 = 14;
const GENERATED_DUAL_CONTRACT_TIME_LIMIT_TURNS: u32 = 18;

impl Game {
    fn generated_collect_contract_constraints(&mut self) -> Vec<ContractConstraint> {
        match self.rng.random_range(0..3) {
            0 => vec![ContractConstraint::TimeLimit {
                start_turn: self.turn,
                max_turns: self.estimate_collect_contract_turn_limit(2, false),
            }],
            1 => vec![ContractConstraint::Stealth { exposed: false }],
            _ => vec![
                ContractConstraint::TimeLimit {
                    start_turn: self.turn,
                    max_turns: self.estimate_collect_contract_turn_limit(2, true),
                },
                ContractConstraint::Stealth { exposed: false },
            ],
        }
    }

    pub(in crate::game) fn estimate_collect_contract_turn_limit(
        &self,
        target: u32,
        include_stealth_buffer: bool,
    ) -> u32 {
        let mut remaining_targets = target as usize;
        let mut current = self.player.pos;
        let blocked = HashSet::new();
        let mut potion_positions = self
            .ground_items
            .iter()
            .filter(|item| item.item_id == "healing_potion")
            .map(|item| item.pos)
            .collect::<Vec<_>>();
        let mut steps = 0_u32;

        while remaining_targets > 0 && !potion_positions.is_empty() {
            let Some((index, distance)) = potion_positions
                .iter()
                .enumerate()
                .filter_map(|(index, pos)| {
                    bfs_distance(&self.map, current, *pos, &blocked)
                        .map(|distance| (index, distance))
                })
                .min_by_key(|(_, distance)| *distance)
            else {
                break;
            };

            steps += distance;
            current = potion_positions.remove(index);
            remaining_targets -= 1;
        }

        let minimum = if include_stealth_buffer {
            GENERATED_DUAL_CONTRACT_TIME_LIMIT_TURNS
        } else {
            GENERATED_COLLECT_TIME_LIMIT_TURNS
        };
        let slack = target.saturating_mul(3) + if include_stealth_buffer { 6 } else { 4 };
        minimum.max(steps + slack)
    }

    pub(in crate::game) fn ensure_side_contract(&mut self, announce: bool) {
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
            self.push_log(format!("新增支线合约：{}", contract.name));
        }
        self.side_contract = Some(contract);
    }
}
