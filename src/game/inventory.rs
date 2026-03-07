mod buffs;
mod equipment;
mod inventory_operations;

#[cfg(test)]
mod tests {
    use super::super::test_support::build_test_game;
    use super::super::*;

    #[test]
    fn item_permissions_should_match_item_effects() {
        let game = build_test_game(61);

        assert_eq!(game.item_permissions("package"), Some((false, false)));
        assert_eq!(game.item_permissions("delivery_note"), Some((false, false)));
        assert_eq!(game.item_permissions("healing_potion"), Some((true, true)));
        assert_eq!(game.item_permissions("rust_sword"), Some((true, true)));
    }

    #[test]
    fn item_permissions_should_be_none_for_unknown_item() {
        let game = build_test_game(62);

        assert_eq!(game.item_permissions("missing_item"), None);
    }

    #[test]
    fn stepping_onto_package_should_collect_it() {
        let mut game = build_test_game(11);
        let package_pos = game
            .ground_items
            .iter()
            .find(|i| i.item_id == "package")
            .map(|i| i.pos)
            .expect("package");

        let from = [
            Pos::new(package_pos.x, package_pos.y - 1),
            Pos::new(package_pos.x, package_pos.y + 1),
            Pos::new(package_pos.x - 1, package_pos.y),
            Pos::new(package_pos.x + 1, package_pos.y),
        ]
        .into_iter()
        .find(|p| game.map.is_walkable(*p))
        .expect("adjacent walkable");

        game.player.pos = from;
        let moved = game.try_move_player(package_pos.x - from.x, package_pos.y - from.y);
        assert!(moved);
        assert!(game.player.has_item("package"));
    }

    #[test]
    fn inventory_navigation_should_not_move_player() {
        let mut game = build_test_game(8);
        game.monsters.clear();
        let start = game.player.pos;
        game.ui_mode = UiMode::Inventory;
        game.inventory_selected = 0;
        let _ = game.add_item_to_inventory("healing_potion", 1);
        let _ = game.add_item_to_inventory("package", 1);

        game.apply_action(Action::Move(0, 1));
        assert_eq!(game.player.pos, start);
        assert_eq!(game.inventory_selected, 1);

        game.apply_action(Action::Move(0, -1));
        assert_eq!(game.player.pos, start);
        assert_eq!(game.inventory_selected, 0);
    }

    #[test]
    fn inventory_use_potion_should_consume_turn() {
        let mut game = build_test_game(9);
        game.monsters.clear();
        game.ui_mode = UiMode::Inventory;
        game.inventory_selected = 0;
        let _ = game.add_item_to_inventory("healing_potion", 2);
        game.player.stats.hp = 10;
        let turn0 = game.turn;

        game.apply_action(Action::InventoryUse);

        assert_eq!(game.player.item_count("healing_potion"), 1);
        assert!(game.player.stats.hp > 10);
        assert_eq!(game.turn, turn0 + 1);
    }

    #[test]
    fn inventory_drop_potion_should_consume_turn() {
        let mut game = build_test_game(10);
        game.monsters.clear();
        game.ui_mode = UiMode::Inventory;
        game.inventory_selected = 0;
        let _ = game.add_item_to_inventory("healing_potion", 2);
        let turn0 = game.turn;

        game.apply_action(Action::InventoryDrop);

        assert_eq!(game.player.item_count("healing_potion"), 1);
        assert_eq!(game.turn, turn0 + 1);
    }

    #[test]
    fn package_item_cannot_be_used_or_dropped() {
        let mut game = build_test_game(12);
        game.monsters.clear();
        game.ui_mode = UiMode::Inventory;
        let _ = game.add_item_to_inventory("package", 1);
        game.inventory_selected = 0;
        let potions0 = game.player.item_count("healing_potion");
        let turn0 = game.turn;

        game.apply_action(Action::InventoryUse);
        game.apply_action(Action::InventoryDrop);

        assert_eq!(game.player.item_count("healing_potion"), potions0);
        assert_eq!(game.turn, turn0);
    }

    #[test]
    fn equipment_use_should_increase_effective_stats() {
        let mut game = build_test_game(13);
        game.monsters.clear();
        game.ui_mode = UiMode::Inventory;
        let _ = game.add_item_to_inventory("rust_sword", 1);
        let index = game
            .inventory_entries()
            .iter()
            .position(|entry| entry.item_id == "rust_sword")
            .expect("sword index");
        game.inventory_selected = index;

        let atk0 = game.player_effective_atk();
        let turn0 = game.turn;
        game.apply_action(Action::InventoryUse);

        assert_eq!(game.turn, turn0 + 1);
        assert_eq!(game.player_effective_atk(), atk0 + 3);
        assert!(game.is_item_equipped("rust_sword"));
    }

    #[test]
    fn inventory_unequip_should_restore_effective_stats() {
        let mut game = build_test_game(14);
        game.monsters.clear();
        game.ui_mode = UiMode::Inventory;
        let _ = game.add_item_to_inventory("rust_sword", 1);
        let index = game
            .inventory_entries()
            .iter()
            .position(|entry| entry.item_id == "rust_sword")
            .expect("sword index");
        game.inventory_selected = index;
        game.apply_action(Action::InventoryUse);
        let atk_after_equip = game.player_effective_atk();
        let turn0 = game.turn;

        game.apply_action(Action::InventoryUnequip);

        assert_eq!(game.turn, turn0 + 1);
        assert!(atk_after_equip > game.player_effective_atk());
        assert!(!game.is_item_equipped("rust_sword"));
    }

    #[test]
    fn equipped_item_cannot_be_dropped() {
        let mut game = build_test_game(15);
        game.monsters.clear();
        game.ui_mode = UiMode::Inventory;
        let _ = game.add_item_to_inventory("rust_sword", 1);
        let index = game
            .inventory_entries()
            .iter()
            .position(|entry| entry.item_id == "rust_sword")
            .expect("sword index");
        game.inventory_selected = index;
        game.apply_action(Action::InventoryUse);
        let turn0 = game.turn;
        let ground0 = game.ground_items.len();

        game.apply_action(Action::InventoryDrop);

        assert_eq!(game.turn, turn0);
        assert_eq!(game.player.item_count("rust_sword"), 1);
        assert_eq!(game.ground_items.len(), ground0);
    }

    #[test]
    fn attack_buff_consumable_should_apply_and_expire() {
        let mut game = build_test_game(17);
        game.monsters.clear();
        game.ui_mode = UiMode::Inventory;
        let _ = game.add_item_to_inventory("battle_tonic", 1);
        let index = game
            .inventory_entries()
            .iter()
            .position(|entry| entry.item_id == "battle_tonic")
            .expect("battle_tonic index");
        game.inventory_selected = index;

        let atk0 = game.player_effective_atk();
        game.apply_action(Action::InventoryUse);
        assert_eq!(game.player_effective_atk(), atk0 + 2);
        game.ui_mode = UiMode::Normal;

        game.apply_action(Action::Wait);
        game.apply_action(Action::Wait);
        game.apply_action(Action::Wait);

        assert_eq!(game.player_effective_atk(), atk0);
    }

    #[test]
    fn defense_buff_consumable_should_apply_and_expire() {
        let mut game = build_test_game(18);
        game.monsters.clear();
        game.ui_mode = UiMode::Inventory;
        let _ = game.add_item_to_inventory("iron_skin_tonic", 1);
        let index = game
            .inventory_entries()
            .iter()
            .position(|entry| entry.item_id == "iron_skin_tonic")
            .expect("iron_skin_tonic index");
        game.inventory_selected = index;

        let def0 = game.player_effective_def();
        game.apply_action(Action::InventoryUse);
        assert_eq!(game.player_effective_def(), def0 + 2);
        game.ui_mode = UiMode::Normal;

        game.apply_action(Action::Wait);
        game.apply_action(Action::Wait);

        assert_eq!(game.player_effective_def(), def0);
    }

    #[test]
    fn required_quest_item_cannot_be_used_or_dropped() {
        let mut game = build_test_game(20);
        game.monsters.clear();
        game.ui_mode = UiMode::Inventory;
        let _ = game.add_item_to_inventory("delivery_note", 1);
        let index = game
            .inventory_entries()
            .iter()
            .position(|entry| entry.item_id == "delivery_note")
            .expect("delivery_note index");
        game.inventory_selected = index;
        let turn0 = game.turn;

        game.apply_action(Action::InventoryUse);
        game.apply_action(Action::InventoryDrop);

        assert_eq!(game.turn, turn0);
        assert_eq!(game.player.item_count("delivery_note"), 1);
    }
}
