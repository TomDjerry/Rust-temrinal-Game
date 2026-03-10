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
    Log,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapTone {
    Hidden,
    Explored,
    Visible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapCellKind {
    Unknown,
    Player,
    Monster,
    Item,
    Door,
    Trap,
    Wall,
    Floor,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapCell {
    pub ch: char,
    pub tone: MapTone,
    pub kind: MapCellKind,
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
    pub log_scroll: usize,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InventoryGroup {
    Weapon,
    Armor,
    Accessory,
    Consumable,
    Quest,
    Other,
}

#[derive(Debug, Clone)]
pub struct InventoryItemView {
    pub name: String,
    pub qty: u32,
    pub group: InventoryGroup,
    pub can_use: bool,
    pub can_drop: bool,
    pub equipped: bool,
    pub action_label: String,
    pub attr_desc: String,
}

const MAP_TITLE: &str = "\u{5730}\u{56FE}";
const STATUS_TITLE: &str = "\u{72B6}\u{6001}";
const QUEST_TITLE: &str = "\u{4EFB}\u{52A1}\u{76EE}\u{6807}";
const CONTRACT_TITLE: &str = "\u{59D4}\u{6258}";
const LOG_TITLE: &str = "\u{65E5}\u{5FD7}";
const LOG_HISTORY_TITLE: &str = "\u{65E5}\u{5FD7}\u{8BB0}\u{5F55}";
const INVENTORY_TITLE: &str = "\u{80CC}\u{5305}";
const INVENTORY_LIST_TITLE: &str = "\u{7269}\u{54C1}\u{6E05}\u{5355}";
const HELP_TITLE: &str = "\u{5E2E}\u{52A9}";

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
        (UiMode::Normal, KeyCode::Char('l')) => UiMode::Log,
        (UiMode::Log, KeyCode::Char('l')) => UiMode::Normal,
        (UiMode::Inventory, KeyCode::Esc)
        | (UiMode::Help, KeyCode::Esc)
        | (UiMode::Log, KeyCode::Esc) => UiMode::Normal,
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
        UiMode::Log => render_log_popup(frame, snapshot),
        UiMode::Normal => {}
    }
}

fn render_map_panel(frame: &mut Frame<'_>, area: Rect, snapshot: &UiSnapshot) {
    let block = Block::default()
        .title(format!(" {MAP_TITLE} "))
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = build_map_lines(
        snapshot.map_rows.as_slice(),
        inner.width as usize,
        inner.height as usize,
    );
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
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
        Line::from(format!("\u{56DE}\u{5408}: {}", snapshot.turn)),
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
        Line::from(format!("\u{836F}\u{6C34}: {}", snapshot.potions)),
        Line::from(format!(
            "\u{6B66}\u{5668}: {}",
            snapshot.equipped_weapon.as_deref().unwrap_or("-")
        )),
        Line::from(format!(
            "\u{62A4}\u{7532}: {}",
            snapshot.equipped_armor.as_deref().unwrap_or("-")
        )),
        Line::from(format!(
            "\u{9970}\u{54C1}: {}",
            snapshot.equipped_accessory.as_deref().unwrap_or("-")
        )),
        Line::from(if snapshot.won {
            "\u{5F53}\u{524D}\u{FF1A}\u{5DF2}\u{5B8C}\u{6210}\u{914D}\u{9001}".to_string()
        } else if !snapshot.alive {
            "\u{5F53}\u{524D}\u{FF1A}\u{4EFB}\u{52A1}\u{5931}\u{8D25}".to_string()
        } else {
            "\u{5F53}\u{524D}\u{FF1A}\u{63A2}\u{7D22}\u{4E2D}".to_string()
        }),
    ];

    let quest_lines = vec![
        Line::from("\u{4EFB}\u{52A1}\u{76EE}\u{6807}"),
        Line::from(if snapshot.has_package {
            "1) \u{5DF2}\u{53D6}\u{5F97}\u{5305}\u{88F9}\u{FF0C}\u{524D}\u{5F80}\u{51FA}\u{53E3} E"
        } else {
            "1) \u{627E}\u{5230}\u{5305}\u{88F9} P"
        }),
        Line::from(format!(
            "2) \u{5FC5}\u{9700}\u{59D4}\u{6258}\u{7269}\u{FF1A}{}/{}",
            snapshot.required_quest_items_collected, snapshot.required_quest_items_total
        )),
        Line::from(if snapshot.won {
            "3) \u{5DF2}\u{5B8C}\u{6210}"
        } else {
            "3) \u{672A}\u{5B8C}\u{6210}"
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
            .block(
                Block::default()
                    .title(format!(" {STATUS_TITLE} "))
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true }),
        parts[0],
    );
    frame.render_widget(
        Paragraph::new(quest_lines)
            .block(
                Block::default()
                    .title(format!(" {QUEST_TITLE} "))
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true }),
        parts[1],
    );
    frame.render_widget(
        Paragraph::new(contract_lines)
            .block(
                Block::default()
                    .title(format!(" {CONTRACT_TITLE} "))
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true }),
        parts[2],
    );
    frame.render_widget(
        Paragraph::new(log_lines)
            .block(
                Block::default()
                    .title(format!(" {LOG_TITLE} "))
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: false }),
        parts[3],
    );
}

fn build_side_contract_lines(snapshot: &UiSnapshot) -> Vec<String> {
    match &snapshot.side_contract {
        Some(contract) => {
            let mut lines = vec![
                format!("\u{59D4}\u{6258}\u{FF1A}{}", contract.name),
                format!("\u{76EE}\u{6807}: {}", contract.objective),
                if contract.completed {
                    "\u{8FDB}\u{5EA6}: \u{5DF2}\u{5B8C}\u{6210}".to_string()
                } else {
                    format!("\u{8FDB}\u{5EA6}: {}", contract.progress_text)
                },
                format!("\u{72B6}\u{6001}: {}", contract.status_text),
            ];
            lines.extend(contract.constraint_lines.iter().cloned());
            lines.push(format!("\u{5956}\u{52B1}: {}", contract.reward_text));
            if let Some(reason) = &contract.failure_reason {
                lines.push(format!("\u{5931}\u{8D25}: {reason}"));
            }
            lines
        }
        None => vec![
            "\u{6682}\u{65E0}\u{652F}\u{7EBF}\u{59D4}\u{6258}".to_string(),
            "\u{7EE7}\u{7EED}\u{63A8}\u{8FDB}\u{4E3B}\u{7EBF}\u{6295}\u{9012}".to_string(),
        ],
    }
}

fn render_inventory_popup(frame: &mut Frame<'_>, snapshot: &UiSnapshot) {
    let rect = centered_rect(62, 52, frame.area());
    frame.render_widget(Clear, rect);

    frame.render_widget(
        Paragraph::new(inventory_popup_lines(snapshot))
            .block(
                Block::default()
                    .title(format!(" {INVENTORY_TITLE} "))
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true }),
        rect,
    );
}

fn inventory_popup_lines(snapshot: &UiSnapshot) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(INVENTORY_LIST_TITLE), Line::from("")];

    if snapshot.inventory_items.is_empty() {
        lines.push(Line::from("  (\u{7A7A})"));
    } else {
        let group_order = [
            InventoryGroup::Weapon,
            InventoryGroup::Armor,
            InventoryGroup::Accessory,
            InventoryGroup::Consumable,
            InventoryGroup::Quest,
            InventoryGroup::Other,
        ];

        for group in group_order {
            let grouped = snapshot
                .inventory_items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.group == group)
                .collect::<Vec<_>>();
            if grouped.is_empty() {
                continue;
            }

            lines.push(Line::styled(
                format!("-- {} --", inventory_group_title(group)),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));

            for (index, item) in grouped {
                let selected = index == snapshot.inventory_selected;
                let sel = if selected { ">" } else { " " };
                let equipped_tag = if item.equipped {
                    " [\u{5DF2}\u{88C5}\u{5907}]"
                } else {
                    ""
                };
                let primary = format!(
                    "{sel} {} x{}{} [{}]",
                    item.name, item.qty, equipped_tag, item.action_label
                );
                let is_actionable = item.equipped || item.can_use || item.can_drop;
                let primary_style = if selected {
                    let background = if is_actionable {
                        Color::Yellow
                    } else {
                        Color::DarkGray
                    };
                    Style::default()
                        .fg(Color::Black)
                        .bg(background)
                        .add_modifier(Modifier::BOLD)
                } else if is_actionable {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                lines.push(Line::styled(primary, primary_style));
                if !item.attr_desc.is_empty() {
                    lines.push(Line::styled(
                        format!("    \u{5C5E}\u{6027}: {}", item.attr_desc),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            }

            lines.push(Line::from(""));
        }
    }

    lines.push(Line::from(
        "w/s \u{6216} \u{2191}/\u{2193} \u{9009}\u{62E9}",
    ));
    lines.push(Line::from(
        "Enter \u{4F7F}\u{7528}\u{6216}\u{88C5}\u{5907}\u{FF0C}r \u{5378}\u{4E0B}\u{FF0C}x \u{4E22}\u{5F03}",
    ));
    lines.push(Line::from("i \u{6216} Esc \u{5173}\u{95ED}"));
    lines
}

fn inventory_group_title(group: InventoryGroup) -> &'static str {
    match group {
        InventoryGroup::Weapon => "\u{6B66}\u{5668}",
        InventoryGroup::Armor => "\u{62A4}\u{7532}",
        InventoryGroup::Accessory => "\u{9970}\u{54C1}",
        InventoryGroup::Consumable => "\u{6D88}\u{8017}\u{54C1}",
        InventoryGroup::Quest => "\u{4EFB}\u{52A1}\u{7269}",
        InventoryGroup::Other => "\u{5176}\u{4ED6}",
    }
}

fn render_log_popup(frame: &mut Frame<'_>, snapshot: &UiSnapshot) {
    let rect = centered_rect(78, 70, frame.area());
    frame.render_widget(Clear, rect);

    let inner_height = rect.height.saturating_sub(2) as usize;
    let body_height = inner_height.saturating_sub(2).max(1);
    let total = snapshot.logs.len();
    let end = total.saturating_sub(snapshot.log_scroll);
    let start = end.saturating_sub(body_height);
    let visible = snapshot.logs[start..end]
        .iter()
        .map(|line| Line::from(line.clone()))
        .collect::<Vec<_>>();
    let footer = Line::from(format!(
        "W/S \u{6216} \u{2191}/\u{2193} \u{7FFB}\u{9875}\u{FF0C}L \u{6216} Esc \u{5173}\u{95ED} ({}-{} / {})",
        if total == 0 { 0 } else { start + 1 },
        end,
        total
    ));

    let mut lines = visible;
    lines.push(Line::from(""));
    lines.push(footer);

    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(format!(" {LOG_HISTORY_TITLE} "))
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true }),
        rect,
    );
}

fn render_help_popup(frame: &mut Frame<'_>) {
    let rect = centered_rect(70, 60, frame.area());
    frame.render_widget(Clear, rect);

    let lines = vec![
        Line::from(HELP_TITLE),
        Line::from(""),
        Line::from("WASD / \u{65B9}\u{5411}\u{952E}: \u{79FB}\u{52A8}"),
        Line::from("g\u{FF1A}\u{62FE}\u{53D6}\u{5730}\u{9762}\u{7269}\u{54C1}"),
        Line::from("u\u{FF1A}\u{4F7F}\u{7528}\u{6CBB}\u{7597}\u{836F}\u{6C34}"),
        Line::from(
            "c\u{FF1A}\u{5173}\u{95ED}\u{76F8}\u{90BB}\u{5DF2}\u{5F00}\u{542F}\u{7684}\u{95E8}",
        ),
        Line::from("l\u{FF1A}\u{67E5}\u{770B}\u{65E5}\u{5FD7}\u{8BB0}\u{5F55}"),
        Line::from(".\u{FF1A}\u{539F}\u{5730}\u{7B49}\u{5F85}\u{4E00}\u{56DE}\u{5408}"),
        Line::from("i\u{FF1A}\u{6253}\u{5F00}\u{80CC}\u{5305}"),
        Line::from(
            "\u{80CC}\u{5305}\u{5185}\u{FF1A}w/s \u{9009}\u{62E9}\u{FF0C}Enter \u{4F7F}\u{7528}\u{6216}\u{88C5}\u{5907}\u{FF0C}r \u{5378}\u{4E0B}\u{FF0C}x \u{4E22}\u{5F03}",
        ),
        Line::from("?\u{FF1A}\u{6253}\u{5F00}\u{5E2E}\u{52A9}"),
        Line::from(
            "F2\u{FF1A}\u{5FEB}\u{901F}\u{5B58}\u{6863}   F3\u{FF1A}\u{5FEB}\u{901F}\u{8BFB}\u{6863}",
        ),
        Line::from(
            "Esc\u{FF1A}\u{5173}\u{95ED}\u{5F53}\u{524D}\u{754C}\u{9762} / \u{9000}\u{51FA}\u{6E38}\u{620F}",
        ),
        Line::from("q\u{FF1A}\u{9000}\u{51FA}\u{6E38}\u{620F}"),
        Line::from(""),
        Line::from("\u{5C5E}\u{6027}\u{8BF4}\u{660E}"),
        Line::from(
            "ATK: \u{653B}\u{51FB}\u{529B}, \u{5F71}\u{54CD}\u{57FA}\u{7840}\u{4F24}\u{5BB3}",
        ),
        Line::from(
            "DEF: \u{9632}\u{5FA1}\u{529B}, \u{964D}\u{4F4E}\u{53D7}\u{5230}\u{7684}\u{57FA}\u{7840}\u{4F24}\u{5BB3}",
        ),
        Line::from(
            "CRIT: \u{66B4}\u{51FB}\u{7387}, \u{89E6}\u{53D1}\u{65F6}\u{4F24}\u{5BB3}\u{7FFB}\u{500D}",
        ),
        Line::from(
            "EVA: \u{95EA}\u{907F}\u{7387}, \u{89E6}\u{53D1}\u{65F6}\u{514D}\u{75AB}\u{672C}\u{6B21}\u{4F24}\u{5BB3}",
        ),
        Line::from(
            "PEN: \u{7A7F}\u{900F}\u{503C}, \u{6309}\u{70B9}\u{6570}\u{964D}\u{4F4E}\u{76EE}\u{6807}\u{9632}\u{5FA1}",
        ),
        Line::from(
            "RES: \u{51CF}\u{4F24}\u{7387}, \u{6309}\u{767E}\u{5206}\u{6BD4}\u{964D}\u{4F4E}\u{53D7}\u{5230}\u{4F24}\u{5BB3}",
        ),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(format!(" {HELP_TITLE} "))
                    .borders(Borders::ALL),
            )
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
                    kind: MapCellKind::Unknown,
                });
            spans.push(Span::styled(cell.ch.to_string(), map_cell_style(cell)));
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn map_cell_style(cell: MapCell) -> Style {
    match cell.tone {
        MapTone::Hidden => Style::default().fg(Color::Black),
        MapTone::Explored => Style::default().fg(Color::DarkGray),
        MapTone::Visible => {
            let color = match cell.kind {
                MapCellKind::Player => Color::Cyan,
                MapCellKind::Monster => Color::Red,
                MapCellKind::Item => Color::Yellow,
                MapCellKind::Door => Color::LightYellow,
                MapCellKind::Trap => Color::Magenta,
                MapCellKind::Wall => Color::Gray,
                MapCellKind::Floor => Color::White,
                MapCellKind::Exit => Color::Green,
                MapCellKind::Unknown => Color::White,
            };
            Style::default().fg(color).add_modifier(Modifier::BOLD)
        }
    }
}

fn log_limit(log_area_height: u16) -> usize {
    if log_area_height >= 14 {
        10
    } else if log_area_height >= 10 {
        6
    } else {
        4
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
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
        .split(popup_layout[1])[1]
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
            log_scroll: 0,
            ui_mode: UiMode::Normal,
            inventory_selected: 0,
            equipped_weapon: None,
            equipped_armor: None,
            equipped_accessory: None,
            side_contract: None,
            inventory_items: Vec::new(),
        }
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<Vec<_>>()
            .join("")
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
        assert_eq!(transition_mode(UiMode::Normal, 'l'), UiMode::Log);
        assert_eq!(transition_mode(UiMode::Log, 'l'), UiMode::Normal);
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
            name: "\u{6536}\u{96C6}\u{8865}\u{7ED9}".to_string(),
            objective: "\u{6536}\u{96C6} \u{6CBB}\u{7597}\u{836F}\u{6C34}".to_string(),
            progress_text: "1/2".to_string(),
            reward_text: "\u{94C1}\u{80A4}\u{836F}\u{5242} x1".to_string(),
            completed: false,
            status_text: "\u{8FDB}\u{884C}\u{4E2D}".to_string(),
            constraint_lines: vec![
                "\u{5269}\u{4F59}: 3 \u{56DE}\u{5408}".to_string(),
                "\u{6F5C}\u{884C}: \u{672A}\u{66B4}\u{9732}".to_string(),
            ],
            failure_reason: None,
        });

        let lines = build_side_contract_lines(&snapshot);

        assert_eq!(
            lines,
            vec![
                "\u{59D4}\u{6258}\u{FF1A}\u{6536}\u{96C6}\u{8865}\u{7ED9}".to_string(),
                "\u{76EE}\u{6807}: \u{6536}\u{96C6} \u{6CBB}\u{7597}\u{836F}\u{6C34}".to_string(),
                "\u{8FDB}\u{5EA6}: 1/2".to_string(),
                "\u{72B6}\u{6001}: \u{8FDB}\u{884C}\u{4E2D}".to_string(),
                "\u{5269}\u{4F59}: 3 \u{56DE}\u{5408}".to_string(),
                "\u{6F5C}\u{884C}: \u{672A}\u{66B4}\u{9732}".to_string(),
                "\u{5956}\u{52B1}: \u{94C1}\u{80A4}\u{836F}\u{5242} x1".to_string(),
            ]
        );
    }

    #[test]
    fn side_contract_panel_lines_should_include_failure_reason() {
        let mut snapshot = build_snapshot();
        snapshot.side_contract = Some(SideContractView {
            name: "\u{9650}\u{65F6}\u{8865}\u{7ED9}".to_string(),
            objective: "\u{6536}\u{96C6} \u{6CBB}\u{7597}\u{836F}\u{6C34}".to_string(),
            progress_text: "0/1".to_string(),
            reward_text: "\u{94C1}\u{80A4}\u{836F}\u{5242} x1".to_string(),
            completed: false,
            status_text: "\u{5DF2}\u{5931}\u{8D25}".to_string(),
            constraint_lines: vec!["\u{5269}\u{4F59}: \u{5DF2}\u{8D85}\u{65F6}".to_string()],
            failure_reason: Some("\u{8D85}\u{8FC7}\u{56DE}\u{5408}\u{9650}\u{5236}".to_string()),
        });

        let lines = build_side_contract_lines(&snapshot);

        assert!(lines.contains(&"\u{72B6}\u{6001}: \u{5DF2}\u{5931}\u{8D25}".to_string()));
        assert!(lines.contains(&"\u{5269}\u{4F59}: \u{5DF2}\u{8D85}\u{65F6}".to_string()));
        assert!(lines.contains(
            &"\u{5931}\u{8D25}: \u{8D85}\u{8FC7}\u{56DE}\u{5408}\u{9650}\u{5236}".to_string()
        ));
    }

    #[test]
    fn map_cell_style_should_color_visible_player_and_monster_differently() {
        let player = map_cell_style(MapCell {
            ch: '@',
            tone: MapTone::Visible,
            kind: MapCellKind::Player,
        });
        let monster = map_cell_style(MapCell {
            ch: 'g',
            tone: MapTone::Visible,
            kind: MapCellKind::Monster,
        });
        let item = map_cell_style(MapCell {
            ch: '!',
            tone: MapTone::Visible,
            kind: MapCellKind::Item,
        });

        assert_ne!(player.fg, monster.fg);
        assert_ne!(monster.fg, item.fg);
        assert!(player.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn map_cell_style_should_keep_hidden_and_explored_cells_muted() {
        let hidden = map_cell_style(MapCell {
            ch: ' ',
            tone: MapTone::Hidden,
            kind: MapCellKind::Unknown,
        });
        let explored = map_cell_style(MapCell {
            ch: '.',
            tone: MapTone::Explored,
            kind: MapCellKind::Floor,
        });

        assert_eq!(hidden.fg, Some(Color::Black));
        assert_eq!(explored.fg, Some(Color::DarkGray));
    }

    #[test]
    fn inventory_popup_lines_should_group_items_by_category_order() {
        let mut snapshot = build_snapshot();
        snapshot.inventory_items = vec![
            InventoryItemView {
                name: "\u{6CBB}\u{7597}\u{836F}\u{6C34}".to_string(),
                qty: 2,
                group: InventoryGroup::Consumable,
                can_use: true,
                can_drop: true,
                equipped: false,
                action_label: "\u{53EF}\u{4F7F}\u{7528}".to_string(),
                attr_desc: "Heal 8 HP".to_string(),
            },
            InventoryItemView {
                name: "\u{751F}\u{9508}\u{77ED}\u{5251}".to_string(),
                qty: 1,
                group: InventoryGroup::Weapon,
                can_use: true,
                can_drop: true,
                equipped: false,
                action_label: "\u{53EF}\u{88C5}\u{5907}".to_string(),
                attr_desc: "ATK+3".to_string(),
            },
            InventoryItemView {
                name: "\u{7B7E}\u{6536}\u{5355}".to_string(),
                qty: 1,
                group: InventoryGroup::Quest,
                can_use: false,
                can_drop: false,
                equipped: false,
                action_label: "\u{4EFB}\u{52A1}\u{7269}".to_string(),
                attr_desc: "\u{5FC5}\u{9700}\u{4EFB}\u{52A1}\u{7269}".to_string(),
            },
        ];

        let lines = inventory_popup_lines(&snapshot);
        let rendered = lines.iter().map(line_text).collect::<Vec<_>>();
        let weapon_index = rendered
            .iter()
            .position(|line| line.contains("\u{6B66}\u{5668}"))
            .expect("weapon header");
        let consumable_index = rendered
            .iter()
            .position(|line| line.contains("\u{6D88}\u{8017}\u{54C1}"))
            .expect("consumable header");
        let quest_index = rendered
            .iter()
            .position(|line| line.contains("\u{4EFB}\u{52A1}\u{7269}"))
            .expect("quest header");

        assert!(weapon_index < consumable_index);
        assert!(consumable_index < quest_index);
    }

    #[test]
    fn inventory_popup_lines_should_highlight_selected_actionable_item() {
        let mut snapshot = build_snapshot();
        snapshot.inventory_selected = 1;
        snapshot.inventory_items = vec![
            InventoryItemView {
                name: "\u{65E7}\u{62AB}\u{98CE}".to_string(),
                qty: 1,
                group: InventoryGroup::Armor,
                can_use: true,
                can_drop: true,
                equipped: false,
                action_label: "\u{53EF}\u{88C5}\u{5907}".to_string(),
                attr_desc: String::new(),
            },
            InventoryItemView {
                name: "\u{6CBB}\u{7597}\u{836F}\u{6C34}".to_string(),
                qty: 1,
                group: InventoryGroup::Consumable,
                can_use: true,
                can_drop: true,
                equipped: false,
                action_label: "\u{53EF}\u{4F7F}\u{7528}".to_string(),
                attr_desc: String::new(),
            },
        ];

        let lines = inventory_popup_lines(&snapshot);
        let rendered = lines.iter().map(line_text).collect::<Vec<_>>();
        let selected_line = rendered
            .iter()
            .find(|line| line.contains("\u{6CBB}\u{7597}\u{836F}\u{6C34}"))
            .expect("selected item line");

        assert!(selected_line.starts_with('>'));
        assert!(selected_line.contains("\u{53EF}\u{4F7F}\u{7528}"));
    }

    #[test]
    fn inventory_popup_lines_should_mark_equipped_items_in_primary_row() {
        let mut snapshot = build_snapshot();
        snapshot.inventory_items = vec![InventoryItemView {
            name: "\u{751F}\u{9508}\u{77ED}\u{5251}".to_string(),
            qty: 1,
            group: InventoryGroup::Weapon,
            can_use: true,
            can_drop: false,
            equipped: true,
            action_label: "\u{53EF}\u{5378}\u{4E0B}".to_string(),
            attr_desc: "ATK+3".to_string(),
        }];

        let lines = inventory_popup_lines(&snapshot);
        let rendered = lines.iter().map(line_text).collect::<Vec<_>>();
        let item_line = rendered
            .iter()
            .find(|line| line.contains("\u{751F}\u{9508}\u{77ED}\u{5251}"))
            .expect("equipped line");

        assert!(item_line.contains("\u{5DF2}\u{88C5}\u{5907}"));
        assert!(item_line.contains("\u{53EF}\u{5378}\u{4E0B}"));
    }
}
