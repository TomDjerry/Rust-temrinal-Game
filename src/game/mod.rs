mod actions;
mod ai;
pub mod combat;
mod contracts;
pub mod data;
mod inventory;
pub mod map;
mod save;
mod snapshot;
#[cfg(test)]
mod test_support;
pub mod ui;
mod util;

use anyhow::{Context, Result, bail};
use crossterm::cursor;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use rand::prelude::*;
use std::collections::{HashSet, VecDeque};
use std::io;
use std::time::Duration;

use crate::game::combat::roll_damage;
use crate::game::data::{EquipmentSlot, GameData, ItemEffectDef};
use crate::game::map::{DungeonLayout, Map, Pos, compute_fov, generate_dungeon};
use crate::game::ui::{
    AppTerminal, InventoryItemView, MapCell, MapTone, SideContractView, UiMode, UiSnapshot,
};
use serde::{Deserialize, Serialize};

const DEFAULT_WIDTH: i32 = 60;
const DEFAULT_HEIGHT: i32 = 26;
const FOV_RADIUS: i32 = 8;
const LOG_CAPACITY: usize = 60;
const SAVE_FILE_PATH: &str = "saves/save1.json";
const NOISE_RADIUS_MOVE: i32 = 6;
const NOISE_RADIUS_INTERACT: i32 = 4;
const NOISE_RADIUS_DOOR: i32 = 6;
const NOISE_RADIUS_TRAP: i32 = 8;
const TRAP_DAMAGE: i32 = 3;
const ALERT_TURNS: u8 = 4;
const FLEE_TURNS: u8 = 3;

#[derive(Debug, Clone, Copy)]
pub struct GameConfig {
    pub seed: Option<u64>,
    pub width: i32,
    pub height: i32,
}

impl GameConfig {
    pub fn from_args() -> Self {
        let mut seed = None;
        let mut width = DEFAULT_WIDTH;
        let mut height = DEFAULT_HEIGHT;

        let args = std::env::args().skip(1).collect::<Vec<_>>();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--seed" => {
                    if let Some(raw) = args.get(i + 1) {
                        seed = raw.parse::<u64>().ok();
                        i += 1;
                    }
                }
                "--width" => {
                    if let Some(raw) = args.get(i + 1) {
                        if let Ok(v) = raw.parse::<i32>() {
                            width = v.max(20);
                        }
                        i += 1;
                    }
                }
                "--height" => {
                    if let Some(raw) = args.get(i + 1) {
                        if let Ok(v) = raw.parse::<i32>() {
                            height = v.max(20);
                        }
                        i += 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }

        Self {
            seed,
            width,
            height,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Stats {
    hp: i32,
    max_hp: i32,
    atk: i32,
    def: i32,
}

impl Stats {
    fn is_alive(&self) -> bool {
        self.hp > 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Player {
    pos: Pos,
    stats: Stats,
    inventory: Vec<InventoryStack>,
    equipment: EquippedItems,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InventoryStack {
    item_id: String,
    qty: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct EquippedItems {
    weapon: Option<String>,
    armor: Option<String>,
    accessory: Option<String>,
}

impl Player {
    fn item_count(&self, item_id: &str) -> u32 {
        self.inventory
            .iter()
            .find(|stack| stack.item_id == item_id)
            .map(|stack| stack.qty)
            .unwrap_or(0)
    }

    fn has_item(&self, item_id: &str) -> bool {
        self.item_count(item_id) > 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Monster {
    kind_id: String,
    name: String,
    glyph: char,
    pos: Pos,
    stats: Stats,
    #[serde(default)]
    ai_state: MonsterAiState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GroundItem {
    item_id: String,
    pos: Pos,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Trap {
    pos: Pos,
    damage: i32,
    #[serde(default)]
    triggered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "state", rename_all = "snake_case")]
enum MonsterAiState {
    #[default]
    Patrol,
    Alert {
        target: Pos,
        turns_left: u8,
    },
    Flee {
        turns_left: u8,
    },
}

#[derive(Debug, Clone, Copy)]
struct NoiseEvent {
    pos: Pos,
    radius: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActiveBuff {
    atk_bonus: i32,
    def_bonus: i32,
    turns_left: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum ContractObjective {
    KillMonsters { target: u32 },
    CollectItem { item_id: String, target: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum ContractConstraint {
    TimeLimit { start_turn: u32, max_turns: u32 },
    Stealth { exposed: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SideContract {
    name: String,
    objective: ContractObjective,
    progress: u32,
    reward_item_id: String,
    reward_qty: u32,
    completed: bool,
    #[serde(default)]
    constraints: Vec<ContractConstraint>,
    #[serde(default)]
    failed: bool,
    #[serde(default)]
    failure_reason: Option<String>,
}

#[derive(Debug)]
struct Game {
    seed: u64,
    map: Map,
    exit_pos: Pos,
    player: Player,
    monsters: Vec<Monster>,
    ground_items: Vec<GroundItem>,
    traps: Vec<Trap>,
    visible: HashSet<Pos>,
    log: VecDeque<String>,
    turn: u32,
    rng: StdRng,
    won: bool,
    quit: bool,
    ui_mode: UiMode,
    inventory_selected: usize,
    data: GameData,
    pending_noise: Option<NoiseEvent>,
    active_buffs: Vec<ActiveBuff>,
    side_contract: Option<SideContract>,
}

#[derive(Debug, Clone, Copy)]
enum Action {
    Move(i32, i32),
    Pickup,
    UsePotion,
    Wait,
    InventoryUse,
    InventoryDrop,
    InventoryUnequip,
    CloseDoor,
    Save,
    Load,
    ToggleInventory,
    ToggleHelp,
    Escape,
    Quit,
}

struct TerminalGuard;

impl TerminalGuard {
    fn new() -> Result<Self> {
        terminal::enable_raw_mode().context("failed to enable raw mode")?;
        execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)
            .context("failed to enter alternate screen")?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
        let _ = terminal::disable_raw_mode();
    }
}

pub fn run(config: GameConfig) -> Result<()> {
    let seed = config.seed.unwrap_or_else(rand::random);
    let data = GameData::load("assets").context("failed to load assets")?;

    let _guard = TerminalGuard::new()?;
    let mut terminal = ui::init_terminal()?;
    let area = terminal.size().context("failed to read terminal size")?;
    let (max_map_width, max_map_height) = ui::max_map_dimensions(area.width, area.height);
    if max_map_width < 20 || max_map_height < 20 {
        bail!(
            "terminal too small: current map viewport max {}x{}, need at least 20x20",
            max_map_width,
            max_map_height
        );
    }

    let mut effective_config = config;
    effective_config.width = effective_config.width.clamp(20, max_map_width);
    effective_config.height = effective_config.height.clamp(20, max_map_height);
    let was_clamped =
        effective_config.width != config.width || effective_config.height != config.height;
    let mut game = Game::new(effective_config, seed, data)?;

    game.push_log(format!("Seed: {seed}"));
    if let Some(line) = game.side_contract_progress_line() {
        game.push_log(line);
    }
    if was_clamped {
        game.push_log(format!(
            "缁堢杈冨皬锛屽湴鍥惧凡璋冩暣涓?{}x{}",
            effective_config.width, effective_config.height
        ));
    }
    game.render(&mut terminal)?;

    while !game.won && !game.quit && game.player.stats.is_alive() {
        if let Some(action) = read_action()? {
            game.apply_action(action);
            game.render(&mut terminal)?;
        }
    }

    game.render(&mut terminal)?;
    terminal.show_cursor().context("failed to show cursor")?;
    Ok(())
}

fn read_action() -> Result<Option<Action>> {
    if !event::poll(Duration::from_millis(200)).context("failed to poll terminal event")? {
        return Ok(None);
    }
    let evt = event::read().context("failed to read terminal event")?;
    let Event::Key(key_event) = evt else {
        return Ok(None);
    };

    Ok(action_from_key_event(key_event))
}

fn action_from_key_event(key_event: KeyEvent) -> Option<Action> {
    if key_event.kind != KeyEventKind::Press {
        return None;
    }

    let action = match key_event.code {
        KeyCode::Char('w') | KeyCode::Up => Action::Move(0, -1),
        KeyCode::Char('s') | KeyCode::Down => Action::Move(0, 1),
        KeyCode::Char('a') | KeyCode::Left => Action::Move(-1, 0),
        KeyCode::Char('d') | KeyCode::Right => Action::Move(1, 0),
        KeyCode::Char('g') => Action::Pickup,
        KeyCode::Char('u') => Action::UsePotion,
        KeyCode::Enter => Action::InventoryUse,
        KeyCode::Char('x') => Action::InventoryDrop,
        KeyCode::Char('r') => Action::InventoryUnequip,
        KeyCode::Char('c') => Action::CloseDoor,
        KeyCode::Char('.') => Action::Wait,
        KeyCode::F(2) => Action::Save,
        KeyCode::F(3) => Action::Load,
        KeyCode::Char('i') => Action::ToggleInventory,
        KeyCode::Char('?') => Action::ToggleHelp,
        KeyCode::Esc => Action::Escape,
        KeyCode::Char('q') => Action::Quit,
        _ => return None,
    };

    Some(action)
}

impl Game {
    fn new(config: GameConfig, seed: u64, data: GameData) -> Result<Self> {
        let mut rng = StdRng::seed_from_u64(seed);
        let layout = generate_dungeon(config.width, config.height, &mut rng)
            .context("failed to generate map")?;

        let DungeonLayout {
            map,
            player_start,
            package_pos,
            exit_pos,
            monster_spawns,
            item_spawns,
        } = layout;

        let player = Player {
            pos: player_start,
            stats: Stats {
                hp: 24,
                max_hp: 24,
                atk: 7,
                def: 2,
            },
            inventory: Vec::new(),
            equipment: EquippedItems::default(),
        };

        let mut game = Self {
            seed,
            map,
            exit_pos,
            player,
            monsters: Vec::new(),
            ground_items: Vec::new(),
            traps: Vec::new(),
            visible: HashSet::new(),
            log: VecDeque::with_capacity(LOG_CAPACITY),
            turn: 0,
            rng,
            won: false,
            quit: false,
            ui_mode: UiMode::Normal,
            inventory_selected: 0,
            data,
            pending_noise: None,
            active_buffs: Vec::new(),
            side_contract: None,
        };

        game.spawn_from_layout(package_pos, monster_spawns, item_spawns);
        game.ensure_side_contract(false);
        game.recompute_fov();
        Ok(game)
    }

    fn spawn_from_layout(
        &mut self,
        package_pos: Pos,
        monster_spawns: Vec<Pos>,
        item_spawns: Vec<Pos>,
    ) {
        let mut occupied: HashSet<Pos> = HashSet::new();
        occupied.insert(self.player.pos);
        occupied.insert(package_pos);

        for pos in monster_spawns.iter().copied().take(8) {
            if occupied.contains(&pos) {
                continue;
            }
            let Some(def) = self.data.monster_defs.choose(&mut self.rng) else {
                break;
            };
            occupied.insert(pos);
            self.monsters.push(Monster {
                kind_id: def.id.clone(),
                name: def.name.clone(),
                glyph: def.glyph,
                pos,
                stats: Stats {
                    hp: def.hp,
                    max_hp: def.hp,
                    atk: def.atk,
                    def: def.def,
                },
                ai_state: MonsterAiState::Patrol,
            });
        }

        self.ground_items.push(GroundItem {
            item_id: "package".to_string(),
            pos: package_pos,
        });

        let mut required_quest_defs = self
            .data
            .item_defs
            .values()
            .filter(|def| {
                matches!(
                    def.effect,
                    ItemEffectDef::QuestItem {
                        required_for_delivery: true
                    }
                )
            })
            .collect::<Vec<_>>();
        required_quest_defs.sort_by(|a, b| a.id.cmp(&b.id));

        for def in required_quest_defs {
            let Some(pos) = item_spawns
                .iter()
                .copied()
                .find(|pos| !occupied.contains(pos) && *pos != package_pos)
            else {
                break;
            };
            occupied.insert(pos);
            self.ground_items.push(GroundItem {
                item_id: def.id.clone(),
                pos,
            });
        }

        let mut spawn_candidates = self
            .data
            .item_defs
            .values()
            .filter(|def| {
                !matches!(
                    def.effect,
                    ItemEffectDef::QuestPackage
                        | ItemEffectDef::QuestItem {
                            required_for_delivery: true
                        }
                )
            })
            .collect::<Vec<_>>();
        if spawn_candidates.is_empty() {
            return;
        }
        spawn_candidates.sort_by(|a, b| a.id.cmp(&b.id));

        for pos in item_spawns.iter().copied().take(6) {
            if occupied.contains(&pos) || pos == package_pos {
                continue;
            }
            let Some(def) = spawn_candidates.choose(&mut self.rng) else {
                continue;
            };
            occupied.insert(pos);
            self.ground_items.push(GroundItem {
                item_id: def.id.clone(),
                pos,
            });
        }

        self.populate_environment(package_pos);
    }

    fn populate_environment(&mut self, package_pos: Pos) {
        let mut blocked: HashSet<Pos> = HashSet::new();
        blocked.insert(self.player.pos);
        blocked.insert(self.exit_pos);
        blocked.insert(package_pos);
        for monster in &self.monsters {
            blocked.insert(monster.pos);
        }
        for item in &self.ground_items {
            blocked.insert(item.pos);
        }

        let mut door_candidates = Vec::new();
        let mut trap_candidates = Vec::new();
        for y in 1..(self.map.height - 1) {
            for x in 1..(self.map.width - 1) {
                let pos = Pos::new(x, y);
                if blocked.contains(&pos) {
                    continue;
                }
                let Some(tile) = self.map.tile(pos) else {
                    continue;
                };
                if tile.tile_type != crate::game::map::TileType::Floor {
                    continue;
                }
                let north = self.map.is_walkable(Pos::new(x, y - 1));
                let south = self.map.is_walkable(Pos::new(x, y + 1));
                let west = self.map.is_walkable(Pos::new(x - 1, y));
                let east = self.map.is_walkable(Pos::new(x + 1, y));
                let vertical_corridor = north && south && !west && !east;
                let horizontal_corridor = west && east && !north && !south;
                if vertical_corridor || horizontal_corridor {
                    door_candidates.push(pos);
                } else {
                    trap_candidates.push(pos);
                }
            }
        }

        door_candidates.shuffle(&mut self.rng);
        for pos in door_candidates.into_iter().take(4) {
            self.map
                .set_tile_type(pos, crate::game::map::TileType::ClosedDoor);
            blocked.insert(pos);
        }

        trap_candidates.retain(|pos| !blocked.contains(pos));
        trap_candidates.shuffle(&mut self.rng);
        for pos in trap_candidates.into_iter().take(4) {
            self.traps.push(Trap {
                pos,
                damage: TRAP_DAMAGE,
                triggered: false,
            });
        }
    }

    fn roll_chance(&mut self, chance_percent: u8) -> bool {
        if chance_percent == 0 {
            return false;
        }
        if chance_percent >= 100 {
            return true;
        }
        self.rng.random_range(0..100) < chance_percent
    }

    fn try_move_player(&mut self, dx: i32, dy: i32) -> bool {
        let target = Pos::new(self.player.pos.x + dx, self.player.pos.y + dy);

        if let Some(index) = self
            .monsters
            .iter()
            .position(|m| m.pos == target && m.stats.is_alive())
        {
            let monster_name = self.monsters[index].name.clone();
            let crit = self.roll_chance(self.player_effective_crit_chance());
            let effective_monster_def =
                (self.monsters[index].stats.def - self.player_effective_armor_penetration()).max(0);
            let mut damage = roll_damage(
                self.player_effective_atk(),
                effective_monster_def,
                &mut self.rng,
            );
            if crit {
                damage *= 2;
            }
            self.monsters[index].stats.hp -= damage;
            if crit {
                self.push_log(format!(
                    "浣犳毚鍑讳簡{}锛岄€犳垚{}浼ゅ",
                    monster_name, damage
                ));
            } else {
                self.push_log(format!(
                    "浣犳敾鍑讳簡{}锛岄€犳垚{}浼ゅ",
                    monster_name, damage
                ));
            }
            if self.monsters[index].stats.hp <= 0 {
                self.push_log(format!("{} 被击倒", monster_name));
                self.on_monster_killed_for_contract();
            }
            return true;
        }

        if self
            .map
            .tile(target)
            .is_some_and(|tile| matches!(tile.tile_type, crate::game::map::TileType::ClosedDoor))
        {
            self.map
                .set_tile_type(target, crate::game::map::TileType::OpenDoor);
            self.push_log("door opened".to_string());
            return true;
        }

        if !self.map.is_walkable(target) {
            self.push_log("blocked ahead".to_string());
            return false;
        }

        self.player.pos = target;
        self.try_auto_pickup_package();
        self.trigger_trap_at_player_pos();
        true
    }

    fn check_victory(&mut self) {
        if self.player.pos == self.exit_pos
            && self.player.has_item("package")
            && !self.has_all_required_quest_items()
        {
            let missing = self.missing_required_quest_item_names().join("、");
            self.push_log(format!(
                "缺少必需任务物: {missing}（{}/{}）",
                self.collected_required_quest_item_count(),
                self.required_quest_item_ids().len()
            ));
            return;
        }

        if self.player.has_item("package")
            && self.has_all_required_quest_items()
            && self.player.pos == self.exit_pos
        {
            self.won = true;
            self.push_log(format!("第{}回合：包裹已送达，任务完成", self.turn));
        }
    }

    fn recompute_fov(&mut self) {
        self.visible = compute_fov(&self.map, self.player.pos, FOV_RADIUS);
        for pos in &self.visible {
            self.map.mark_explored(*pos);
        }
    }

    fn cleanup_dead_monsters(&mut self) {
        self.monsters.retain(|m| m.stats.is_alive());
    }

    fn push_log(&mut self, line: String) {
        if self.log.len() == LOG_CAPACITY {
            self.log.pop_front();
        }
        self.log.push_back(line);
    }

    fn render(&self, terminal: &mut AppTerminal) -> Result<()> {
        let snapshot = self.snapshot();
        ui::draw(terminal, &snapshot)
    }
}
