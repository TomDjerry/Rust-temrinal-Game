pub mod path;

use anyhow::{Result, bail};
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn manhattan(self, other: Pos) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }

    pub fn is_adjacent4(self, other: Pos) -> bool {
        self.manhattan(other) == 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileType {
    Wall,
    Floor,
    Exit,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Tile {
    pub tile_type: TileType,
    pub explored: bool,
}

impl Tile {
    pub fn is_walkable(self) -> bool {
        !matches!(self.tile_type, TileType::Wall)
    }

    pub fn blocks_vision(self) -> bool {
        matches!(self.tile_type, TileType::Wall)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Map {
    pub width: i32,
    pub height: i32,
    pub tiles: Vec<Tile>,
}

impl Map {
    pub fn new(width: i32, height: i32) -> Self {
        let tile = Tile {
            tile_type: TileType::Wall,
            explored: false,
        };
        Self {
            width,
            height,
            tiles: vec![tile; (width * height) as usize],
        }
    }

    pub fn in_bounds(&self, pos: Pos) -> bool {
        pos.x >= 0 && pos.x < self.width && pos.y >= 0 && pos.y < self.height
    }

    fn idx(&self, pos: Pos) -> usize {
        (pos.y * self.width + pos.x) as usize
    }

    pub fn tile(&self, pos: Pos) -> Option<Tile> {
        self.in_bounds(pos).then(|| self.tiles[self.idx(pos)])
    }

    pub fn tile_mut(&mut self, pos: Pos) -> Option<&mut Tile> {
        if !self.in_bounds(pos) {
            return None;
        }
        let idx = self.idx(pos);
        Some(&mut self.tiles[idx])
    }

    pub fn is_walkable(&self, pos: Pos) -> bool {
        self.tile(pos).is_some_and(Tile::is_walkable)
    }

    pub fn set_tile_type(&mut self, pos: Pos, tile_type: TileType) {
        if let Some(tile) = self.tile_mut(pos) {
            tile.tile_type = tile_type;
        }
    }

    pub fn mark_explored(&mut self, pos: Pos) {
        if let Some(tile) = self.tile_mut(pos) {
            tile.explored = true;
        }
    }

    pub fn is_explored(&self, pos: Pos) -> bool {
        self.tile(pos).is_some_and(|t| t.explored)
    }

    pub fn base_glyph(&self, pos: Pos) -> char {
        let Some(tile) = self.tile(pos) else {
            return ' ';
        };

        match tile.tile_type {
            TileType::Wall => '#',
            TileType::Floor => '.',
            TileType::Exit => 'E',
        }
    }
}

#[derive(Debug, Clone)]
pub struct DungeonLayout {
    pub map: Map,
    pub player_start: Pos,
    pub package_pos: Pos,
    pub exit_pos: Pos,
    pub monster_spawns: Vec<Pos>,
    pub item_spawns: Vec<Pos>,
}

#[derive(Debug, Clone)]
struct Room {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl Room {
    fn center(&self) -> Pos {
        Pos::new((self.x1 + self.x2) / 2, (self.y1 + self.y2) / 2)
    }

    fn intersects(&self, other: &Room) -> bool {
        self.x1 <= other.x2 && self.x2 >= other.x1 && self.y1 <= other.y2 && self.y2 >= other.y1
    }
}

pub fn generate_dungeon(width: i32, height: i32, rng: &mut StdRng) -> Result<DungeonLayout> {
    if width < 20 || height < 20 {
        bail!("map size too small")
    }

    let mut map = Map::new(width, height);
    let mut rooms = Vec::new();

    for _ in 0..80 {
        let room_w = rng.random_range(5..=10);
        let room_h = rng.random_range(5..=9);
        let x = rng.random_range(1..(width - room_w - 1));
        let y = rng.random_range(1..(height - room_h - 1));
        let room = Room {
            x1: x,
            y1: y,
            x2: x + room_w,
            y2: y + room_h,
        };

        if rooms.iter().any(|other: &Room| room.intersects(other)) {
            continue;
        }

        carve_room(&mut map, &room);

        if let Some(prev) = rooms.last() {
            let start = prev.center();
            let end = room.center();
            carve_corridor(&mut map, start, end, rng.random_bool(0.5));
        }

        rooms.push(room);
        if rooms.len() >= 12 {
            break;
        }
    }

    if rooms.len() < 3 {
        bail!("failed to create enough rooms")
    }

    let player_start = rooms[0].center();
    let package_pos = rooms[rooms.len() / 2].center();
    let exit_pos = rooms[rooms.len() - 1].center();
    map.set_tile_type(exit_pos, TileType::Exit);

    let mut floor_tiles = Vec::new();
    for y in 1..(height - 1) {
        for x in 1..(width - 1) {
            let pos = Pos::new(x, y);
            if map.is_walkable(pos) && pos != player_start && pos != package_pos && pos != exit_pos
            {
                floor_tiles.push(pos);
            }
        }
    }
    floor_tiles.shuffle(rng);

    let monster_spawns = floor_tiles.iter().copied().take(20).collect::<Vec<_>>();
    let item_spawns = floor_tiles
        .iter()
        .copied()
        .skip(20)
        .take(16)
        .collect::<Vec<_>>();

    Ok(DungeonLayout {
        map,
        player_start,
        package_pos,
        exit_pos,
        monster_spawns,
        item_spawns,
    })
}

fn carve_room(map: &mut Map, room: &Room) {
    for y in room.y1..=room.y2 {
        for x in room.x1..=room.x2 {
            map.set_tile_type(Pos::new(x, y), TileType::Floor);
        }
    }
}

fn carve_corridor(map: &mut Map, start: Pos, end: Pos, horizontal_first: bool) {
    if horizontal_first {
        for x in range_inclusive(start.x, end.x) {
            map.set_tile_type(Pos::new(x, start.y), TileType::Floor);
        }
        for y in range_inclusive(start.y, end.y) {
            map.set_tile_type(Pos::new(end.x, y), TileType::Floor);
        }
    } else {
        for y in range_inclusive(start.y, end.y) {
            map.set_tile_type(Pos::new(start.x, y), TileType::Floor);
        }
        for x in range_inclusive(start.x, end.x) {
            map.set_tile_type(Pos::new(x, end.y), TileType::Floor);
        }
    }
}

fn range_inclusive(a: i32, b: i32) -> Vec<i32> {
    if a <= b {
        (a..=b).collect()
    } else {
        (b..=a).rev().collect()
    }
}

pub fn compute_fov(map: &Map, origin: Pos, radius: i32) -> HashSet<Pos> {
    let mut visible = HashSet::new();
    for y in (origin.y - radius)..=(origin.y + radius) {
        for x in (origin.x - radius)..=(origin.x + radius) {
            let pos = Pos::new(x, y);
            if !map.in_bounds(pos) {
                continue;
            }
            if origin.manhattan(pos) > radius {
                continue;
            }
            if line_of_sight(map, origin, pos) {
                visible.insert(pos);
            }
        }
    }
    visible.insert(origin);
    visible
}

pub fn line_of_sight(map: &Map, from: Pos, to: Pos) -> bool {
    let mut x0 = from.x;
    let mut y0 = from.y;
    let x1 = to.x;
    let y1 = to.y;

    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        let pos = Pos::new(x0, y0);
        if pos != from && pos != to && map.tile(pos).is_some_and(Tile::blocks_vision) {
            return false;
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::map::path::path_exists;

    #[test]
    fn generated_map_is_connected_for_main_objective() {
        let mut rng = StdRng::seed_from_u64(7);
        let layout = generate_dungeon(60, 26, &mut rng).expect("layout");
        assert!(path_exists(
            &layout.map,
            layout.player_start,
            layout.package_pos
        ));
        assert!(path_exists(
            &layout.map,
            layout.package_pos,
            layout.exit_pos
        ));
    }

    #[test]
    fn same_seed_produces_same_layout() {
        let mut rng_a = StdRng::seed_from_u64(1234);
        let mut rng_b = StdRng::seed_from_u64(1234);

        let layout_a = generate_dungeon(60, 26, &mut rng_a).expect("layout");
        let layout_b = generate_dungeon(60, 26, &mut rng_b).expect("layout");

        let kinds_a: Vec<_> = layout_a.map.tiles.iter().map(|t| t.tile_type).collect();
        let kinds_b: Vec<_> = layout_b.map.tiles.iter().map(|t| t.tile_type).collect();

        assert_eq!(layout_a.player_start, layout_b.player_start);
        assert_eq!(layout_a.package_pos, layout_b.package_pos);
        assert_eq!(layout_a.exit_pos, layout_b.exit_pos);
        assert_eq!(kinds_a, kinds_b);
    }
}
