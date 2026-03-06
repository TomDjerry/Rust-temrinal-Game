use super::*;

impl Game {
    fn use_selected_inventory_item(&mut self) -> bool {
        let Some(item_id) = self
            .inventory_entries()
            .get(self.inventory_selected)
            .map(|entry| entry.item_id.clone())
        else {
            return false;
        };

        self.try_use_item(&item_id)
    }

    fn unequip_selected_inventory_item(&mut self) -> bool {
        let Some(item_id) = self
            .inventory_entries()
            .get(self.inventory_selected)
            .map(|entry| entry.item_id.clone())
        else {
            return false;
        };
        self.try_unequip_item(&item_id)
    }

    fn drop_selected_inventory_item(&mut self) -> bool {
        let Some(item_id) = self
            .inventory_entries()
            .get(self.inventory_selected)
            .map(|entry| entry.item_id.clone())
        else {
            return false;
        };

        let Some(def) = self.data.item_defs.get(&item_id) else {
            self.push_log("物品定义缺失，无法丢弃".to_string());
            return false;
        };
        let def_name = def.name.clone();

        let Some((_, can_drop)) = self.item_permissions(&item_id) else {
            self.push_log("物品定义缺失，无法丢弃".to_string());
            return false;
        };

        if !can_drop {
            self.push_log("任务道具不可丢弃".to_string());
            return false;
        }
        if self.is_item_equipped(&item_id) {
            self.push_log("该物品已装备，请先按 r 卸下".to_string());
            return false;
        }

        if !self.remove_item_from_inventory(&item_id, 1) {
            self.push_log("背包中没有该物品".to_string());
            return false;
        }

        self.ground_items.push(GroundItem {
            item_id: item_id.clone(),
            pos: self.player.pos,
        });
        self.push_log(format!("你丢弃了 {}", def_name));
        self.clamp_inventory_selected();
        true
    }

    pub(super) fn apply_inventory_action(&mut self, action: Action) -> bool {
        match action {
            Action::Move(_, dy) if dy < 0 => {
                self.inventory_selected = self.inventory_selected.saturating_sub(1);
                false
            }
            Action::Move(_, dy) if dy > 0 => {
                let max_index = self.inventory_entries().len().saturating_sub(1);
                self.inventory_selected = (self.inventory_selected + 1).min(max_index);
                false
            }
            Action::InventoryUse => self.use_selected_inventory_item(),
            Action::InventoryDrop => self.drop_selected_inventory_item(),
            Action::InventoryUnequip => self.unequip_selected_inventory_item(),
            _ => false,
        }
    }

    pub(super) fn inventory_entries(&self) -> Vec<InventoryStack> {
        self.player.inventory.clone()
    }

    pub(super) fn clamp_inventory_selected(&mut self) {
        let max_index = self.inventory_entries().len().saturating_sub(1);
        self.inventory_selected = self.inventory_selected.min(max_index);
    }

    pub(super) fn item_permissions(&self, item_id: &str) -> Option<(bool, bool)> {
        self.data.item_defs.get(item_id).map(|def| {
            let can_use = matches!(
                def.effect,
                ItemEffectDef::Consumable { .. }
                    | ItemEffectDef::BuffConsumable { .. }
                    | ItemEffectDef::Equipment { .. }
            );
            let can_drop = !matches!(
                def.effect,
                ItemEffectDef::QuestPackage | ItemEffectDef::QuestItem { .. }
            );
            (can_use, can_drop)
        })
    }

    pub(super) fn add_item_to_inventory(&mut self, item_id: &str, qty: u32) -> u32 {
        if qty == 0 {
            return 0;
        }
        let Some(def) = self.data.item_defs.get(item_id) else {
            return 0;
        };

        if def.stackable {
            if let Some(stack) = self
                .player
                .inventory
                .iter_mut()
                .find(|stack| stack.item_id == item_id)
            {
                let available = def.max_stack.saturating_sub(stack.qty);
                let add = qty.min(available);
                stack.qty += add;
                return add;
            }

            let add = qty.min(def.max_stack.max(1));
            if add > 0 {
                self.player.inventory.push(InventoryStack {
                    item_id: item_id.to_string(),
                    qty: add,
                });
            }
            return add;
        }

        if self
            .player
            .inventory
            .iter()
            .any(|stack| stack.item_id == item_id)
        {
            return 0;
        }
        self.player.inventory.push(InventoryStack {
            item_id: item_id.to_string(),
            qty: 1,
        });
        1
    }

    pub(super) fn remove_item_from_inventory(&mut self, item_id: &str, qty: u32) -> bool {
        if qty == 0 {
            return false;
        }
        let Some(index) = self
            .player
            .inventory
            .iter()
            .position(|stack| stack.item_id == item_id && stack.qty >= qty)
        else {
            return false;
        };

        let stack = &mut self.player.inventory[index];
        stack.qty -= qty;
        if stack.qty == 0 {
            self.player.inventory.swap_remove(index);
        }
        true
    }

    fn equipped_slot_ref(&self, slot: EquipmentSlot) -> &Option<String> {
        match slot {
            EquipmentSlot::Weapon => &self.player.equipment.weapon,
            EquipmentSlot::Armor => &self.player.equipment.armor,
            EquipmentSlot::Accessory => &self.player.equipment.accessory,
        }
    }

    fn equipped_slot_mut(&mut self, slot: EquipmentSlot) -> &mut Option<String> {
        match slot {
            EquipmentSlot::Weapon => &mut self.player.equipment.weapon,
            EquipmentSlot::Armor => &mut self.player.equipment.armor,
            EquipmentSlot::Accessory => &mut self.player.equipment.accessory,
        }
    }

    pub(super) fn is_item_equipped(&self, item_id: &str) -> bool {
        self.player.equipment.weapon.as_deref() == Some(item_id)
            || self.player.equipment.armor.as_deref() == Some(item_id)
            || self.player.equipment.accessory.as_deref() == Some(item_id)
    }

    pub(super) fn try_equip_item(&mut self, item_id: &str) -> bool {
        if self.player.item_count(item_id) == 0 {
            self.push_log("背包中没有该物品".to_string());
            return false;
        }
        let Some(def) = self.data.item_defs.get(item_id) else {
            self.push_log("物品定义缺失".to_string());
            return false;
        };
        let def_name = def.name.clone();

        let ItemEffectDef::Equipment { slot, .. } = def.effect else {
            self.push_log("该物品不可装备".to_string());
            return false;
        };

        if self.equipped_slot_ref(slot).as_deref() == Some(item_id) {
            self.push_log(format!("{def_name} 已在对应槽位装备"));
            return false;
        }

        let replaced = self.equipped_slot_mut(slot).replace(item_id.to_string());
        if let Some(old_item_id) = replaced {
            let old_name = self
                .data
                .item_defs
                .get(&old_item_id)
                .map(|item| item.name.clone())
                .unwrap_or(old_item_id);
            self.push_log(format!("卸下 {}，装备 {}", old_name, def_name));
        } else {
            self.push_log(format!("装备 {def_name}"));
        }
        true
    }

    pub(super) fn try_unequip_item(&mut self, item_id: &str) -> bool {
        let Some(def) = self.data.item_defs.get(item_id) else {
            self.push_log("物品定义缺失".to_string());
            return false;
        };
        let def_name = def.name.clone();

        let ItemEffectDef::Equipment { slot, .. } = def.effect else {
            self.push_log("该物品不是装备".to_string());
            return false;
        };

        if self.equipped_slot_ref(slot).as_deref() != Some(item_id) {
            self.push_log(format!("{def_name} 当前未装备"));
            return false;
        }

        *self.equipped_slot_mut(slot) = None;
        self.push_log(format!("已卸下 {def_name}"));
        true
    }

    fn equipment_bonus_totals(&self) -> (i32, i32, u8, u8, i32, u8) {
        let mut atk_bonus = 0;
        let mut def_bonus = 0;
        let mut crit_bonus = 0u8;
        let mut dodge_bonus = 0u8;
        let mut armor_penetration_bonus = 0i32;
        let mut damage_reduction_pct_bonus = 0u8;
        for item_id in [
            self.player.equipment.weapon.as_deref(),
            self.player.equipment.armor.as_deref(),
            self.player.equipment.accessory.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            let Some(def) = self.data.item_defs.get(item_id) else {
                continue;
            };
            if let ItemEffectDef::Equipment {
                atk_bonus: atk,
                def_bonus: def,
                crit_chance_bonus: crit,
                dodge_chance_bonus: dodge,
                armor_penetration_bonus: penetration,
                damage_reduction_pct_bonus: reduction,
                ..
            } = def.effect
            {
                atk_bonus += atk;
                def_bonus += def;
                crit_bonus = crit_bonus.saturating_add(crit);
                dodge_bonus = dodge_bonus.saturating_add(dodge);
                armor_penetration_bonus += penetration;
                damage_reduction_pct_bonus = damage_reduction_pct_bonus.saturating_add(reduction);
            }
        }
        (
            atk_bonus,
            def_bonus,
            crit_bonus.min(100),
            dodge_bonus.min(100),
            armor_penetration_bonus.max(0),
            damage_reduction_pct_bonus.min(95),
        )
    }

    fn active_buff_bonus_totals(&self) -> (i32, i32) {
        let atk_bonus = self.active_buffs.iter().map(|buff| buff.atk_bonus).sum();
        let def_bonus = self.active_buffs.iter().map(|buff| buff.def_bonus).sum();
        (atk_bonus, def_bonus)
    }

    pub(super) fn tick_active_buffs(&mut self) {
        for buff in &mut self.active_buffs {
            if buff.turns_left > 0 {
                buff.turns_left -= 1;
            }
        }
        self.active_buffs.retain(|buff| buff.turns_left > 0);
    }

    pub(super) fn player_effective_atk(&self) -> i32 {
        let (equip_atk_bonus, _, _, _, _, _) = self.equipment_bonus_totals();
        let (buff_atk_bonus, _) = self.active_buff_bonus_totals();
        self.player.stats.atk + equip_atk_bonus + buff_atk_bonus
    }

    pub(super) fn player_effective_def(&self) -> i32 {
        let (_, equip_def_bonus, _, _, _, _) = self.equipment_bonus_totals();
        let (_, buff_def_bonus) = self.active_buff_bonus_totals();
        self.player.stats.def + equip_def_bonus + buff_def_bonus
    }

    pub(super) fn player_effective_crit_chance(&self) -> u8 {
        let (_, _, crit_bonus, _, _, _) = self.equipment_bonus_totals();
        crit_bonus
    }

    pub(super) fn player_effective_dodge_chance(&self) -> u8 {
        let (_, _, _, dodge_bonus, _, _) = self.equipment_bonus_totals();
        dodge_bonus
    }

    pub(super) fn player_effective_armor_penetration(&self) -> i32 {
        let (_, _, _, _, penetration, _) = self.equipment_bonus_totals();
        penetration
    }

    pub(super) fn player_effective_damage_reduction_pct(&self) -> u8 {
        let (_, _, _, _, _, reduction) = self.equipment_bonus_totals();
        reduction
    }

    fn try_use_item(&mut self, item_id: &str) -> bool {
        if self.player.item_count(item_id) == 0 {
            self.push_log("背包中没有可用物品".to_string());
            return false;
        }

        let Some(def) = self.data.item_defs.get(item_id) else {
            self.push_log("物品定义缺失".to_string());
            return false;
        };
        let def_name = def.name.clone();
        let effect = def.effect;

        match effect {
            ItemEffectDef::Consumable { heal } => {
                if !self.remove_item_from_inventory(item_id, 1) {
                    self.push_log("背包中没有可用物品".to_string());
                    return false;
                }
                self.player.stats.hp = (self.player.stats.hp + heal).min(self.player.stats.max_hp);
                self.push_log(format!("你使用了{}，回复{} HP", def_name, heal));
                self.clamp_inventory_selected();
                true
            }
            ItemEffectDef::BuffConsumable {
                atk_bonus,
                def_bonus,
                duration_turns,
            } => {
                if !self.remove_item_from_inventory(item_id, 1) {
                    self.push_log("背包中没有可用物品".to_string());
                    return false;
                }
                self.active_buffs.push(ActiveBuff {
                    atk_bonus,
                    def_bonus,
                    turns_left: duration_turns,
                });
                self.push_log(format!(
                    "你使用了{}，获得 ATK+{} DEF+{}（{} 回合）",
                    def_name, atk_bonus, def_bonus, duration_turns
                ));
                self.clamp_inventory_selected();
                true
            }
            ItemEffectDef::QuestPackage => {
                self.push_log("任务包裹不可使用".to_string());
                false
            }
            ItemEffectDef::QuestItem {
                required_for_delivery: _,
            } => {
                self.push_log("任务道具不可使用".to_string());
                false
            }
            ItemEffectDef::Equipment { .. } => self.try_equip_item(item_id),
        }
    }

    pub(super) fn try_auto_pickup_package(&mut self) {
        if self.player.has_item("package") {
            return;
        }
        let Some(index) = self
            .ground_items
            .iter()
            .position(|item| item.pos == self.player.pos && item.item_id == "package")
        else {
            return;
        };

        self.ground_items.swap_remove(index);
        let _ = self.add_item_to_inventory("package", 1);
        self.push_log("你已自动拾取包裹，前往出口 E".to_string());
    }

    pub(super) fn try_pickup(&mut self) -> bool {
        let pos = self.player.pos;
        let mut picked_any = false;
        let old_items = std::mem::take(&mut self.ground_items);
        let mut kept = Vec::with_capacity(old_items.len());

        for item in old_items {
            if item.pos != pos {
                kept.push(item);
                continue;
            }

            picked_any = true;
            if let Some(def) = self.data.item_defs.get(&item.item_id) {
                let def_effect = def.effect;
                let def_name = def.name.clone();
                let added = self.add_item_to_inventory(&item.item_id, 1);
                if added == 0 {
                    self.push_log(format!("{} 已满，无法继续拾取", def_name));
                    kept.push(item);
                    continue;
                }
                self.on_item_collected_for_contract(&item.item_id, added);
                match def_effect {
                    ItemEffectDef::QuestPackage => {
                        self.push_log("你已拾取包裹，前往出口 E".to_string());
                        self.log_required_quest_progress();
                    }
                    ItemEffectDef::QuestItem {
                        required_for_delivery,
                    } => {
                        if required_for_delivery {
                            self.push_log(format!("你已拾取{}，交付前请妥善保管", def_name));
                            self.log_required_quest_progress();
                        } else {
                            self.push_log(format!("你已拾取任务道具 {}", def_name));
                        }
                    }
                    ItemEffectDef::Consumable { .. } | ItemEffectDef::BuffConsumable { .. } => {
                        self.push_log(format!(
                            "拾取 {}，当前数量 {}",
                            def_name,
                            self.player.item_count(&item.item_id)
                        ));
                    }
                    ItemEffectDef::Equipment { .. } => {
                        self.push_log(format!("拾取装备 {}", def_name));
                    }
                }
            }
        }

        self.ground_items = kept;

        if !picked_any {
            self.push_log("脚下没有可拾取物品".to_string());
        }

        picked_any
    }

    pub(super) fn try_use_potion(&mut self) -> bool {
        self.try_use_item("healing_potion")
    }
}

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
