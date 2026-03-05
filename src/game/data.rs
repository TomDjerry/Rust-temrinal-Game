use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipmentSlot {
    Weapon,
    Armor,
    Accessory,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ItemEffectDef {
    Consumable {
        heal: i32,
    },
    QuestPackage,
    Equipment {
        slot: EquipmentSlot,
        atk_bonus: i32,
        def_bonus: i32,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    pub glyph: char,
    pub stackable: bool,
    pub max_stack: u32,
    pub effect: ItemEffectDef,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MonsterDef {
    pub id: String,
    pub name: String,
    pub glyph: char,
    pub hp: i32,
    pub atk: i32,
    pub def: i32,
}

#[derive(Debug, Clone)]
pub struct GameData {
    pub item_defs: HashMap<String, ItemDef>,
    pub monster_defs: Vec<MonsterDef>,
}

impl GameData {
    pub fn load<P: AsRef<Path>>(assets_dir: P) -> Result<Self> {
        let assets_dir = assets_dir.as_ref();
        let items_raw = fs::read_to_string(assets_dir.join("items.json")).with_context(|| {
            format!("failed to read {}", assets_dir.join("items.json").display())
        })?;
        let monsters_raw =
            fs::read_to_string(assets_dir.join("monsters.json")).with_context(|| {
                format!(
                    "failed to read {}",
                    assets_dir.join("monsters.json").display()
                )
            })?;

        let items_json = strip_bom(&items_raw);
        let monsters_json = strip_bom(&monsters_raw);

        let items: Vec<ItemDef> =
            serde_json::from_str(items_json).context("failed to parse items.json")?;
        let monsters: Vec<MonsterDef> =
            serde_json::from_str(monsters_json).context("failed to parse monsters.json")?;

        let item_defs = items.into_iter().map(|def| (def.id.clone(), def)).collect();

        Ok(Self {
            item_defs,
            monster_defs: monsters,
        })
    }
}

fn strip_bom(content: &str) -> &str {
    content.strip_prefix('\u{feff}').unwrap_or(content)
}
