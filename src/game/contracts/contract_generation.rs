use super::super::*;

const GENERATED_COLLECT_TIME_LIMIT_TURNS: u32 = 8;
const GENERATED_DUAL_CONTRACT_TIME_LIMIT_TURNS: u32 = 10;

impl Game {
    fn generated_collect_contract_constraints(&mut self) -> Vec<ContractConstraint> {
        match self.rng.random_range(0..3) {
            0 => vec![ContractConstraint::TimeLimit {
                start_turn: self.turn,
                max_turns: GENERATED_COLLECT_TIME_LIMIT_TURNS,
            }],
            1 => vec![ContractConstraint::Stealth { exposed: false }],
            _ => vec![
                ContractConstraint::TimeLimit {
                    start_turn: self.turn,
                    max_turns: GENERATED_DUAL_CONTRACT_TIME_LIMIT_TURNS,
                },
                ContractConstraint::Stealth { exposed: false },
            ],
        }
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
            self.push_log(format!("新增支线合约: {}", contract.name));
        }
        self.side_contract = Some(contract);
    }
}
