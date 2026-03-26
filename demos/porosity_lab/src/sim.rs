use std::collections::VecDeque;

use crate::levels::{index, Cell, LevelSpec, BOARD_H, BOARD_W};

pub struct Simulation {
    pub reachable: Vec<bool>,
    pub clearance: Vec<u8>,
    pub accessible_count: usize,
    pub accessible_ratio: f64,
    pub adsorbed_sites: usize,
    pub probe_pass: [bool; 3],
    pub probe_paths: [Vec<(i32, i32)>; 3],
    pub best_probe_tier: usize,
}

pub fn analyze(board: &[Cell], level: &LevelSpec) -> Simulation {
    let reachable = reachable_cells(board, level.sources);
    let clearance = compute_clearance(board);

    let accessible_count = reachable.iter().filter(|value| **value).count();
    let accessible_ratio = accessible_count as f64 / (BOARD_W * BOARD_H) as f64;
    let adsorbed_sites = level
        .pockets
        .iter()
        .filter_map(|&(x, y)| index(x, y))
        .filter(|&cell_index| reachable[cell_index])
        .count();

    let mut probe_pass = [false; 3];
    let mut probe_paths = std::array::from_fn(|_| Vec::new());
    let mut best_probe_tier = 0;

    for tier in 1..=3 {
        if !level.outlets.is_empty() {
            let (passed, path) = path_for_tier(board, &clearance, level.sources, level.outlets, tier);
            probe_pass[tier - 1] = passed;
            probe_paths[tier - 1] = path;
            if passed {
                best_probe_tier = tier;
            }
        } else if reachable
            .iter()
            .enumerate()
            .any(|(cell_index, open)| *open && clearance[cell_index] >= tier as u8)
        {
            best_probe_tier = tier;
        }
    }

    Simulation {
        reachable,
        clearance,
        accessible_count,
        accessible_ratio,
        adsorbed_sites,
        probe_pass,
        probe_paths,
        best_probe_tier,
    }
}

fn reachable_cells(board: &[Cell], sources: &[(i32, i32)]) -> Vec<bool> {
    let mut visited = vec![false; BOARD_W * BOARD_H];
    let mut queue = VecDeque::new();

    for &(x, y) in sources {
        if let Some(cell_index) = index(x, y) {
            if board[cell_index].is_open() && !visited[cell_index] {
                visited[cell_index] = true;
                queue.push_back((x, y));
            }
        }
    }

    while let Some((x, y)) = queue.pop_front() {
        for (nx, ny) in neighbors4(x, y) {
            if let Some(cell_index) = index(nx, ny) {
                if !visited[cell_index] && board[cell_index].is_open() {
                    visited[cell_index] = true;
                    queue.push_back((nx, ny));
                }
            }
        }
    }

    visited
}

fn compute_clearance(board: &[Cell]) -> Vec<u8> {
    let mut clearance = vec![0; BOARD_W * BOARD_H];

    for y in 0..BOARD_H as i32 {
        for x in 0..BOARD_W as i32 {
            let Some(cell_index) = index(x, y) else {
                continue;
            };

            if !board[cell_index].is_open() {
                continue;
            }

            clearance[cell_index] = if included_in_square(board, x, y, 3) {
                3
            } else if included_in_square(board, x, y, 2) {
                2
            } else {
                1
            };
        }
    }

    clearance
}

fn included_in_square(board: &[Cell], x: i32, y: i32, size: i32) -> bool {
    for offset_y in 0..size {
        for offset_x in 0..size {
            let left = x - offset_x;
            let top = y - offset_y;
            if open_square(board, left, top, size) {
                return true;
            }
        }
    }
    false
}

fn open_square(board: &[Cell], left: i32, top: i32, size: i32) -> bool {
    if left < 0 || top < 0 || left + size > BOARD_W as i32 || top + size > BOARD_H as i32 {
        return false;
    }

    for yy in top..top + size {
        for xx in left..left + size {
            let Some(cell_index) = index(xx, yy) else {
                return false;
            };
            if !board[cell_index].is_open() {
                return false;
            }
        }
    }

    true
}

fn path_for_tier(
    board: &[Cell],
    clearance: &[u8],
    sources: &[(i32, i32)],
    outlets: &[(i32, i32)],
    tier: usize,
) -> (bool, Vec<(i32, i32)>) {
    let mut visited = vec![false; BOARD_W * BOARD_H];
    let mut previous = vec![None; BOARD_W * BOARD_H];
    let mut queue = VecDeque::new();
    let outlet_center = outlet_center(outlets);
    let outlet_indices: Vec<usize> = outlets.iter().filter_map(|&(x, y)| index(x, y)).collect();

    for &(x, y) in sources {
        if let Some(cell_index) = index(x, y) {
            if board[cell_index].is_open() && clearance[cell_index] >= tier as u8 && !visited[cell_index] {
                visited[cell_index] = true;
                queue.push_back((x, y));
            }
        }
    }

    let mut best_partial: Option<(usize, i32, i32)> = None;

    while let Some((x, y)) = queue.pop_front() {
        let Some(cell_index) = index(x, y) else {
            continue;
        };

        if outlet_indices.contains(&cell_index) {
            return (true, reconstruct_path(&previous, cell_index));
        }

        match best_partial {
            Some((_, best_y, best_dx)) => {
                let dx = (x - outlet_center).abs();
                if y < best_y || (y == best_y && dx < best_dx) {
                    best_partial = Some((cell_index, y, dx));
                }
            }
            None => best_partial = Some((cell_index, y, (x - outlet_center).abs())),
        }

        for (nx, ny) in neighbors4(x, y) {
            if let Some(next_index) = index(nx, ny) {
                if !visited[next_index]
                    && board[next_index].is_open()
                    && clearance[next_index] >= tier as u8
                {
                    visited[next_index] = true;
                    previous[next_index] = Some(cell_index);
                    queue.push_back((nx, ny));
                }
            }
        }
    }

    let path = best_partial
        .map(|(cell_index, _, _)| reconstruct_path(&previous, cell_index))
        .unwrap_or_default();

    (false, path)
}

fn reconstruct_path(previous: &[Option<usize>], end: usize) -> Vec<(i32, i32)> {
    let mut path = Vec::new();
    let mut cursor = Some(end);

    while let Some(cell_index) = cursor {
        let x = (cell_index % BOARD_W) as i32;
        let y = (cell_index / BOARD_W) as i32;
        path.push((x, y));
        cursor = previous[cell_index];
    }

    path.reverse();
    path
}

fn outlet_center(outlets: &[(i32, i32)]) -> i32 {
    if outlets.is_empty() {
        return (BOARD_W / 2) as i32;
    }

    let sum: i32 = outlets.iter().map(|(x, _)| *x).sum();
    sum / outlets.len() as i32
}

fn neighbors4(x: i32, y: i32) -> [(i32, i32); 4] {
    [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
}
