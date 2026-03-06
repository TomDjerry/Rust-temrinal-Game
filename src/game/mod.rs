pub mod combat;
pub mod data;
pub mod map;
pub mod ui;

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
use crate::game::map::path::bfs_next_step;
use crate::game::map::{DungeonLayout, Map, Pos, compute_fov, generate_dungeon, line_of_sight};
use crate::game::ui::{
    AppTerminal, InventoryItemView, MapCell, MapTone, SideContractView, UiMode, UiSnapshot,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const DEFAULT_WIDTH: i32 = 60;
const DEFAULT_HEIGHT: i32 = 26;
const FOV_RADIUS: i32 = 8;
const LOG_CAPACITY: usize = 60;
const SAVE_FILE_PATH: &str = "saves/save1.json";
const NOISE_RADIUS_MOVE: i32 = 6;
const NOISE_RADIUS_INTERACT: i32 = 4;
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum MonsterAiState {
    Patrol,
    Alert { target: Pos, turns_left: u8 },
    Flee { turns_left: u8 },
}

impl Default for MonsterAiState {
    fn default() -> Self {
        Self::Patrol
    }
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
struct SideContract {
    name: String,
    objective: ContractObjective,
    progress: u32,
    reward_item_id: String,
    reward_qty: u32,
    completed: bool,
}

#[derive(Debug)]
struct Game {
    seed: u64,
    map: Map,
    exit_pos: Pos,
    player: Player,
    monsters: Vec<Monster>,
    ground_items: Vec<GroundItem>,
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
            "终端较小，地图已调整为 {}x{}",
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
    }

    fn ensure_side_contract(&mut self, announce: bool) {
        if self.side_contract.is_some() {
            return;
        }

        let contract = if self.rng.random_bool(0.5) {
            SideContract {
                name: "清剿威胁".to_string(),
                objective: ContractObjective::KillMonsters { target: 3 },
                progress: 0,
                reward_item_id: "battle_tonic".to_string(),
                reward_qty: 1,
                completed: false,
            }
        } else {
            SideContract {
                name: "药剂补给".to_string(),
                objective: ContractObjective::CollectItem {
                    item_id: "healing_potion".to_string(),
                    target: 2,
                },
                progress: 0,
                reward_item_id: "iron_skin_tonic".to_string(),
                reward_qty: 1,
                completed: false,
            }
        };

        if announce {
            self.push_log(format!("新增支线合约: {}", contract.name));
        }
        self.side_contract = Some(contract);
    }

    fn apply_action(&mut self, action: Action) {
        match action {
            Action::Quit => {
                self.quit = true;
                return;
            }
            Action::Save => {
                match self.save_to_file(SAVE_FILE_PATH) {
                    Ok(()) => self.push_log(format!("存档成功: {SAVE_FILE_PATH}")),
                    Err(err) => self.push_log(format!("存档失败: {err:#}")),
                }
                return;
            }
            Action::Load => {
                let data = self.data.clone();
                match Self::load_from_file(SAVE_FILE_PATH, data) {
                    Ok(mut loaded) => {
                        loaded.push_log(format!("读档成功: {SAVE_FILE_PATH}"));
                        *self = loaded;
                    }
                    Err(err) => self.push_log(format!("读档失败: {err:#}")),
                }
                return;
            }
            Action::Escape => {
                if self.ui_mode == UiMode::Normal {
                    self.quit = true;
                } else {
                    self.ui_mode = UiMode::Normal;
                }
                return;
            }
            Action::ToggleInventory => {
                self.ui_mode = ui::transition_mode(self.ui_mode, 'i');
                if self.ui_mode == UiMode::Inventory {
                    self.clamp_inventory_selected();
                }
                return;
            }
            Action::ToggleHelp => {
                self.ui_mode = ui::transition_mode(self.ui_mode, '?');
                return;
            }
            _ => {}
        }

        if self.ui_mode == UiMode::Help {
            return;
        }

        if self.ui_mode == UiMode::Inventory {
            let consumed_turn = self.apply_inventory_action(action);
            if consumed_turn {
                self.pending_noise = self.noise_from_action(action);
                self.finish_player_turn();
            }
            return;
        }

        let mut consumed_turn = false;

        match action {
            Action::Move(dx, dy) => consumed_turn = self.try_move_player(dx, dy),
            Action::Pickup => consumed_turn = self.try_pickup(),
            Action::UsePotion => consumed_turn = self.try_use_potion(),
            Action::Wait => {
                self.push_log("你选择等待一回合".to_string());
                consumed_turn = true;
            }
            Action::InventoryUse | Action::InventoryDrop | Action::InventoryUnequip => {}
            Action::Save
            | Action::Load
            | Action::ToggleInventory
            | Action::ToggleHelp
            | Action::Escape
            | Action::Quit => {}
        }

        if consumed_turn {
            self.pending_noise = self.noise_from_action(action);
            self.finish_player_turn();
        }
    }

    fn noise_from_action(&self, action: Action) -> Option<NoiseEvent> {
        let radius = match action {
            Action::Move(_, _) => NOISE_RADIUS_MOVE,
            Action::Pickup
            | Action::UsePotion
            | Action::InventoryUse
            | Action::InventoryDrop
            | Action::InventoryUnequip => NOISE_RADIUS_INTERACT,
            Action::Wait
            | Action::Save
            | Action::Load
            | Action::ToggleInventory
            | Action::ToggleHelp
            | Action::Escape
            | Action::Quit => return None,
        };
        Some(NoiseEvent {
            pos: self.player.pos,
            radius,
        })
    }

    fn apply_inventory_action(&mut self, action: Action) -> bool {
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
        let is_quest = matches!(
            def.effect,
            ItemEffectDef::QuestPackage | ItemEffectDef::QuestItem { .. }
        );

        if is_quest {
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

    fn finish_player_turn(&mut self) {
        if self.quit || self.won || !self.player.stats.is_alive() {
            return;
        }
        self.turn += 1;
        self.monster_turn();
        self.pending_noise = None;
        self.tick_active_buffs();
        self.cleanup_dead_monsters();
        self.check_victory();
        self.recompute_fov();
    }

    fn inventory_entries(&self) -> Vec<InventoryStack> {
        self.player.inventory.clone()
    }

    fn clamp_inventory_selected(&mut self) {
        let max_index = self.inventory_entries().len().saturating_sub(1);
        self.inventory_selected = self.inventory_selected.min(max_index);
    }

    fn add_item_to_inventory(&mut self, item_id: &str, qty: u32) -> u32 {
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

    fn remove_item_from_inventory(&mut self, item_id: &str, qty: u32) -> bool {
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

    fn required_quest_item_ids(&self) -> Vec<String> {
        let mut ids = self
            .data
            .item_defs
            .values()
            .filter_map(|def| match def.effect {
                ItemEffectDef::QuestItem {
                    required_for_delivery: true,
                } => Some(def.id.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        ids.sort();
        ids
    }

    fn collected_required_quest_item_count(&self) -> usize {
        self.required_quest_item_ids()
            .iter()
            .filter(|id| self.player.has_item(id))
            .count()
    }

    fn has_all_required_quest_items(&self) -> bool {
        let required = self.required_quest_item_ids();
        required.iter().all(|id| self.player.has_item(id))
    }

    fn side_contract_target(contract: &SideContract) -> u32 {
        match contract.objective {
            ContractObjective::KillMonsters { target } => target,
            ContractObjective::CollectItem { target, .. } => target,
        }
    }

    fn side_contract_progress_line(&self) -> Option<String> {
        self.side_contract.as_ref().map(|contract| {
            let target = Self::side_contract_target(contract);
            if contract.completed {
                format!("支线合约 {}: 已完成", contract.name)
            } else {
                format!(
                    "支线合约 {}: {}/{}",
                    contract.name, contract.progress, target
                )
            }
        })
    }

    fn side_contract_view(&self) -> Option<SideContractView> {
        self.side_contract.as_ref().map(|contract| {
            let target = Self::side_contract_target(contract);
            let progress = contract.progress.min(target);
            SideContractView {
                name: contract.name.clone(),
                objective: self.side_contract_objective_text(contract),
                progress_text: format!("{progress}/{target}"),
                reward_text: self.side_contract_reward_text(contract),
                completed: contract.completed,
            }
        })
    }

    fn side_contract_objective_text(&self, contract: &SideContract) -> String {
        match &contract.objective {
            ContractObjective::KillMonsters { .. } => "击杀怪物".to_string(),
            ContractObjective::CollectItem { item_id, .. } => {
                let item_name = self
                    .data
                    .item_defs
                    .get(item_id)
                    .map(|item| item.name.as_str())
                    .unwrap_or(item_id.as_str());
                format!("收集 {item_name}")
            }
        }
    }

    fn side_contract_reward_text(&self, contract: &SideContract) -> String {
        let reward_name = self
            .data
            .item_defs
            .get(&contract.reward_item_id)
            .map(|item| item.name.as_str())
            .unwrap_or(contract.reward_item_id.as_str());
        format!("{reward_name} x{}", contract.reward_qty)
    }

    fn on_monster_killed_for_contract(&mut self) {
        let mut progress_log: Option<String> = None;
        if let Some(contract) = &mut self.side_contract
            && !contract.completed
            && matches!(contract.objective, ContractObjective::KillMonsters { .. })
        {
            contract.progress = contract.progress.saturating_add(1);
            let target = Self::side_contract_target(contract);
            progress_log = Some(format!(
                "支线合约 {}: {}/{}",
                contract.name, contract.progress, target
            ));
        }
        if let Some(line) = progress_log {
            self.push_log(line);
        }
        if self
            .side_contract
            .as_ref()
            .is_some_and(|contract| !contract.completed)
        {
            self.try_complete_side_contract();
        }
    }

    fn on_item_collected_for_contract(&mut self, item_id: &str, qty: u32) {
        if qty == 0 {
            return;
        }
        let mut progress_log: Option<String> = None;
        if let Some(contract) = &mut self.side_contract {
            if contract.completed {
                return;
            }
            if let ContractObjective::CollectItem {
                item_id: target_item_id,
                target: _,
            } = &contract.objective
                && target_item_id == item_id
            {
                contract.progress = contract.progress.saturating_add(qty);
                let target = Self::side_contract_target(contract);
                progress_log = Some(format!(
                    "支线合约 {}: {}/{}",
                    contract.name, contract.progress, target
                ));
            }
        }
        if let Some(line) = progress_log {
            self.push_log(line);
            self.try_complete_side_contract();
        }
    }

    fn try_complete_side_contract(&mut self) {
        let mut reward: Option<(String, u32, String)> = None;
        if let Some(contract) = &mut self.side_contract {
            if contract.completed {
                return;
            }
            let target = Self::side_contract_target(contract);
            if contract.progress >= target {
                contract.progress = target;
                contract.completed = true;
                reward = Some((
                    contract.reward_item_id.clone(),
                    contract.reward_qty,
                    contract.name.clone(),
                ));
            }
        }

        let Some((reward_item_id, reward_qty, contract_name)) = reward else {
            return;
        };
        let added = self.add_item_to_inventory(&reward_item_id, reward_qty);
        if added > 0 {
            let reward_name = self
                .data
                .item_defs
                .get(&reward_item_id)
                .map(|item| item.name.clone())
                .unwrap_or(reward_item_id);
            self.push_log(format!(
                "支线合约完成: {contract_name}，获得 {reward_name} x{added}"
            ));
        } else {
            self.push_log(format!("支线合约完成: {contract_name}，但奖励未能放入背包"));
        }
    }

    fn missing_required_quest_item_names(&self) -> Vec<String> {
        self.required_quest_item_ids()
            .into_iter()
            .filter(|id| !self.player.has_item(id))
            .map(|id| {
                self.data
                    .item_defs
                    .get(&id)
                    .map(|def| def.name.clone())
                    .unwrap_or(id)
            })
            .collect()
    }

    fn log_required_quest_progress(&mut self) {
        self.push_log(format!(
            "必需任务物进度: {}/{}",
            self.collected_required_quest_item_count(),
            self.required_quest_item_ids().len()
        ));
        if let Some(line) = self.side_contract_progress_line() {
            self.push_log(line);
        }
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

    fn is_item_equipped(&self, item_id: &str) -> bool {
        self.player.equipment.weapon.as_deref() == Some(item_id)
            || self.player.equipment.armor.as_deref() == Some(item_id)
            || self.player.equipment.accessory.as_deref() == Some(item_id)
    }

    fn try_equip_item(&mut self, item_id: &str) -> bool {
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

    fn try_unequip_item(&mut self, item_id: &str) -> bool {
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

    fn tick_active_buffs(&mut self) {
        for buff in &mut self.active_buffs {
            if buff.turns_left > 0 {
                buff.turns_left -= 1;
            }
        }
        self.active_buffs.retain(|buff| buff.turns_left > 0);
    }

    fn player_effective_atk(&self) -> i32 {
        let (equip_atk_bonus, _, _, _, _, _) = self.equipment_bonus_totals();
        let (buff_atk_bonus, _) = self.active_buff_bonus_totals();
        self.player.stats.atk + equip_atk_bonus + buff_atk_bonus
    }

    fn player_effective_def(&self) -> i32 {
        let (_, equip_def_bonus, _, _, _, _) = self.equipment_bonus_totals();
        let (_, buff_def_bonus) = self.active_buff_bonus_totals();
        self.player.stats.def + equip_def_bonus + buff_def_bonus
    }

    fn player_effective_crit_chance(&self) -> u8 {
        let (_, _, crit_bonus, _, _, _) = self.equipment_bonus_totals();
        crit_bonus
    }

    fn player_effective_dodge_chance(&self) -> u8 {
        let (_, _, _, dodge_bonus, _, _) = self.equipment_bonus_totals();
        dodge_bonus
    }

    fn player_effective_armor_penetration(&self) -> i32 {
        let (_, _, _, _, penetration, _) = self.equipment_bonus_totals();
        penetration
    }

    fn player_effective_damage_reduction_pct(&self) -> u8 {
        let (_, _, _, _, _, reduction) = self.equipment_bonus_totals();
        reduction
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
                self.push_log(format!("你暴击了{}，造成{}伤害", monster_name, damage));
            } else {
                self.push_log(format!("你攻击了{}，造成{}伤害", monster_name, damage));
            }
            if self.monsters[index].stats.hp <= 0 {
                self.push_log(format!("{} 被击倒", monster_name));
                self.on_monster_killed_for_contract();
            }
            return true;
        }

        if !self.map.is_walkable(target) {
            self.push_log("前方被阻挡".to_string());
            return false;
        }

        self.player.pos = target;
        self.try_auto_pickup_package();
        true
    }

    fn try_auto_pickup_package(&mut self) {
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

    fn try_pickup(&mut self) -> bool {
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
                    ItemEffectDef::Consumable { .. } => {
                        self.push_log(format!(
                            "拾取 {}，当前数量 {}",
                            def_name,
                            self.player.item_count(&item.item_id)
                        ));
                    }
                    ItemEffectDef::BuffConsumable { .. } => {
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

    fn try_use_potion(&mut self) -> bool {
        self.try_use_item("healing_potion")
    }

    fn monster_turn(&mut self) {
        if !self.player.stats.is_alive() {
            return;
        }

        let noise_event = self.pending_noise;
        let mut occupied: HashSet<Pos> = self
            .monsters
            .iter()
            .filter(|m| m.stats.is_alive())
            .map(|m| m.pos)
            .collect();

        for idx in 0..self.monsters.len() {
            if !self.monsters[idx].stats.is_alive() {
                continue;
            }

            occupied.remove(&self.monsters[idx].pos);
            let monster_pos = self.monsters[idx].pos;
            let low_hp_threshold = (self.monsters[idx].stats.max_hp / 3).max(1);
            let is_low_hp = self.monsters[idx].stats.hp <= low_hp_threshold;

            let sees_player = monster_pos.manhattan(self.player.pos) <= FOV_RADIUS
                && line_of_sight(&self.map, monster_pos, self.player.pos);

            self.monsters[idx].ai_state = if is_low_hp {
                MonsterAiState::Flee {
                    turns_left: FLEE_TURNS,
                }
            } else if sees_player {
                MonsterAiState::Alert {
                    target: self.player.pos,
                    turns_left: ALERT_TURNS,
                }
            } else if let Some(noise) = noise_event {
                if monster_pos.manhattan(noise.pos) <= noise.radius {
                    MonsterAiState::Alert {
                        target: noise.pos,
                        turns_left: ALERT_TURNS,
                    }
                } else {
                    Self::decay_ai_state(self.monsters[idx].ai_state)
                }
            } else {
                Self::decay_ai_state(self.monsters[idx].ai_state)
            };

            let current_state = self.monsters[idx].ai_state;

            if monster_pos.is_adjacent4(self.player.pos)
                && !matches!(current_state, MonsterAiState::Flee { turns_left: _ })
            {
                if self.roll_chance(self.player_effective_dodge_chance()) {
                    self.push_log(format!(
                        "你闪避了{}({})的攻击",
                        self.monsters[idx].name, self.monsters[idx].kind_id
                    ));
                    occupied.insert(self.monsters[idx].pos);
                    continue;
                }
                let damage = roll_damage(
                    self.monsters[idx].stats.atk,
                    self.player_effective_def(),
                    &mut self.rng,
                );
                let reduction_pct = self.player_effective_damage_reduction_pct() as i32;
                let reduced_damage = (damage * (100 - reduction_pct) / 100).max(1);
                self.player.stats.hp -= reduced_damage;
                self.push_log(format!(
                    "{}({}) 命中你，造成{}伤害",
                    self.monsters[idx].name, self.monsters[idx].kind_id, reduced_damage
                ));
                if self.player.stats.hp <= 0 {
                    self.push_log("你倒下了，投递失败".to_string());
                    occupied.insert(self.monsters[idx].pos);
                    break;
                }
                occupied.insert(self.monsters[idx].pos);
                continue;
            }

            let mut moved = false;
            match current_state {
                MonsterAiState::Flee { turns_left: _ } => {
                    if let Some(step) = self.best_flee_step(monster_pos, &occupied) {
                        self.monsters[idx].pos = step;
                        moved = true;
                    }
                }
                MonsterAiState::Alert {
                    target,
                    turns_left: _,
                } => {
                    if let Some(step) = bfs_next_step(&self.map, monster_pos, target, &occupied) {
                        if step != self.player.pos && !occupied.contains(&step) {
                            self.monsters[idx].pos = step;
                            moved = true;
                        }
                    }
                }
                MonsterAiState::Patrol => {}
            }

            if !moved {
                let dirs = [(0, -1), (0, 1), (-1, 0), (1, 0)];
                let mut choices = Vec::new();
                for (dx, dy) in dirs {
                    let next = Pos::new(monster_pos.x + dx, monster_pos.y + dy);
                    if next == self.player.pos {
                        continue;
                    }
                    if self.map.is_walkable(next) && !occupied.contains(&next) {
                        choices.push(next);
                    }
                }
                if let Some(choice) = choices.choose(&mut self.rng).copied() {
                    self.monsters[idx].pos = choice;
                }
            }

            occupied.insert(self.monsters[idx].pos);
        }
    }

    fn decay_ai_state(state: MonsterAiState) -> MonsterAiState {
        match state {
            MonsterAiState::Patrol => MonsterAiState::Patrol,
            MonsterAiState::Alert { target, turns_left } => {
                if turns_left > 1 {
                    MonsterAiState::Alert {
                        target,
                        turns_left: turns_left - 1,
                    }
                } else {
                    MonsterAiState::Patrol
                }
            }
            MonsterAiState::Flee { turns_left } => {
                if turns_left > 1 {
                    MonsterAiState::Flee {
                        turns_left: turns_left - 1,
                    }
                } else {
                    MonsterAiState::Patrol
                }
            }
        }
    }

    fn best_flee_step(&self, from: Pos, occupied: &HashSet<Pos>) -> Option<Pos> {
        let dirs = [(0, -1), (0, 1), (-1, 0), (1, 0)];
        let mut best: Option<(Pos, i32)> = None;
        for (dx, dy) in dirs {
            let next = Pos::new(from.x + dx, from.y + dy);
            if next == self.player.pos || occupied.contains(&next) || !self.map.is_walkable(next) {
                continue;
            }
            let score = next.manhattan(self.player.pos);
            match best {
                Some((_, best_score)) if score <= best_score => {}
                _ => best = Some((next, score)),
            }
        }
        best.map(|(pos, _)| pos)
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

    fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let save = self.to_save_state();
        let json = serde_json::to_string_pretty(&save).context("failed to serialize save data")?;
        fs::write(path, json).with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }

    fn load_from_file<P: AsRef<Path>>(path: P, data: GameData) -> Result<Self> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let save: SaveState =
            serde_json::from_str(strip_bom(&raw)).context("failed to parse save file")?;
        Ok(Self::from_save_state(save, data))
    }

    fn to_save_state(&self) -> SaveState {
        SaveState {
            seed: self.seed,
            map: self.map.clone(),
            exit_pos: self.exit_pos,
            player: self.player.clone(),
            monsters: self.monsters.clone(),
            ground_items: self.ground_items.clone(),
            turn: self.turn,
            won: self.won,
            logs: self.log.iter().cloned().collect(),
            active_buffs: self.active_buffs.clone(),
            side_contract: self.side_contract.clone(),
        }
    }

    fn from_save_state(save: SaveState, data: GameData) -> Self {
        let mut game = Self {
            seed: save.seed,
            map: save.map,
            exit_pos: save.exit_pos,
            player: save.player,
            monsters: save.monsters,
            ground_items: save.ground_items,
            visible: HashSet::new(),
            log: VecDeque::from(save.logs),
            turn: save.turn,
            rng: StdRng::seed_from_u64(save.seed ^ ((save.turn as u64) << 32) ^ 0x9E3779B97F4A7C15),
            won: save.won,
            quit: false,
            ui_mode: UiMode::Normal,
            inventory_selected: 0,
            data,
            pending_noise: None,
            active_buffs: save.active_buffs,
            side_contract: save.side_contract,
        };
        game.ensure_side_contract(false);
        game.recompute_fov();
        game
    }

    fn snapshot(&self) -> UiSnapshot {
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
                .map(|stack| {
                    let (name, can_use, can_drop, attr_desc) = self
                        .data
                        .item_defs
                        .get(&stack.item_id)
                        .map(|def| {
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
                })
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SaveState {
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

fn strip_bom(content: &str) -> &str {
    content.strip_prefix('\u{feff}').unwrap_or(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventState, KeyModifiers};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn build_test_game(seed: u64) -> Game {
        let data = GameData::load("assets").expect("assets");
        let config = GameConfig {
            seed: Some(seed),
            width: 40,
            height: 22,
        };
        Game::new(config, seed, data).expect("game")
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
        assert!(
            game.player.has_item("package"),
            "package should auto collect on step"
        );
    }

    #[test]
    fn should_ignore_key_release_event_for_movement() {
        let release = KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        };
        let press = KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };

        assert!(action_from_key_event(release).is_none());
        assert!(matches!(
            action_from_key_event(press),
            Some(Action::Move(1, 0))
        ));
    }

    #[test]
    fn should_map_f2_f3_to_save_load_actions() {
        let f2 = KeyEvent {
            code: KeyCode::F(2),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        let f3 = KeyEvent {
            code: KeyCode::F(3),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };

        assert!(matches!(action_from_key_event(f2), Some(Action::Save)));
        assert!(matches!(action_from_key_event(f3), Some(Action::Load)));
    }

    #[test]
    fn should_map_inventory_operation_keys() {
        let enter = KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        let x = KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        let r = KeyEvent {
            code: KeyCode::Char('r'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };

        assert!(matches!(
            action_from_key_event(enter),
            Some(Action::InventoryUse)
        ));
        assert!(matches!(
            action_from_key_event(x),
            Some(Action::InventoryDrop)
        ));
        assert!(matches!(
            action_from_key_event(r),
            Some(Action::InventoryUnequip)
        ));
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
    fn monster_should_enter_alert_and_move_toward_noise() {
        let mut game = build_test_game(16);
        game.monsters.clear();

        let mut map = Map::new(20, 20);
        for y in 2..=4 {
            map.set_tile_type(Pos::new(2, y), map::TileType::Floor);
            map.set_tile_type(Pos::new(8, y), map::TileType::Floor);
        }
        for x in 2..=8 {
            map.set_tile_type(Pos::new(x, 4), map::TileType::Floor);
        }
        game.map = map;
        game.player.pos = Pos::new(2, 2);
        game.monsters.push(Monster {
            kind_id: "test".to_string(),
            name: "Test".to_string(),
            glyph: 't',
            pos: Pos::new(8, 2),
            stats: Stats {
                hp: 8,
                max_hp: 8,
                atk: 3,
                def: 0,
            },
            ai_state: MonsterAiState::Patrol,
        });
        game.pending_noise = Some(NoiseEvent {
            pos: game.player.pos,
            radius: 10,
        });

        game.monster_turn();

        assert_eq!(game.monsters[0].pos, Pos::new(8, 3));
        assert!(matches!(
            game.monsters[0].ai_state,
            MonsterAiState::Alert {
                target,
                turns_left: _
            } if target == game.player.pos
        ));
    }

    #[test]
    fn low_hp_monster_should_flee_instead_of_attacking() {
        let mut game = build_test_game(17);
        game.monsters.clear();

        let mut map = Map::new(20, 20);
        for y in 4..=8 {
            for x in 4..=8 {
                map.set_tile_type(Pos::new(x, y), map::TileType::Floor);
            }
        }
        game.map = map;
        game.player.pos = Pos::new(6, 6);
        game.player.stats.hp = 20;
        game.monsters.push(Monster {
            kind_id: "test".to_string(),
            name: "Coward".to_string(),
            glyph: 'c',
            pos: Pos::new(6, 7),
            stats: Stats {
                hp: 1,
                max_hp: 9,
                atk: 6,
                def: 0,
            },
            ai_state: MonsterAiState::Patrol,
        });
        let hp0 = game.player.stats.hp;
        let dist0 = game.monsters[0].pos.manhattan(game.player.pos);

        game.monster_turn();

        assert_eq!(game.player.stats.hp, hp0);
        assert!(game.monsters[0].pos.manhattan(game.player.pos) > dist0);
        assert!(matches!(
            game.monsters[0].ai_state,
            MonsterAiState::Flee { turns_left: _ }
        ));
    }

    #[test]
    fn attack_buff_consumable_should_apply_and_expire() {
        let mut game = build_test_game(18);
        game.monsters.clear();
        game.ui_mode = UiMode::Inventory;
        let added = game.add_item_to_inventory("battle_tonic", 1);
        assert_eq!(added, 1, "battle_tonic should exist in assets");
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
        let mut game = build_test_game(19);
        game.monsters.clear();
        game.ui_mode = UiMode::Inventory;
        let added = game.add_item_to_inventory("iron_skin_tonic", 1);
        assert_eq!(added, 1, "iron_skin_tonic should exist in assets");
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

    #[test]
    fn victory_should_require_required_quest_items() {
        let mut game = build_test_game(21);
        game.monsters.clear();
        game.player.pos = game.exit_pos;
        let _ = game.add_item_to_inventory("package", 1);

        game.check_victory();
        assert!(
            !game.won,
            "missing required quest item should block victory"
        );

        let _ = game.add_item_to_inventory("delivery_note", 1);
        game.check_victory();
        assert!(game.won, "having package + required quest item should win");
    }

    #[test]
    fn map_should_spawn_required_quest_item() {
        let game = build_test_game(22);
        assert!(
            game.ground_items
                .iter()
                .any(|item| item.item_id == "delivery_note"),
            "required quest item should be spawned on map"
        );
    }

    #[test]
    fn guaranteed_crit_equipment_should_double_attack_damage() {
        let mut baseline = build_test_game(23);
        let mut crit_game = build_test_game(23);
        baseline.monsters.clear();
        crit_game.monsters.clear();

        let mut map = Map::new(20, 20);
        for y in 4..=8 {
            for x in 4..=8 {
                map.set_tile_type(Pos::new(x, y), map::TileType::Floor);
            }
        }
        baseline.map = map.clone();
        crit_game.map = map;
        baseline.player.pos = Pos::new(5, 5);
        crit_game.player.pos = Pos::new(5, 5);

        let monster = Monster {
            kind_id: "test".to_string(),
            name: "Dummy".to_string(),
            glyph: 'd',
            pos: Pos::new(6, 5),
            stats: Stats {
                hp: 50,
                max_hp: 50,
                atk: 1,
                def: 0,
            },
            ai_state: MonsterAiState::Patrol,
        };
        baseline.monsters.push(monster.clone());
        crit_game.monsters.push(monster);

        let added = crit_game.add_item_to_inventory("precision_dagger", 1);
        assert_eq!(added, 1, "precision_dagger should exist in assets");
        assert!(crit_game.try_equip_item("precision_dagger"));

        let _ = baseline.try_move_player(1, 0);
        let _ = crit_game.try_move_player(1, 0);

        let base_damage = 50 - baseline.monsters[0].stats.hp;
        let crit_damage = 50 - crit_game.monsters[0].stats.hp;
        assert_eq!(crit_damage, base_damage * 2);
    }

    #[test]
    fn guaranteed_dodge_equipment_should_prevent_monster_hit() {
        let mut game = build_test_game(24);
        game.monsters.clear();

        let mut map = Map::new(20, 20);
        for y in 4..=8 {
            for x in 4..=8 {
                map.set_tile_type(Pos::new(x, y), map::TileType::Floor);
            }
        }
        game.map = map;
        game.player.pos = Pos::new(6, 6);
        game.monsters.push(Monster {
            kind_id: "test".to_string(),
            name: "Striker".to_string(),
            glyph: 's',
            pos: Pos::new(6, 7),
            stats: Stats {
                hp: 12,
                max_hp: 12,
                atk: 6,
                def: 0,
            },
            ai_state: MonsterAiState::Patrol,
        });
        let added = game.add_item_to_inventory("feather_cloak", 1);
        assert_eq!(added, 1, "feather_cloak should exist in assets");
        assert!(game.try_equip_item("feather_cloak"));
        let hp0 = game.player.stats.hp;

        game.monster_turn();

        assert_eq!(game.player.stats.hp, hp0);
    }

    #[test]
    fn armor_penetration_equipment_should_increase_damage_against_high_def() {
        let mut baseline = build_test_game(25);
        let mut pen_game = build_test_game(25);
        baseline.monsters.clear();
        pen_game.monsters.clear();

        let mut map = Map::new(20, 20);
        for y in 4..=8 {
            for x in 4..=8 {
                map.set_tile_type(Pos::new(x, y), map::TileType::Floor);
            }
        }
        baseline.map = map.clone();
        pen_game.map = map;
        baseline.player.pos = Pos::new(5, 5);
        pen_game.player.pos = Pos::new(5, 5);

        let monster = Monster {
            kind_id: "tank".to_string(),
            name: "Tank".to_string(),
            glyph: 't',
            pos: Pos::new(6, 5),
            stats: Stats {
                hp: 50,
                max_hp: 50,
                atk: 1,
                def: 9,
            },
            ai_state: MonsterAiState::Patrol,
        };
        baseline.monsters.push(monster.clone());
        pen_game.monsters.push(monster);

        let added = pen_game.add_item_to_inventory("armor_breaker", 1);
        assert_eq!(added, 1, "armor_breaker should exist in assets");
        assert!(pen_game.try_equip_item("armor_breaker"));

        let _ = baseline.try_move_player(1, 0);
        let _ = pen_game.try_move_player(1, 0);

        let base_damage = 50 - baseline.monsters[0].stats.hp;
        let pen_damage = 50 - pen_game.monsters[0].stats.hp;
        assert!(pen_damage > base_damage);
    }

    #[test]
    fn damage_reduction_equipment_should_reduce_monster_damage() {
        let mut game = build_test_game(26);
        game.monsters.clear();

        let mut map = Map::new(20, 20);
        for y in 4..=8 {
            for x in 4..=8 {
                map.set_tile_type(Pos::new(x, y), map::TileType::Floor);
            }
        }
        game.map = map;
        game.player.pos = Pos::new(6, 6);
        game.player.stats.hp = 20;
        game.monsters.push(Monster {
            kind_id: "brute".to_string(),
            name: "Brute".to_string(),
            glyph: 'b',
            pos: Pos::new(6, 7),
            stats: Stats {
                hp: 10,
                max_hp: 10,
                atk: 8,
                def: 0,
            },
            ai_state: MonsterAiState::Patrol,
        });

        let added = game.add_item_to_inventory("tower_plate", 1);
        assert_eq!(added, 1, "tower_plate should exist in assets");
        assert!(game.try_equip_item("tower_plate"));

        game.monster_turn();

        assert!(game.player.stats.hp >= 19);
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
        assert!(
            last_log.contains("缺少必需任务物"),
            "should log missing required quest item progress"
        );
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
        assert!(
            game.log.iter().any(|line| line.contains("必需任务物进度")),
            "should log required quest progress when picking required item"
        );
    }

    #[test]
    fn kill_contract_should_complete_and_grant_reward() {
        let mut game = build_test_game(29);
        game.monsters.clear();
        game.side_contract = Some(SideContract {
            name: "测试击杀".to_string(),
            objective: ContractObjective::KillMonsters { target: 1 },
            progress: 0,
            reward_item_id: "battle_tonic".to_string(),
            reward_qty: 1,
            completed: false,
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
            name: "测试收集".to_string(),
            objective: ContractObjective::CollectItem {
                item_id: "healing_potion".to_string(),
                target: 1,
            },
            progress: 0,
            reward_item_id: "iron_skin_tonic".to_string(),
            reward_qty: 1,
            completed: false,
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

    #[test]
    fn snapshot_should_include_side_contract_view() {
        let mut game = build_test_game(31);
        game.side_contract = Some(SideContract {
            name: "测试收集".to_string(),
            objective: ContractObjective::CollectItem {
                item_id: "healing_potion".to_string(),
                target: 2,
            },
            progress: 1,
            reward_item_id: "iron_skin_tonic".to_string(),
            reward_qty: 1,
            completed: false,
        });

        let snapshot = game.snapshot();

        assert_eq!(
            snapshot.side_contract,
            Some(SideContractView {
                name: "测试收集".to_string(),
                objective: "收集 治疗药水".to_string(),
                progress_text: "1/2".to_string(),
                reward_text: "铁肤药剂 x1".to_string(),
                completed: false,
            })
        );
    }
}
