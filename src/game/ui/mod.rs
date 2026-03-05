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
    pub potions: u32,
    pub has_package: bool,
    pub won: bool,
    pub alive: bool,
    pub logs: Vec<String>,
    pub ui_mode: UiMode,
    pub inventory_selected: usize,
    pub equipped_weapon: Option<String>,
    pub equipped_armor: Option<String>,
    pub equipped_accessory: Option<String>,
    pub inventory_items: Vec<InventoryItemView>,
}

#[derive(Debug, Clone)]
pub struct InventoryItemView {
    pub name: String,
    pub qty: u32,
    pub can_use: bool,
    pub can_drop: bool,
    pub equipped: bool,
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
            Constraint::Length(8),
            Constraint::Length(5),
            Constraint::Min(6),
        ])
        .split(area);

    let state_lines = vec![
        Line::from(format!("回合: {}", snapshot.turn)),
        Line::from(format!("HP: {}/{}", snapshot.hp.max(0), snapshot.max_hp)),
        Line::from(format!("ATK: {}  DEF: {}", snapshot.atk, snapshot.def)),
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
        Line::from(if snapshot.won {
            "2) 已送达"
        } else {
            "2) 尚未送达"
        }),
    ];

    let max_logs = log_limit(parts[2].height);
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
        Paragraph::new(log_lines)
            .block(Block::default().title(" 日志 ").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        parts[2],
    );
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
        Line::from("地图图例: P=包裹, !=治疗药水"),
        Line::from("走到 P 上会自动拾取；药水可按 g 拾取"),
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
            } else if cell.ch == '!' || cell.ch == 'P' {
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
}
