use super::*;
use crate::game::map::TileType;
use crate::game::ui::{InventoryGroup, MapCellKind};

impl Game {
    pub(super) fn inventory_item_view_for(&self, stack: &InventoryStack) -> InventoryItemView {
        let equipped = self.is_item_equipped(&stack.item_id);
        let (name, group, can_use, can_drop, action_label, attr_desc) = self
            .data
            .item_defs
            .get(&stack.item_id)
            .map(|def| {
                let (can_use, can_drop) = self
                    .item_permissions(&stack.item_id)
                    .unwrap_or((false, false));
                let group = inventory_group_for_effect(def.effect);
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
                let action_label = inventory_action_label(group, equipped, can_use, can_drop);
                (
                    def.name.clone(),
                    group,
                    can_use,
                    can_drop,
                    action_label,
                    attr_desc,
                )
            })
            .unwrap_or_else(|| {
                (
                    stack.item_id.clone(),
                    InventoryGroup::Other,
                    false,
                    false,
                    "不可操作".to_string(),
                    String::new(),
                )
            });

        InventoryItemView {
            name,
            qty: stack.qty,
            group,
            can_use,
            can_drop,
            equipped,
            action_label,
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
            log_scroll: self.log_scroll,
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
                kind: MapCellKind::Player,
            };
        }

        if !self.map.is_explored(pos) {
            return MapCell {
                ch: ' ',
                tone: MapTone::Hidden,
                kind: MapCellKind::Unknown,
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
                    kind: MapCellKind::Monster,
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
                    kind: MapCellKind::Item,
                };
            }

            if self
                .traps
                .iter()
                .any(|trap| trap.pos == pos && !trap.triggered)
            {
                return MapCell {
                    ch: '^',
                    tone: MapTone::Visible,
                    kind: MapCellKind::Trap,
                };
            }

            return MapCell {
                ch: self.map.base_glyph(pos),
                tone: MapTone::Visible,
                kind: map_cell_kind_for_tile(self.map.tile(pos).map(|tile| tile.tile_type)),
            };
        }

        MapCell {
            ch: self.map.base_glyph(pos),
            tone: MapTone::Explored,
            kind: map_cell_kind_for_tile(self.map.tile(pos).map(|tile| tile.tile_type)),
        }
    }
}

fn inventory_group_for_effect(effect: ItemEffectDef) -> InventoryGroup {
    match effect {
        ItemEffectDef::Equipment { slot, .. } => match slot {
            EquipmentSlot::Weapon => InventoryGroup::Weapon,
            EquipmentSlot::Armor => InventoryGroup::Armor,
            EquipmentSlot::Accessory => InventoryGroup::Accessory,
        },
        ItemEffectDef::Consumable { .. } | ItemEffectDef::BuffConsumable { .. } => {
            InventoryGroup::Consumable
        }
        ItemEffectDef::QuestPackage | ItemEffectDef::QuestItem { .. } => InventoryGroup::Quest,
    }
}

fn inventory_action_label(
    group: InventoryGroup,
    equipped: bool,
    can_use: bool,
    can_drop: bool,
) -> String {
    if equipped {
        "可卸下".to_string()
    } else if matches!(group, InventoryGroup::Quest) && !can_use && !can_drop {
        "任务物".to_string()
    } else if can_use {
        match group {
            InventoryGroup::Weapon | InventoryGroup::Armor | InventoryGroup::Accessory => {
                "可装备".to_string()
            }
            _ => "可使用".to_string(),
        }
    } else if can_drop {
        "可丢弃".to_string()
    } else {
        "不可操作".to_string()
    }
}

fn map_cell_kind_for_tile(tile_type: Option<TileType>) -> MapCellKind {
    match tile_type {
        Some(TileType::ClosedDoor | TileType::OpenDoor) => MapCellKind::Door,
        Some(TileType::Wall) => MapCellKind::Wall,
        Some(TileType::Floor) => MapCellKind::Floor,
        Some(TileType::Exit) => MapCellKind::Exit,
        None => MapCellKind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{build_test_game, open_floor_map, test_monster};
    use super::super::*;
    use crate::game::map::TileType;
    use crate::game::ui::{InventoryGroup, MapCellKind};

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
    fn inventory_item_view_should_group_items_by_slot_or_usage() {
        let game = build_test_game(111);

        let weapon = game.inventory_item_view_for(&InventoryStack {
            item_id: "rust_sword".to_string(),
            qty: 1,
        });
        let consumable = game.inventory_item_view_for(&InventoryStack {
            item_id: "healing_potion".to_string(),
            qty: 1,
        });
        let quest = game.inventory_item_view_for(&InventoryStack {
            item_id: "delivery_note".to_string(),
            qty: 1,
        });

        assert_eq!(weapon.group, InventoryGroup::Weapon);
        assert_eq!(consumable.group, InventoryGroup::Consumable);
        assert_eq!(quest.group, InventoryGroup::Quest);
    }

    #[test]
    fn inventory_item_view_should_expose_action_label_for_current_state() {
        let mut game = build_test_game(112);
        game.monsters.clear();
        let _ = game.add_item_to_inventory("rust_sword", 1);
        let sword_index = game
            .inventory_entries()
            .iter()
            .position(|entry| entry.item_id == "rust_sword")
            .expect("sword index");
        game.inventory_selected = sword_index;
        game.try_equip_item("rust_sword");

        let equipped = game.inventory_item_view_for(&InventoryStack {
            item_id: "rust_sword".to_string(),
            qty: 1,
        });
        let quest = game.inventory_item_view_for(&InventoryStack {
            item_id: "delivery_note".to_string(),
            qty: 1,
        });

        assert_eq!(equipped.action_label, "可卸下");
        assert_eq!(quest.action_label, "任务物");
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
            vec!["剩余：3 回合".to_string(), "潜行：未暴露".to_string()]
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
            failure_reason: Some("超过回合限制".to_string()),
        });

        let snapshot = game.snapshot();
        let contract = snapshot.side_contract.expect("side contract view");

        assert_eq!(contract.status_text, "已失败");
        assert_eq!(contract.failure_reason, Some("超过回合限制".to_string()));
        assert_eq!(contract.constraint_lines, vec!["剩余：已超时".to_string()]);
    }
    #[test]
    fn snapshot_should_render_door_and_trap_tiles() {
        let mut game = build_test_game(303);
        game.monsters.clear();
        game.ground_items.clear();
        game.map = open_floor_map(8, 8, 1..=6, 1..=6);
        game.player.pos = Pos::new(2, 2);
        game.map.set_tile_type(Pos::new(3, 2), TileType::ClosedDoor);
        game.traps = vec![Trap {
            pos: Pos::new(2, 3),
            damage: 3,
            triggered: false,
        }];
        game.recompute_fov();

        let snapshot = game.snapshot();

        assert_eq!(snapshot.map_rows[2][3].ch, '+');
        assert_eq!(snapshot.map_rows[3][2].ch, '^');
    }

    #[test]
    fn snapshot_cell_view_should_classify_visible_entities_and_tiles() {
        let mut game = build_test_game(113);
        game.monsters.clear();
        game.ground_items.clear();
        game.map = open_floor_map(8, 8, 1..=6, 1..=6);
        game.player.pos = Pos::new(2, 2);
        game.monsters.push(test_monster(
            "slime",
            "史莱姆",
            's',
            Pos::new(3, 2),
            Stats {
                hp: 8,
                max_hp: 8,
                atk: 3,
                def: 1,
            },
        ));
        game.ground_items.push(GroundItem {
            item_id: "healing_potion".to_string(),
            pos: Pos::new(2, 3),
        });
        game.traps = vec![Trap {
            pos: Pos::new(2, 4),
            damage: 3,
            triggered: false,
        }];
        game.map.set_tile_type(Pos::new(4, 2), TileType::ClosedDoor);
        game.map.set_tile_type(Pos::new(1, 2), TileType::Wall);
        game.recompute_fov();

        let snapshot = game.snapshot();

        assert_eq!(snapshot.map_rows[2][2].kind, MapCellKind::Player);
        assert_eq!(snapshot.map_rows[2][3].kind, MapCellKind::Monster);
        assert_eq!(snapshot.map_rows[3][2].kind, MapCellKind::Item);
        assert_eq!(snapshot.map_rows[4][2].kind, MapCellKind::Trap);
        assert_eq!(snapshot.map_rows[2][4].kind, MapCellKind::Door);
        assert_eq!(snapshot.map_rows[2][1].kind, MapCellKind::Wall);
    }
}
