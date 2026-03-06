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
}
