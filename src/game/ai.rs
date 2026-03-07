use super::*;
use crate::game::map::line_of_sight;
use crate::game::map::path::bfs_next_step;
use std::collections::HashSet;

impl MonsterAiState {
    pub(super) fn decay(self) -> Self {
        match self {
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
}

impl Game {
    pub(super) fn monster_turn(&mut self) {
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
            let previous_state = self.monsters[idx].ai_state;
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
                    self.monsters[idx].ai_state.decay()
                }
            } else {
                self.monsters[idx].ai_state.decay()
            };

            if self.can_progress_side_contract()
                && !matches!(previous_state, MonsterAiState::Alert { .. })
                && matches!(self.monsters[idx].ai_state, MonsterAiState::Alert { .. })
            {
                self.on_contract_alert_triggered();
            }

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
                    if let Some(step) = bfs_next_step(&self.map, monster_pos, target, &occupied)
                        && step != self.player.pos
                        && !occupied.contains(&step)
                    {
                        self.monsters[idx].pos = step;
                        moved = true;
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
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{build_test_game, open_floor_map, test_monster};
    use super::super::*;

    #[test]
    fn ai_state_decay_should_step_down_to_patrol() {
        let alert = MonsterAiState::Alert {
            target: Pos::new(3, 4),
            turns_left: 2,
        };
        let flee = MonsterAiState::Flee { turns_left: 1 };

        assert_eq!(
            alert.decay(),
            MonsterAiState::Alert {
                target: Pos::new(3, 4),
                turns_left: 1,
            }
        );
        assert_eq!(flee.decay(), MonsterAiState::Patrol);
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
        game.monsters.push(test_monster(
            "test",
            "Test",
            't',
            Pos::new(8, 2),
            Stats {
                hp: 8,
                max_hp: 8,
                atk: 3,
                def: 0,
            },
        ));
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

        game.map = open_floor_map(20, 20, 4..=8, 4..=8);
        game.player.pos = Pos::new(6, 6);
        game.player.stats.hp = 20;
        game.monsters.push(test_monster(
            "test",
            "Coward",
            'c',
            Pos::new(6, 7),
            Stats {
                hp: 1,
                max_hp: 9,
                atk: 6,
                def: 0,
            },
        ));
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
    fn monster_alert_should_fail_stealth_contract() {
        let mut game = build_test_game(91);
        game.monsters.clear();
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
        game.monsters.push(test_monster(
            "test",
            "Test",
            't',
            Pos::new(8, 2),
            Stats {
                hp: 8,
                max_hp: 8,
                atk: 3,
                def: 0,
            },
        ));
        game.pending_noise = Some(NoiseEvent {
            pos: game.player.pos,
            radius: 10,
        });

        game.monster_turn();

        assert!(game.side_contract.as_ref().expect("contract").failed);
    }
}
