use super::*;
use std::ops::RangeInclusive;

pub(crate) fn build_test_game(seed: u64) -> Game {
    let data = GameData::load("assets").expect("assets");
    let config = GameConfig {
        seed: Some(seed),
        width: 40,
        height: 22,
    };
    Game::new(config, seed, data).expect("game")
}

pub(crate) fn open_floor_map(
    width: i32,
    height: i32,
    x_range: RangeInclusive<i32>,
    y_range: RangeInclusive<i32>,
) -> Map {
    let mut map = Map::new(width, height);
    for y in y_range.clone() {
        for x in x_range.clone() {
            map.set_tile_type(Pos::new(x, y), map::TileType::Floor);
        }
    }
    map
}

pub(crate) fn test_monster(
    kind_id: &str,
    name: &str,
    glyph: char,
    pos: Pos,
    stats: Stats,
) -> Monster {
    Monster {
        kind_id: kind_id.to_string(),
        name: name.to_string(),
        glyph,
        pos,
        stats,
        ai_state: MonsterAiState::Patrol,
    }
}
