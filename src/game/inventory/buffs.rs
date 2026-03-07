use super::super::*;

impl Game {
    pub(in crate::game) fn active_buff_bonus_totals(&self) -> (i32, i32) {
        let atk_bonus = self.active_buffs.iter().map(|buff| buff.atk_bonus).sum();
        let def_bonus = self.active_buffs.iter().map(|buff| buff.def_bonus).sum();
        (atk_bonus, def_bonus)
    }

    pub(in crate::game) fn tick_active_buffs(&mut self) {
        for buff in &mut self.active_buffs {
            if buff.turns_left > 0 {
                buff.turns_left -= 1;
            }
        }
        self.active_buffs.retain(|buff| buff.turns_left > 0);
    }
}
