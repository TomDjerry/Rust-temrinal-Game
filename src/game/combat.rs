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
}
