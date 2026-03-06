use rand::Rng;

pub fn calculate_damage(atk: i32, def: i32, variance: i32) -> i32 {
    (atk - def + variance).max(1)
}

pub fn roll_damage<R: Rng>(atk: i32, def: i32, rng: &mut R) -> i32 {
    let variance = rng.random_range(-1..=1);
    calculate_damage(atk, def, variance)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{build_test_game, open_floor_map, test_monster};
    use super::super::*;
    use super::*;

    #[test]
    fn damage_has_minimum_one() {
        assert_eq!(calculate_damage(1, 9, -1), 1);
        assert_eq!(calculate_damage(5, 8, 0), 1);
    }

    #[test]
    fn damage_follows_formula_boundaries() {
        assert_eq!(calculate_damage(10, 4, -1), 5);
        assert_eq!(calculate_damage(10, 4, 0), 6);
        assert_eq!(calculate_damage(10, 4, 1), 7);
    }

    #[test]
    fn guaranteed_crit_equipment_should_double_attack_damage() {
        let mut baseline = build_test_game(23);
        let mut crit_game = build_test_game(23);
        baseline.monsters.clear();
        crit_game.monsters.clear();

        let map = open_floor_map(20, 20, 4..=8, 4..=8);
        baseline.map = map.clone();
        crit_game.map = map;
        baseline.player.pos = Pos::new(5, 5);
        crit_game.player.pos = Pos::new(5, 5);

        let monster = test_monster(
            "test",
            "Dummy",
            'd',
            Pos::new(6, 5),
            Stats {
                hp: 50,
                max_hp: 50,
                atk: 1,
                def: 0,
            },
        );
        baseline.monsters.push(monster.clone());
        crit_game.monsters.push(monster);

        let added = crit_game.add_item_to_inventory("precision_dagger", 1);
        assert_eq!(added, 1);
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

        game.map = open_floor_map(20, 20, 4..=8, 4..=8);
        game.player.pos = Pos::new(6, 6);
        game.monsters.push(test_monster(
            "test",
            "Striker",
            's',
            Pos::new(6, 7),
            Stats {
                hp: 12,
                max_hp: 12,
                atk: 6,
                def: 0,
            },
        ));
        let added = game.add_item_to_inventory("feather_cloak", 1);
        assert_eq!(added, 1);
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

        let map = open_floor_map(20, 20, 4..=8, 4..=8);
        baseline.map = map.clone();
        pen_game.map = map;
        baseline.player.pos = Pos::new(5, 5);
        pen_game.player.pos = Pos::new(5, 5);

        let monster = test_monster(
            "tank",
            "Tank",
            't',
            Pos::new(6, 5),
            Stats {
                hp: 50,
                max_hp: 50,
                atk: 1,
                def: 9,
            },
        );
        baseline.monsters.push(monster.clone());
        pen_game.monsters.push(monster);

        let added = pen_game.add_item_to_inventory("armor_breaker", 1);
        assert_eq!(added, 1);
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

        game.map = open_floor_map(20, 20, 4..=8, 4..=8);
        game.player.pos = Pos::new(6, 6);
        game.player.stats.hp = 20;
        game.monsters.push(test_monster(
            "brute",
            "Brute",
            'b',
            Pos::new(6, 7),
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 8,
                def: 0,
            },
        ));

        let added = game.add_item_to_inventory("tower_plate", 1);
        assert_eq!(added, 1);
        assert!(game.try_equip_item("tower_plate"));

        game.monster_turn();

        assert!(game.player.stats.hp >= 19);
    }
}
