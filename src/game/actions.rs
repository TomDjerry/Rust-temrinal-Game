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
            Action::Wait
            | Action::Save
            | Action::Load
            | Action::ToggleInventory
            | Action::ToggleHelp
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
        let radius = Self::noise_radius_for_action(action)?;
        Some(NoiseEvent {
            pos: self.player.pos,
            radius,
        })
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
}

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn noise_radius_for_action_should_match_action_kind() {
        assert_eq!(Game::noise_radius_for_action(Action::Move(1, 0)), Some(6));
        assert_eq!(Game::noise_radius_for_action(Action::Pickup), Some(4));
        assert_eq!(Game::noise_radius_for_action(Action::Wait), None);
    }
}
