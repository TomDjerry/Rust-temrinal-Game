use super::super::*;

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
            self.push_log(
                "\u{7269}\u{54C1}\u{5B9A}\u{4E49}\u{7F3A}\u{5931}\u{FF0C}\u{65E0}\u{6CD5}\u{4E22}\u{5F03}"
                    .to_string(),
            );
            return false;
        };
        let def_name = def.name.clone();

        let Some((_, can_drop)) = self.item_permissions(&item_id) else {
            self.push_log(
                "\u{7269}\u{54C1}\u{5B9A}\u{4E49}\u{7F3A}\u{5931}\u{FF0C}\u{65E0}\u{6CD5}\u{4E22}\u{5F03}"
                    .to_string(),
            );
            return false;
        };

        if !can_drop {
            self.push_log(
                "\u{59D4}\u{6258}\u{9053}\u{5177}\u{4E0D}\u{53EF}\u{4E22}\u{5F03}".to_string(),
            );
            return false;
        }
        if self.is_item_equipped(&item_id) {
            self.push_log(
                "\u{8BE5}\u{7269}\u{54C1}\u{5DF2}\u{88C5}\u{5907}\u{FF0C}\u{8BF7}\u{5148}\u{6309} r \u{5378}\u{4E0B}"
                    .to_string(),
            );
            return false;
        }

        if !self.remove_item_from_inventory(&item_id, 1) {
            self.push_log(
                "\u{80CC}\u{5305}\u{4E2D}\u{6CA1}\u{6709}\u{8BE5}\u{7269}\u{54C1}".to_string(),
            );
            return false;
        }

        self.ground_items.push(GroundItem {
            item_id: item_id.clone(),
            pos: self.player.pos,
        });
        self.push_log(format!("\u{4F60}\u{4E22}\u{5F03}\u{4E86} {}", def_name));
        self.clamp_inventory_selected();
        true
    }

    pub(in crate::game) fn apply_inventory_action(&mut self, action: Action) -> bool {
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

    pub(in crate::game) fn inventory_entries(&self) -> Vec<InventoryStack> {
        self.player.inventory.clone()
    }

    pub(in crate::game) fn clamp_inventory_selected(&mut self) {
        let max_index = self.inventory_entries().len().saturating_sub(1);
        self.inventory_selected = self.inventory_selected.min(max_index);
    }

    pub(in crate::game) fn item_permissions(&self, item_id: &str) -> Option<(bool, bool)> {
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

    pub(in crate::game) fn add_item_to_inventory(&mut self, item_id: &str, qty: u32) -> u32 {
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

    pub(in crate::game) fn remove_item_from_inventory(&mut self, item_id: &str, qty: u32) -> bool {
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

    fn try_use_item(&mut self, item_id: &str) -> bool {
        if self.player.item_count(item_id) == 0 {
            self.push_log(
                "\u{80CC}\u{5305}\u{4E2D}\u{6CA1}\u{6709}\u{53EF}\u{7528}\u{7269}\u{54C1}"
                    .to_string(),
            );
            return false;
        }

        let Some(def) = self.data.item_defs.get(item_id) else {
            self.push_log("\u{7269}\u{54C1}\u{5B9A}\u{4E49}\u{7F3A}\u{5931}".to_string());
            return false;
        };
        let def_name = def.name.clone();
        let effect = def.effect;

        match effect {
            ItemEffectDef::Consumable { heal } => {
                if !self.remove_item_from_inventory(item_id, 1) {
                    self.push_log(
                        "\u{80CC}\u{5305}\u{4E2D}\u{6CA1}\u{6709}\u{53EF}\u{7528}\u{7269}\u{54C1}"
                            .to_string(),
                    );
                    return false;
                }
                self.player.stats.hp = (self.player.stats.hp + heal).min(self.player.stats.max_hp);
                self.push_log(format!(
                    "\u{4F60}\u{4F7F}\u{7528}\u{4E86}{}\u{FF0C}\u{56DE}\u{590D}{} HP",
                    def_name, heal
                ));
                self.clamp_inventory_selected();
                true
            }
            ItemEffectDef::BuffConsumable {
                atk_bonus,
                def_bonus,
                duration_turns,
            } => {
                if !self.remove_item_from_inventory(item_id, 1) {
                    self.push_log(
                        "\u{80CC}\u{5305}\u{4E2D}\u{6CA1}\u{6709}\u{53EF}\u{7528}\u{7269}\u{54C1}"
                            .to_string(),
                    );
                    return false;
                }
                self.active_buffs.push(ActiveBuff {
                    atk_bonus,
                    def_bonus,
                    turns_left: duration_turns,
                });
                self.push_log(format!(
                    "\u{4F60}\u{4F7F}\u{7528}\u{4E86}{}\u{FF0C}\u{83B7}\u{5F97} ATK+{} DEF+{}\u{FF08}{} \u{56DE}\u{5408}\u{FF09}",
                    def_name, atk_bonus, def_bonus, duration_turns
                ));
                self.clamp_inventory_selected();
                true
            }
            ItemEffectDef::QuestPackage => {
                self.push_log("\u{5305}\u{88F9}\u{4E0D}\u{53EF}\u{4F7F}\u{7528}".to_string());
                false
            }
            ItemEffectDef::QuestItem {
                required_for_delivery: _,
            } => {
                self.push_log(
                    "\u{59D4}\u{6258}\u{9053}\u{5177}\u{4E0D}\u{53EF}\u{4F7F}\u{7528}".to_string(),
                );
                false
            }
            ItemEffectDef::Equipment { .. } => self.try_equip_item(item_id),
        }
    }

    pub(in crate::game) fn try_auto_pickup_package(&mut self) {
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
        self.push_log(
            "\u{4F60}\u{5DF2}\u{53D6}\u{5F97}\u{5305}\u{88F9}\u{FF0C}\u{524D}\u{5F80}\u{51FA}\u{53E3} E"
                .to_string(),
        );
    }

    pub(in crate::game) fn try_pickup(&mut self) -> bool {
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
                    self.push_log(format!(
                        "{} \u{5DF2}\u{6EE1}\u{FF0C}\u{65E0}\u{6CD5}\u{7EE7}\u{7EED}\u{62FE}\u{53D6}",
                        def_name
                    ));
                    kept.push(item);
                    continue;
                }
                self.on_item_collected_for_contract(&item.item_id, added);
                match def_effect {
                    ItemEffectDef::QuestPackage => {
                        self.push_log(
                            "\u{4F60}\u{5DF2}\u{62FE}\u{53D6}\u{5305}\u{88F9}\u{FF0C}\u{524D}\u{5F80}\u{51FA}\u{53E3} E"
                                .to_string(),
                        );
                        self.log_required_quest_progress();
                    }
                    ItemEffectDef::QuestItem {
                        required_for_delivery,
                    } => {
                        if required_for_delivery {
                            self.push_log(format!(
                                "\u{4F60}\u{5DF2}\u{62FE}\u{53D6}{}\u{FF0C}\u{914D}\u{9001}\u{524D}\u{8BF7}\u{59A5}\u{5584}\u{4FDD}\u{7BA1}",
                                def_name
                            ));
                            self.log_required_quest_progress();
                        } else {
                            self.push_log(format!(
                                "\u{4F60}\u{5DF2}\u{62FE}\u{53D6}\u{59D4}\u{6258}\u{9053}\u{5177} {}",
                                def_name
                            ));
                        }
                    }
                    ItemEffectDef::Consumable { .. } | ItemEffectDef::BuffConsumable { .. } => {
                        self.push_log(format!(
                            "\u{62FE}\u{53D6} {}\u{FF0C}\u{5F53}\u{524D}\u{6570}\u{91CF} {}",
                            def_name,
                            self.player.item_count(&item.item_id)
                        ));
                    }
                    ItemEffectDef::Equipment { .. } => {
                        self.push_log(format!("\u{62FE}\u{53D6}\u{88C5}\u{5907} {}", def_name));
                    }
                }
            }
        }

        self.ground_items = kept;

        if !picked_any {
            self.push_log(
                "\u{811A}\u{4E0B}\u{6CA1}\u{6709}\u{53EF}\u{62FE}\u{53D6}\u{7684}\u{7269}\u{54C1}"
                    .to_string(),
            );
        }

        picked_any
    }

    pub(in crate::game) fn try_use_potion(&mut self) -> bool {
        self.try_use_item("healing_potion")
    }
}
