use super::*;

impl Game {
    pub(super) fn inventory_item_view_for(&self, stack: &InventoryStack) -> InventoryItemView {
        let (name, can_use, can_drop, attr_desc) = self
            .data
            .item_defs
            .get(&stack.item_id)
            .map(|def| {
                let (can_use, can_drop) = self
                    .item_permissions(&stack.item_id)
                    .unwrap_or((false, false));
                let attr_desc = match def.effect {
                    ItemEffectDef::Consumable { heal } => format!("回复 {heal} HP"),
                    ItemEffectDef::BuffConsumable {
                        atk_bonus,
                        def_bonus,
                        duration_turns,
                    } => format!(
                        "ATK+{} DEF+{} 持续{}回合",
                        atk_bonus, def_bonus, duration_turns
                    ),
                    ItemEffectDef::QuestPackage => "主线任务物".to_string(),
                    ItemEffectDef::QuestItem {
                        required_for_delivery,
                    } => {
                        if required_for_delivery {
                            "必需任务物".to_string()
                        } else {
                            "可选任务物".to_string()
                        }
                    }
                    ItemEffectDef::Equipment {
                        slot: _,
                        atk_bonus,
                        def_bonus,
                        crit_chance_bonus,
                        dodge_chance_bonus,
                        armor_penetration_bonus,
                        damage_reduction_pct_bonus,
                    } => {
                        let mut tags = Vec::new();
                        if atk_bonus != 0 {
                            tags.push(format!("ATK+{atk_bonus}"));
                        }
                        if def_bonus != 0 {
                            tags.push(format!("DEF+{def_bonus}"));
                        }
                        if crit_chance_bonus != 0 {
                            tags.push(format!("CRIT+{}%", crit_chance_bonus));
                        }
                        if dodge_chance_bonus != 0 {
                            tags.push(format!("EVA+{}%", dodge_chance_bonus));
                        }
                        if armor_penetration_bonus != 0 {
                            tags.push(format!("PEN+{}", armor_penetration_bonus));
                        }
                        if damage_reduction_pct_bonus != 0 {
                            tags.push(format!("RES+{}%", damage_reduction_pct_bonus));
                        }
                        tags.join(" ")
                    }
                };
                (def.name.clone(), can_use, can_drop, attr_desc)
            })
            .unwrap_or_else(|| (stack.item_id.clone(), false, false, String::new()));

        InventoryItemView {
            name,
            qty: stack.qty,
            can_use,
            can_drop,
            equipped: self.is_item_equipped(&stack.item_id),
            attr_desc,
        }
    }

    pub(super) fn snapshot(&self) -> UiSnapshot {
        UiSnapshot {
            map_rows: self.map_rows(),
            turn: self.turn,
            hp: self.player.stats.hp,
            max_hp: self.player.stats.max_hp,
            atk: self.player_effective_atk(),
            def: self.player_effective_def(),
            crit_chance: self.player_effective_crit_chance(),
            dodge_chance: self.player_effective_dodge_chance(),
            armor_penetration: self.player_effective_armor_penetration(),
            damage_reduction_pct: self.player_effective_damage_reduction_pct(),
            potions: self.player.item_count("healing_potion"),
            has_package: self.player.has_item("package"),
            required_quest_items_collected: self.collected_required_quest_item_count(),
            required_quest_items_total: self.required_quest_item_ids().len(),
            won: self.won,
            alive: self.player.stats.is_alive(),
            logs: self.log.iter().cloned().collect(),
            ui_mode: self.ui_mode,
            inventory_selected: self.inventory_selected,
            equipped_weapon: self
                .player
                .equipment
                .weapon
                .as_ref()
                .and_then(|id| self.data.item_defs.get(id))
                .map(|def| def.name.clone()),
            equipped_armor: self
                .player
                .equipment
                .armor
                .as_ref()
                .and_then(|id| self.data.item_defs.get(id))
                .map(|def| def.name.clone()),
            equipped_accessory: self
                .player
                .equipment
                .accessory
                .as_ref()
                .and_then(|id| self.data.item_defs.get(id))
                .map(|def| def.name.clone()),
            side_contract: self.side_contract_view(),
            inventory_items: self
                .inventory_entries()
                .into_iter()
                .map(|stack| self.inventory_item_view_for(&stack))
                .collect(),
        }
    }

    fn map_rows(&self) -> Vec<Vec<MapCell>> {
        let mut rows = Vec::with_capacity(self.map.height as usize);
        for y in 0..self.map.height {
            let mut row = Vec::with_capacity(self.map.width as usize);
            for x in 0..self.map.width {
                let pos = Pos::new(x, y);
                row.push(self.cell_view(pos));
            }
            rows.push(row);
        }
        rows
    }

    fn cell_view(&self, pos: Pos) -> MapCell {
        if self.player.pos == pos && self.visible.contains(&pos) {
            return MapCell {
                ch: '@',
                tone: MapTone::Visible,
            };
        }

        if !self.map.is_explored(pos) {
            return MapCell {
                ch: ' ',
                tone: MapTone::Hidden,
            };
        }

        if self.visible.contains(&pos) {
            if let Some(monster) = self
                .monsters
                .iter()
                .find(|m| m.pos == pos && m.stats.is_alive())
            {
                return MapCell {
                    ch: monster.glyph,
                    tone: MapTone::Visible,
                };
            }

            if let Some(item) = self.ground_items.iter().find(|i| i.pos == pos) {
                let ch = self
                    .data
                    .item_defs
                    .get(&item.item_id)
                    .map(|def| def.glyph)
                    .unwrap_or('?');
                return MapCell {
                    ch,
                    tone: MapTone::Visible,
                };
            }

            return MapCell {
                ch: self.map.base_glyph(pos),
                tone: MapTone::Visible,
            };
        }

        MapCell {
            ch: self.map.base_glyph(pos),
            tone: MapTone::Explored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::build_test_game;
    use super::super::*;

    #[test]
    fn inventory_item_view_should_describe_equipment_bonuses() {
        let game = build_test_game(71);
        let stack = InventoryStack {
            item_id: "rust_sword".to_string(),
            qty: 1,
        };

        let view = game.inventory_item_view_for(&stack);

        assert_eq!(view.name, "生锈短剑");
        assert!(view.can_use);
        assert!(view.can_drop);
        assert!(view.attr_desc.contains("ATK+3"));
        assert!(view.attr_desc.contains("CRIT+10%"));
    }

    #[test]
    fn snapshot_should_include_side_contract_view() {
        let mut game = build_test_game(31);
        game.side_contract = Some(SideContract {
            name: "collect test".to_string(),
            objective: ContractObjective::CollectItem {
                item_id: "healing_potion".to_string(),
                target: 2,
            },
            progress: 1,
            reward_item_id: "iron_skin_tonic".to_string(),
            reward_qty: 1,
            completed: false,
            constraints: Vec::new(),
            failed: false,
            failure_reason: None,
        });

        let snapshot = game.snapshot();
        let contract = snapshot.side_contract.expect("side contract view");

        assert_eq!(contract.name, "collect test");
        assert_eq!(contract.progress_text, "1/2");
        assert_eq!(contract.status_text, "进行中");
        assert!(contract.constraint_lines.is_empty());
        assert_eq!(contract.failure_reason, None);
        assert!(!contract.objective.is_empty());
        assert!(!contract.reward_text.is_empty());
        assert!(!contract.completed);
    }

    #[test]
    fn snapshot_should_include_contract_constraint_status() {
        let mut game = build_test_game(72);
        game.turn = 2;
        game.side_contract = Some(SideContract {
            name: "timed stealth collect".to_string(),
            objective: ContractObjective::CollectItem {
                item_id: "healing_potion".to_string(),
                target: 2,
            },
            progress: 1,
            reward_item_id: "iron_skin_tonic".to_string(),
            reward_qty: 1,
            completed: false,
            constraints: vec![
                ContractConstraint::TimeLimit {
                    start_turn: 0,
                    max_turns: 5,
                },
                ContractConstraint::Stealth { exposed: false },
            ],
            failed: false,
            failure_reason: None,
        });

        let snapshot = game.snapshot();
        let contract = snapshot.side_contract.expect("side contract view");

        assert_eq!(contract.status_text, "进行中");
        assert_eq!(
            contract.constraint_lines,
            vec!["剩余: 3 回合".to_string(), "潜行: 未暴露".to_string()]
        );
        assert_eq!(contract.failure_reason, None);
    }

    #[test]
    fn snapshot_should_include_failed_contract_reason() {
        let mut game = build_test_game(73);
        game.turn = 4;
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
                start_turn: 0,
                max_turns: 2,
            }],
            failed: true,
            failure_reason: Some("time limit exceeded".to_string()),
        });

        let snapshot = game.snapshot();
        let contract = snapshot.side_contract.expect("side contract view");

        assert_eq!(contract.status_text, "已失败");
        assert_eq!(
            contract.failure_reason,
            Some("time limit exceeded".to_string())
        );
        assert_eq!(contract.constraint_lines, vec!["剩余: 已超时".to_string()]);
    }
}
