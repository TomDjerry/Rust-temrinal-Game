use super::*;
use crate::game::util::strip_bom;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct SaveState {
    seed: u64,
    map: Map,
    exit_pos: Pos,
    player: Player,
    monsters: Vec<Monster>,
    ground_items: Vec<GroundItem>,
    turn: u32,
    won: bool,
    logs: Vec<String>,
    #[serde(default)]
    active_buffs: Vec<ActiveBuff>,
    #[serde(default)]
    side_contract: Option<SideContract>,
}

impl SaveState {
    pub(super) fn from_game(game: &Game) -> Self {
        Self {
            seed: game.seed,
            map: game.map.clone(),
            exit_pos: game.exit_pos,
            player: game.player.clone(),
            monsters: game.monsters.clone(),
            ground_items: game.ground_items.clone(),
            turn: game.turn,
            won: game.won,
            logs: game.log.iter().cloned().collect(),
            active_buffs: game.active_buffs.clone(),
            side_contract: game.side_contract.clone(),
        }
    }

    pub(super) fn into_game(self, data: GameData) -> Game {
        let mut game = Game {
            seed: self.seed,
            map: self.map,
            exit_pos: self.exit_pos,
            player: self.player,
            monsters: self.monsters,
            ground_items: self.ground_items,
            visible: HashSet::new(),
            log: VecDeque::from(self.logs),
            turn: self.turn,
            rng: StdRng::seed_from_u64(self.seed ^ ((self.turn as u64) << 32) ^ 0x9E3779B97F4A7C15),
            won: self.won,
            quit: false,
            ui_mode: UiMode::Normal,
            inventory_selected: 0,
            data,
            pending_noise: None,
            active_buffs: self.active_buffs,
            side_contract: self.side_contract,
        };
        game.ensure_side_contract(false);
        game.recompute_fov();
        game
    }
}

impl Game {
    pub(super) fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let save = SaveState::from_game(self);
        let json = serde_json::to_string_pretty(&save).context("failed to serialize save data")?;
        fs::write(path, json).with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }

    pub(super) fn load_from_file<P: AsRef<Path>>(path: P, data: GameData) -> Result<Self> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let save: SaveState =
            serde_json::from_str(strip_bom(&raw)).context("failed to parse save file")?;
        Ok(save.into_game(data))
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::build_test_game;
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn save_state_roundtrip_should_restore_core_fields() {
        let mut game = build_test_game(42);
        game.turn = 9;
        let _ = game.add_item_to_inventory("healing_potion", 3);
        let _ = game.add_item_to_inventory("package", 1);
        game.player.pos = Pos::new(game.player.pos.x + 1, game.player.pos.y);
        game.ui_mode = UiMode::Help;

        let save = SaveState::from_game(&game);
        let restored = save.into_game(game.data.clone());

        assert_eq!(restored.seed, game.seed);
        assert_eq!(restored.turn, game.turn);
        assert_eq!(restored.player.pos, game.player.pos);
        assert_eq!(
            restored.player.item_count("healing_potion"),
            game.player.item_count("healing_potion")
        );
        assert_eq!(
            restored.player.has_item("package"),
            game.player.has_item("package")
        );
        assert_eq!(restored.map.width, game.map.width);
        assert_eq!(restored.map.height, game.map.height);
        assert_eq!(restored.ui_mode, UiMode::Normal);
    }

    #[test]
    fn save_load_roundtrip_should_restore_core_state() {
        let mut game = build_test_game(42);
        game.turn = 9;
        let _ = game.add_item_to_inventory("healing_potion", 3);
        let _ = game.add_item_to_inventory("package", 1);
        game.player.pos = Pos::new(game.player.pos.x + 1, game.player.pos.y);
        game.ui_mode = UiMode::Help;

        let mut path = PathBuf::from("target");
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("test-save-{nanos}.json"));

        game.save_to_file(path.to_str().expect("path"))
            .expect("save");
        let restored =
            Game::load_from_file(path.to_str().expect("path"), game.data.clone()).expect("load");
        fs::remove_file(&path).expect("cleanup");

        assert_eq!(restored.seed, game.seed);
        assert_eq!(restored.turn, game.turn);
        assert_eq!(restored.player.pos, game.player.pos);
        assert_eq!(
            restored.player.item_count("healing_potion"),
            game.player.item_count("healing_potion")
        );
        assert_eq!(
            restored.player.has_item("package"),
            game.player.has_item("package")
        );
        assert_eq!(restored.map.width, game.map.width);
        assert_eq!(restored.map.height, game.map.height);
        assert_eq!(restored.ui_mode, UiMode::Normal);
    }
}
