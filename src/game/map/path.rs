use crate::game::map::{Map, Pos};
use std::collections::{HashMap, HashSet, VecDeque};

pub fn bfs_next_step(map: &Map, start: Pos, goal: Pos, blocked: &HashSet<Pos>) -> Option<Pos> {
    if start == goal {
        return None;
    }

    let mut frontier = VecDeque::new();
    let mut visited = HashSet::new();
    let mut parent = HashMap::new();

    frontier.push_back(start);
    visited.insert(start);

    while let Some(current) = frontier.pop_front() {
        for next in neighbors4(current) {
            if !map.in_bounds(next) || !map.is_walkable(next) {
                continue;
            }
            if next != goal && blocked.contains(&next) {
                continue;
            }
            if !visited.insert(next) {
                continue;
            }
            parent.insert(next, current);
            if next == goal {
                return backtrack_first_step(start, goal, &parent);
            }
            frontier.push_back(next);
        }
    }

    None
}

#[cfg(test)]
pub fn path_exists(map: &Map, start: Pos, goal: Pos) -> bool {
    let blocked = HashSet::new();
    bfs_next_step(map, start, goal, &blocked).is_some() || start == goal
}

fn neighbors4(pos: Pos) -> [Pos; 4] {
    [
        Pos::new(pos.x, pos.y - 1),
        Pos::new(pos.x, pos.y + 1),
        Pos::new(pos.x - 1, pos.y),
        Pos::new(pos.x + 1, pos.y),
    ]
}

fn backtrack_first_step(start: Pos, goal: Pos, parent: &HashMap<Pos, Pos>) -> Option<Pos> {
    let mut current = goal;
    while let Some(prev) = parent.get(&current).copied() {
        if prev == start {
            return Some(current);
        }
        current = prev;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::map::{Map, TileType};

    #[test]
    fn bfs_avoids_walls_and_finds_corridor() {
        let mut map = Map::new(7, 5);
        for y in 0..5 {
            for x in 0..7 {
                map.set_tile_type(Pos::new(x, y), TileType::Wall);
            }
        }

        for x in 1..=5 {
            map.set_tile_type(Pos::new(x, 2), TileType::Floor);
        }

        let step =
            bfs_next_step(&map, Pos::new(1, 2), Pos::new(5, 2), &HashSet::new()).expect("step");

        assert_eq!(step, Pos::new(2, 2));
    }

    #[test]
    fn bfs_respects_blocked_positions() {
        let mut map = Map::new(5, 5);
        for y in 0..5 {
            for x in 0..5 {
                map.set_tile_type(Pos::new(x, y), TileType::Floor);
            }
        }

        let blocked = HashSet::from([Pos::new(2, 1)]);
        let step = bfs_next_step(&map, Pos::new(1, 1), Pos::new(4, 1), &blocked).expect("step");

        assert_ne!(step, Pos::new(2, 1));
    }
}
