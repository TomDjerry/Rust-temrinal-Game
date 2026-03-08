use anyhow::{Context, Result};
use crossterm::event::KeyCode;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use std::io::Stdout;

pub type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    Normal,
    Inventory,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapTone {
    Hidden,
    Explored,
    Visible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapCell {
    pub ch: char,
    pub tone: MapTone,
}

#[derive(Debug, Clone)]
pub struct UiSnapshot {
    pub map_rows: Vec<Vec<MapCell>>,
    pub turn: u32,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    pub crit_chance: u8,
    pub dodge_chance: u8,
    pub armor_penetration: i32,
    pub damage_reduction_pct: u8,
    pub potions: u32,
    pub has_package: bool,
    pub required_quest_items_collected: usize,
    pub required_quest_items_total: usize,
    pub won: bool,
    pub alive: bool,
    pub logs: Vec<String>,
    pub ui_mode: UiMode,
    pub inventory_selected: usize,
    pub equipped_weapon: Option<String>,
    pub equipped_armor: Option<String>,
    pub equipped_accessory: Option<String>,
    pub side_contract: Option<SideContractView>,
    pub inventory_items: Vec<InventoryItemView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideContractView {
    pub name: String,
    pub objective: String,
    pub progress_text: String,
    pub reward_text: String,
    pub completed: bool,
    pub status_text: String,
    pub constraint_lines: Vec<String>,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InventoryItemView {
    pub name: String,
    pub qty: u32,
    pub can_use: bool,
    pub can_drop: bool,
    pub equipped: bool,
    pub attr_desc: String,
}

pub fn init_terminal() -> Result<AppTerminal> {
    Terminal::new(CrosstermBackend::new(std::io::stdout())).context("failed to init ratatui")
}

pub fn sidebar_percent(width: u16) -> u16 {
    if width >= 120 {
        32
    } else if width >= 90 {
        38
    } else {
        42
    }
}

pub fn transition_mode(mode: UiMode, input: char) -> UiMode {
    transition_mode_key(mode, KeyCode::Char(input))
}

pub fn max_map_dimensions(term_width: u16, term_height: u16) -> (i32, i32) {
    let side = sidebar_percent(term_width) as i32;
    let map_percent = 100 - side;
    let map_panel_width = (term_width as i32 * map_percent) / 100;
    let map_width = (map_panel_width - 2).max(0);
    let map_height = (term_height as i32 - 2).max(0);
    (map_width, map_height)
}

pub fn transition_mode_key(mode: UiMode, key: KeyCode) -> UiMode {
    match (mode, key) {
        (UiMode::Normal, KeyCode::Char('i')) => UiMode::Inventory,
        (UiMode::Inventory, KeyCode::Char('i')) => UiMode::Normal,
        (UiMode::Normal, KeyCode::Char('?')) => UiMode::Help,
        (UiMode::Help, KeyCode::Char('?')) => UiMode::Normal,
        (UiMode::Inventory, KeyCode::Esc) | (UiMode::Help, KeyCode::Esc) => UiMode::Normal,
        _ => mode,
    }
}

pub fn draw(terminal: &mut AppTerminal, snapshot: &UiSnapshot) -> Result<()> {
    terminal
        .draw(|frame| render(frame, snapshot))
        .context("failed to draw frame")?;
    Ok(())
}

fn render(frame: &mut Frame<'_>, snapshot: &UiSnapshot) {
    let area = frame.area();
    let side = sidebar_percent(area.width);
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(100 - side),
            Constraint::Percentage(side),
        ])
        .split(area);

    render_map_panel(frame, chunks[0], snapshot);
    render_sidebar(frame, chunks[1], snapshot);

    match snapshot.ui_mode {
        UiMode::Inventory => render_inventory_popup(frame, snapshot),
        UiMode::Help => render_help_popup(frame),
        UiMode::Normal => {}
    }
}

fn render_map_panel(frame: &mut Frame<'_>, area: Rect, snapshot: &UiSnapshot) {
    let block = Block::default().title(" 地图 ").borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = build_map_lines(
        snapshot.map_rows.as_slice(),
        inner.width as usize,
        inner.height as usize,
    );
    let widget = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(widget, inner);
}

fn render_sidebar(frame: &mut Frame<'_>, area: Rect, snapshot: &UiSnapshot) {
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Min(6),
        ])
        .split(area);

    let state_lines = vec![
        Line::from(format!("回合: {}", snapshot.turn)),
        Line::from(format!("HP: {}/{}", snapshot.hp.max(0), snapshot.max_hp)),
        Line::from(format!("ATK: {}  DEF: {}", snapshot.atk, snapshot.def)),
        Line::from(format!(
            "CRIT: {}%  EVA: {}%",
            snapshot.crit_chance, snapshot.dodge_chance
        )),
        Line::from(format!(
            "PEN: {}  RES: {}%",
            snapshot.armor_penetration, snapshot.damage_reduction_pct
        )),
        Line::from(format!("药水: {}", snapshot.potions)),
        Line::from(format!(
            "武器: {}",
            snapshot.equipped_weapon.as_deref().unwrap_or("-")
        )),
        Line::from(format!(
            "护甲: {}",
            snapshot.equipped_armor.as_deref().unwrap_or("-")
        )),
        Line::from(format!(
            "饰品: {}",
            snapshot.equipped_accessory.as_deref().unwrap_or("-")
        )),
        Line::from(if snapshot.won {
            "状态: 已完成投递".to_string()
        } else if !snapshot.alive {
            "状态: 已死亡".to_string()
        } else {
            "状态: 进行中".to_string()
        }),
    ];

    let quest_lines = vec![
        Line::from("主线目标"),
        Line::from(if snapshot.has_package {
            "1) 已拿包裹，前往 E"
        } else {
            "1) 寻找包裹 P"
        }),
        Line::from(format!(
            "2) 必需任务物: {}/{}",
            snapshot.required_quest_items_collected, snapshot.required_quest_items_total
        )),
        Line::from(if snapshot.won {
            "3) 已送达"
        } else {
            "3) 尚未送达"
        }),
    ];

    let contract_lines = build_side_contract_lines(snapshot)
        .into_iter()
        .map(Line::from)
        .collect::<Vec<_>>();

    let max_logs = log_limit(parts[3].height);
    let mut logs = snapshot
        .logs
        .iter()
        .rev()
        .take(max_logs)
        .cloned()
        .collect::<Vec<_>>();
    logs.reverse();
    let log_lines = logs
        .into_iter()
        .map(|line| Line::from(format!("- {line}")))
        .collect::<Vec<_>>();

    frame.render_widget(
        Paragraph::new(state_lines)
            .block(Block::default().title(" 状态 ").borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        parts[0],
    );

    frame.render_widget(
        Paragraph::new(quest_lines)
            .block(Block::default().title(" 任务 ").borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        parts[1],
    );

    frame.render_widget(
        Paragraph::new(contract_lines)
            .block(Block::default().title(" 合约 ").borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        parts[2],
    );

    frame.render_widget(
        Paragraph::new(log_lines)
            .block(Block::default().title(" 日志 ").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        parts[3],
    );
}

fn build_side_contract_lines(snapshot: &UiSnapshot) -> Vec<String> {
    match &snapshot.side_contract {
        Some(contract) => {
            let mut lines = vec![
                format!("合约: {}", contract.name),
                format!("目标: {}", contract.objective),
                if contract.completed {
                    "进度: 已完成".to_string()
                } else {
                    format!("进度: {}", contract.progress_text)
                },
                format!("状态: {}", contract.status_text),
            ];
            lines.extend(contract.constraint_lines.iter().cloned());
            lines.push(format!("奖励: {}", contract.reward_text));
            if let Some(reason) = &contract.failure_reason {
                lines.push(format!("失败: {reason}"));
            }
            lines
        }
        None => vec!["暂无支线合约".to_string(), "继续推进主线投递".to_string()],
    }
}

fn render_inventory_popup(frame: &mut Frame<'_>, snapshot: &UiSnapshot) {
    let rect = centered_rect(62, 52, frame.area());
    frame.render_widget(Clear, rect);

    let mut lines = vec![Line::from("背包列表"), Line::from("")];

    if snapshot.inventory_items.is_empty() {
        lines.push(Line::from("  (空)"));
    } else {
        for (index, item) in snapshot.inventory_items.iter().enumerate() {
            let sel = if index == snapshot.inventory_selected {
                ">"
            } else {
                " "
            };
            let ops = match (item.can_use, item.can_drop) {
                (true, true) => "(Enter 使用, x 丢弃)",
                (true, false) => "(Enter 使用)",
                (false, true) => "(x 丢弃)",
                (false, false) => "(不可操作)",
            };
            let equipped_tag = if item.equipped { " [已装备]" } else { "" };
            lines.push(Line::from(format!(
                "{sel} {} x{}{} {}",
                item.name, item.qty, equipped_tag, ops
            )));
            if !item.attr_desc.is_empty() {
                lines.push(Line::from(format!("    属性: {}", item.attr_desc)));
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from("w/s 或 ↑/↓ 选择条目"));
    lines.push(Line::from("Enter 使用/装备, r 卸下, x 丢弃"));
    lines.push(Line::from("i 或 Esc 关闭"));

    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().title(" 背包 ").borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        rect,
    );
}

fn render_help_popup(frame: &mut Frame<'_>) {
    let rect = centered_rect(70, 60, frame.area());
    frame.render_widget(Clear, rect);

    let lines = vec![
        Line::from("c: close adjacent open door"),
        Line::from("帮助"),
        Line::from(""),
        Line::from("WASD / 方向键: 移动"),
        Line::from("g: 拾取"),
        Line::from("u: 使用治疗药水"),
        Line::from(".: 等待一回合"),
        Line::from("i: 背包弹窗"),
        Line::from("背包内: w/s 选择, Enter 使用/装备, r 卸下, x 丢弃"),
        Line::from("?: 帮助弹窗"),
        Line::from("F2: 快速存档  F3: 读取存档"),
        Line::from("Esc: 关闭弹窗 / 退出"),
        Line::from("q: 退出游戏"),
        Line::from(""),
        Line::from("属性说明"),
        Line::from("ATK: 攻击力，影响基础伤害"),
        Line::from("DEF: 防御力，降低受到的基础伤害"),
        Line::from("CRIT: 暴击率，触发时伤害翻倍"),
        Line::from("EVA: 闪避率，触发时免疫本次伤害"),
        Line::from("PEN: 穿透值，按点数降低目标防御"),
        Line::from("RES: 减伤率，按百分比降低受击伤害（保底1）"),
        Line::from(""),
        Line::from("地图图例: P=包裹, D/B=任务道具, !=治疗药水"),
        Line::from("走到 P 上会自动拾取；其余道具可按 g 拾取"),
        Line::from(""),
        Line::from("按 ? 或 Esc 关闭"),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().title(" 帮助 ").borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        rect,
    );
}

fn build_map_lines(rows: &[Vec<MapCell>], width: usize, height: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(height);
    for y in 0..height {
        let mut spans = Vec::with_capacity(width);
        for x in 0..width {
            let cell = rows
                .get(y)
                .and_then(|row| row.get(x))
                .copied()
                .unwrap_or(MapCell {
                    ch: ' ',
                    tone: MapTone::Hidden,
                });
            spans.push(Span::styled(cell.ch.to_string(), style_for_cell(cell)));
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn style_for_cell(cell: MapCell) -> Style {
    match cell.tone {
        MapTone::Hidden => Style::default(),
        MapTone::Explored => Style::default().fg(Color::DarkGray),
        MapTone::Visible => {
            if cell.ch == '@' {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if cell.ch == 'E' {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if cell.ch == '#' {
                Style::default().fg(Color::Gray)
            } else if cell.ch == '.' {
                Style::default().fg(Color::White)
            } else if cell.ch == '!' || cell.ch == 'P' || cell.ch == 'D' || cell.ch == 'B' {
                Style::default().fg(Color::Green)
            } else if cell.ch.is_ascii_lowercase() {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::White)
            }
        }
    }
}

fn log_limit(log_area_height: u16) -> usize {
    if log_area_height >= 14 {
        10
    } else if log_area_height >= 10 {
        8
    } else {
        6
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_snapshot() -> UiSnapshot {
        UiSnapshot {
            map_rows: Vec::new(),
            turn: 1,
            hp: 10,
            max_hp: 10,
            atk: 4,
            def: 2,
            crit_chance: 5,
            dodge_chance: 5,
            armor_penetration: 0,
            damage_reduction_pct: 0,
            potions: 1,
            has_package: false,
            required_quest_items_collected: 0,
            required_quest_items_total: 1,
            won: false,
            alive: true,
            logs: Vec::new(),
            ui_mode: UiMode::Normal,
            inventory_selected: 0,
            equipped_weapon: None,
            equipped_armor: None,
            equipped_accessory: None,
            side_contract: None,
            inventory_items: Vec::new(),
        }
    }

    #[test]
    fn sidebar_ratio_is_adaptive() {
        assert_eq!(sidebar_percent(130), 32);
        assert_eq!(sidebar_percent(100), 38);
        assert_eq!(sidebar_percent(80), 42);
    }

    #[test]
    fn mode_transitions_for_popups_work() {
        assert_eq!(transition_mode(UiMode::Normal, 'i'), UiMode::Inventory);
        assert_eq!(transition_mode(UiMode::Inventory, 'i'), UiMode::Normal);
        assert_eq!(transition_mode(UiMode::Normal, '?'), UiMode::Help);
        assert_eq!(transition_mode(UiMode::Help, '?'), UiMode::Normal);
        assert_eq!(transition_mode(UiMode::Help, 'x'), UiMode::Help);
    }

    #[test]
    fn map_dimensions_should_respect_panel_space() {
        let (w, h) = max_map_dimensions(90, 30);
        assert_eq!(w, 53);
        assert_eq!(h, 28);
    }

    #[test]
    fn side_contract_panel_lines_should_include_details() {
        let mut snapshot = build_snapshot();
        snapshot.side_contract = Some(SideContractView {
            name: "收集补给".to_string(),
            objective: "收集 治疗药水".to_string(),
            progress_text: "1/2".to_string(),
            reward_text: "铁肤药剂 x1".to_string(),
            completed: false,
            status_text: "进行中".to_string(),
            constraint_lines: vec!["剩余: 8 回合".to_string(), "潜行: 未暴露".to_string()],
            failure_reason: None,
        });

        let lines = build_side_contract_lines(&snapshot);

        assert_eq!(
            lines,
            vec![
                "合约: 收集补给".to_string(),
                "目标: 收集 治疗药水".to_string(),
                "进度: 1/2".to_string(),
                "状态: 进行中".to_string(),
                "剩余: 8 回合".to_string(),
                "潜行: 未暴露".to_string(),
                "奖励: 铁肤药剂 x1".to_string(),
            ]
        );
    }

    #[test]
    fn side_contract_panel_lines_should_include_failure_reason() {
        let mut snapshot = build_snapshot();
        snapshot.side_contract = Some(SideContractView {
            name: "限时补给".to_string(),
            objective: "收集 治疗药水".to_string(),
            progress_text: "0/1".to_string(),
            reward_text: "铁肤药剂 x1".to_string(),
            completed: false,
            status_text: "已失败".to_string(),
            constraint_lines: vec!["剩余: 已超时".to_string()],
            failure_reason: Some("time limit exceeded".to_string()),
        });

        let lines = build_side_contract_lines(&snapshot);

        assert!(lines.contains(&"状态: 已失败".to_string()));
        assert!(lines.contains(&"剩余: 已超时".to_string()));
        assert!(lines.contains(&"失败: time limit exceeded".to_string()));
    }
}
