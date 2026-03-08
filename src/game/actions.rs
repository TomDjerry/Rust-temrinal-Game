use super::*;

impl Game {
    pub(super) fn noise_radius_for_action(action: Action) -> Option<i32> {
        match action {
            Action::Move(_, _) => Some(NOISE_RADIUS_MOVE),
            Action::Pickup
            | Action::UsePotion
            | Action::InventoryUse
            | Action::InventoryDrop
            | Action::InventoryUnequip => Some(NOISE_RADIUS_INTERACT),
            Action::CloseDoor => Some(NOISE_RADIUS_DOOR),
            Action::Wait
            | Action::Save
            | Action::Load
            | Action::ToggleInventory
            | Action::ToggleHelp
            | Action::ToggleLog
            | Action::Escape
            | Action::Quit => None,
        }
    }

    pub(super) fn apply_action(&mut self, action: Action) {
        match action {
            Action::Quit => {
                self.quit = true;
                return;
            }
            Action::Save => {
                match self.save_to_file(SAVE_FILE_PATH) {
                    Ok(()) => self.push_log(format!("存档成功：{SAVE_FILE_PATH}")),
                    Err(err) => self.push_log(format!("存档失败：{err:#}")),
                }
                return;
            }
            Action::Load => {
                let data = self.data.clone();
                match Self::load_from_file(SAVE_FILE_PATH, data) {
                    Ok(mut loaded) => {
                        loaded.push_log(format!("读档成功：{SAVE_FILE_PATH}"));
                        *self = loaded;
                    }
                    Err(err) => self.push_log(format!("读档失败：{err:#}")),
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
            Action::ToggleLog => {
                self.ui_mode = ui::transition_mode(self.ui_mode, 'l');
                if self.ui_mode == UiMode::Log {
                    self.log_scroll = 0;
                }
                return;
            }
            _ => {}
        }

        if self.ui_mode == UiMode::Help {
            return;
        }

        if self.ui_mode == UiMode::Log {
            match action {
                Action::Move(0, -1) => self.scroll_log_older(1),
                Action::Move(0, 1) => self.scroll_log_newer(1),
                _ => {}
            }
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
            Action::CloseDoor => consumed_turn = self.try_close_adjacent_door(),
            Action::Wait => {
                self.push_log("你选择等待一回合".to_string());
                consumed_turn = true;
            }
            Action::InventoryUse | Action::InventoryDrop | Action::InventoryUnequip => {}
            Action::Save
            | Action::Load
            | Action::ToggleInventory
            | Action::ToggleHelp
            | Action::ToggleLog
            | Action::Escape
            | Action::Quit => {}
        }

        if consumed_turn {
            if let Some(noise) = self.noise_from_action(action) {
                self.queue_noise(noise.pos, noise.radius);
            }
            self.finish_player_turn();
        }
    }

    fn noise_from_action(&self, action: Action) -> Option<NoiseEvent> {
        let radius = Self::noise_radius_for_action(action)?;
        Some(NoiseEvent {
            pos: self.player.pos,
            radius,
        })
    }

    fn queue_noise(&mut self, pos: Pos, radius: i32) {
        match self.pending_noise {
            Some(existing) if existing.radius >= radius => {}
            _ => self.pending_noise = Some(NoiseEvent { pos, radius }),
        }
    }

    fn scroll_log_older(&mut self, amount: usize) {
        let max_scroll = self.log.len().saturating_sub(1);
        self.log_scroll = (self.log_scroll + amount).min(max_scroll);
    }

    fn scroll_log_newer(&mut self, amount: usize) {
        self.log_scroll = self.log_scroll.saturating_sub(amount);
    }

    fn try_close_adjacent_door(&mut self) -> bool {
        let dirs = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        for (dx, dy) in dirs {
            let pos = Pos::new(self.player.pos.x + dx, self.player.pos.y + dy);
            if self
                .monsters
                .iter()
                .any(|monster| monster.pos == pos && monster.stats.is_alive())
            {
                continue;
            }
            if self
                .map
                .tile(pos)
                .is_some_and(|tile| matches!(tile.tile_type, crate::game::map::TileType::OpenDoor))
            {
                self.map
                    .set_tile_type(pos, crate::game::map::TileType::ClosedDoor);
                self.push_log("你关上了门".to_string());
                return true;
            }
        }
        self.push_log("附近没有可关闭的门".to_string());
        false
    }

    pub(in crate::game) fn trigger_trap_at_player_pos(&mut self) {
        let Some(index) = self
            .traps
            .iter()
            .position(|trap| trap.pos == self.player.pos && !trap.triggered)
        else {
            return;
        };
        let damage = self.traps[index].damage;
        self.traps[index].triggered = true;
        self.player.stats.hp -= damage;
        self.push_log(format!("你踩中了陷阱，受到 {damage} 点伤害"));
        self.queue_noise(self.player.pos, NOISE_RADIUS_TRAP);
    }

    fn finish_player_turn(&mut self) {
        if self.quit || self.won || !self.player.stats.is_alive() {
            return;
        }
        self.turn += 1;
        self.tick_contract_constraints();
        self.monster_turn();
        self.pending_noise = None;
        self.tick_active_buffs();
        self.cleanup_dead_monsters();
        self.check_victory();
        self.recompute_fov();
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{build_test_game, open_floor_map, test_monster};
    use super::super::*;
    use crate::game::map::TileType;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    #[test]
    fn noise_radius_for_action_should_match_action_kind() {
        assert_eq!(Game::noise_radius_for_action(Action::Move(1, 0)), Some(6));
        assert_eq!(Game::noise_radius_for_action(Action::Pickup), Some(4));
        assert_eq!(Game::noise_radius_for_action(Action::Wait), None);
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
    fn moving_into_closed_door_should_open_it_without_moving_player() {
        let mut game = build_test_game(301);
        game.monsters.clear();
        game.ground_items.clear();
        game.map = open_floor_map(8, 8, 1..=6, 1..=6);
        game.player.pos = Pos::new(2, 2);
        game.map.set_tile_type(Pos::new(3, 2), TileType::ClosedDoor);
        game.recompute_fov();
        let turn_before = game.turn;

        game.apply_action(Action::Move(1, 0));

        assert_eq!(game.player.pos, Pos::new(2, 2));
        assert_eq!(game.turn, turn_before + 1);
        assert_eq!(
            game.map.tile(Pos::new(3, 2)).expect("door").tile_type,
            TileType::OpenDoor
        );
    }

    #[test]
    fn trap_should_trigger_once_and_alert_nearby_monster() {
        let mut game = build_test_game(302);
        game.monsters.clear();
        game.ground_items.clear();
        game.map = open_floor_map(10, 8, 1..=8, 1..=6);
        game.player.pos = Pos::new(2, 2);
        game.traps = vec![Trap {
            pos: Pos::new(3, 2),
            damage: 3,
            triggered: false,
        }];
        game.monsters.push(test_monster(
            "watcher",
            "Watcher",
            'w',
            Pos::new(5, 2),
            Stats {
                hp: 6,
                max_hp: 6,
                atk: 2,
                def: 0,
            },
        ));
        let hp_before = game.player.stats.hp;

        game.apply_action(Action::Move(1, 0));

        assert_eq!(game.player.pos, Pos::new(3, 2));
        assert_eq!(game.player.stats.hp, hp_before - 3);
        assert!(game.traps[0].triggered);
        assert!(matches!(
            game.monsters[0].ai_state,
            MonsterAiState::Alert { .. }
        ));

        let hp_after_first = game.player.stats.hp;
        game.apply_action(Action::Move(-1, 0));
        game.apply_action(Action::Move(1, 0));
        assert_eq!(game.player.stats.hp, hp_after_first);
    }

    #[test]
    fn should_map_close_door_key() {
        let close = KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };

        assert!(matches!(
            action_from_key_event(close),
            Some(Action::CloseDoor)
        ));
    }

    #[test]
    fn should_map_log_key() {
        let log = KeyEvent {
            code: KeyCode::Char('l'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };

        assert!(matches!(
            action_from_key_event(log),
            Some(Action::ToggleLog)
        ));
    }

    #[test]
    fn close_door_action_should_close_adjacent_open_door() {
        let mut game = build_test_game(304);
        game.monsters.clear();
        game.ground_items.clear();
        game.map = open_floor_map(8, 8, 1..=6, 1..=6);
        game.player.pos = Pos::new(2, 2);
        game.map.set_tile_type(Pos::new(3, 2), TileType::OpenDoor);
        let turn_before = game.turn;

        game.apply_action(Action::CloseDoor);

        assert_eq!(game.turn, turn_before + 1);
        assert_eq!(
            game.map.tile(Pos::new(3, 2)).expect("door").tile_type,
            TileType::ClosedDoor
        );
    }

    #[test]
    fn close_door_without_adjacent_open_door_should_not_consume_turn() {
        let mut game = build_test_game(305);
        game.monsters.clear();
        game.ground_items.clear();
        game.map = open_floor_map(8, 8, 1..=6, 1..=6);
        game.player.pos = Pos::new(2, 2);
        let turn_before = game.turn;

        game.apply_action(Action::CloseDoor);

        assert_eq!(game.turn, turn_before);
        assert_eq!(
            game.log.back().map(String::as_str),
            Some("附近没有可关闭的门")
        );
    }

    #[test]
    fn trap_noise_should_fail_stealth_contract_when_it_alerts_monster() {
        let mut game = build_test_game(306);
        game.monsters.clear();
        game.ground_items.clear();
        game.map = open_floor_map(10, 8, 1..=8, 1..=6);
        game.player.pos = Pos::new(2, 2);
        game.traps = vec![Trap {
            pos: Pos::new(3, 2),
            damage: 3,
            triggered: false,
        }];
        game.side_contract = Some(SideContract {
            name: "stealth collect".to_string(),
            objective: ContractObjective::CollectItem {
                item_id: "healing_potion".to_string(),
                target: 1,
            },
            progress: 0,
            reward_item_id: "battle_tonic".to_string(),
            reward_qty: 1,
            completed: false,
            constraints: vec![ContractConstraint::Stealth { exposed: false }],
            failed: false,
            failure_reason: None,
        });
        game.monsters.push(test_monster(
            "watcher",
            "Watcher",
            'w',
            Pos::new(7, 2),
            Stats {
                hp: 6,
                max_hp: 6,
                atk: 2,
                def: 0,
            },
        ));

        game.apply_action(Action::Move(1, 0));

        assert!(game.traps[0].triggered);
        assert!(matches!(
            game.monsters[0].ai_state,
            MonsterAiState::Alert { .. }
        ));
        assert!(game.side_contract.as_ref().expect("contract").failed);
    }

    #[test]
    fn log_view_should_scroll_with_vertical_move_actions() {
        let mut game = build_test_game(307);
        for index in 0..20 {
            game.push_log(format!("日志 {index}"));
        }

        game.apply_action(Action::ToggleLog);
        game.apply_action(Action::Move(0, -1));

        assert_eq!(game.ui_mode, UiMode::Log);
        assert_eq!(game.log_scroll, 1);

        game.apply_action(Action::Move(0, 1));

        assert_eq!(game.log_scroll, 0);
    }
}
