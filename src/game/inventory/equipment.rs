use super::super::*;

#[derive(Default, Clone, Copy)]
struct EquipmentBonuses {
    atk: i32,
    def: i32,
    crit_chance: u8,
    dodge_chance: u8,
    armor_penetration: i32,
    damage_reduction_pct: u8,
}

impl Game {
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

    pub(in crate::game) fn is_item_equipped(&self, item_id: &str) -> bool {
        self.player.equipment.weapon.as_deref() == Some(item_id)
            || self.player.equipment.armor.as_deref() == Some(item_id)
            || self.player.equipment.accessory.as_deref() == Some(item_id)
    }

    pub(in crate::game) fn try_equip_item(&mut self, item_id: &str) -> bool {
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

    pub(in crate::game) fn try_unequip_item(&mut self, item_id: &str) -> bool {
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

    fn equipment_bonus_totals(&self) -> EquipmentBonuses {
        let mut bonuses = EquipmentBonuses::default();
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
                atk_bonus,
                def_bonus,
                crit_chance_bonus,
                dodge_chance_bonus,
                armor_penetration_bonus,
                damage_reduction_pct_bonus,
                ..
            } = def.effect
            {
                bonuses.atk += atk_bonus;
                bonuses.def += def_bonus;
                bonuses.crit_chance = bonuses.crit_chance.saturating_add(crit_chance_bonus);
                bonuses.dodge_chance = bonuses.dodge_chance.saturating_add(dodge_chance_bonus);
                bonuses.armor_penetration += armor_penetration_bonus;
                bonuses.damage_reduction_pct = bonuses
                    .damage_reduction_pct
                    .saturating_add(damage_reduction_pct_bonus);
            }
        }
        bonuses.crit_chance = bonuses.crit_chance.min(100);
        bonuses.dodge_chance = bonuses.dodge_chance.min(100);
        bonuses.armor_penetration = bonuses.armor_penetration.max(0);
        bonuses.damage_reduction_pct = bonuses.damage_reduction_pct.min(95);
        bonuses
    }

    pub(in crate::game) fn player_effective_atk(&self) -> i32 {
        let bonuses = self.equipment_bonus_totals();
        let (buff_atk_bonus, _) = self.active_buff_bonus_totals();
        self.player.stats.atk + bonuses.atk + buff_atk_bonus
    }

    pub(in crate::game) fn player_effective_def(&self) -> i32 {
        let bonuses = self.equipment_bonus_totals();
        let (_, buff_def_bonus) = self.active_buff_bonus_totals();
        self.player.stats.def + bonuses.def + buff_def_bonus
    }

    pub(in crate::game) fn player_effective_crit_chance(&self) -> u8 {
        self.equipment_bonus_totals().crit_chance
    }

    pub(in crate::game) fn player_effective_dodge_chance(&self) -> u8 {
        self.equipment_bonus_totals().dodge_chance
    }

    pub(in crate::game) fn player_effective_armor_penetration(&self) -> i32 {
        self.equipment_bonus_totals().armor_penetration
    }

    pub(in crate::game) fn player_effective_damage_reduction_pct(&self) -> u8 {
        self.equipment_bonus_totals().damage_reduction_pct
    }
}
