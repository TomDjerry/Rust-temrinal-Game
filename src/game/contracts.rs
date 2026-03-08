mod contract_generation;
mod contract_runtime;

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
        assert_eq!(contract.failure_reason.as_deref(), Some("超过回合限制"));
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
            Some("潜行失败：敌人进入警戒")
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
    fn collect_contract_turn_limit_should_scale_beyond_old_fixed_values() {
        let game = build_test_game(401);

        let timed_turns = game.estimate_collect_contract_turn_limit(2, false);
        let dual_turns = game.estimate_collect_contract_turn_limit(2, true);

        assert!(timed_turns > 8);
        assert!(dual_turns > 10);
        assert!(dual_turns >= timed_turns);
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
