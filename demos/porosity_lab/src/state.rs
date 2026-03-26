use crate::puzzles::load_puzzles;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    Inlet,
    Outlet,
    Internal,
}

#[derive(Clone)]
pub struct Node {
    pub x: f64,
    pub y: f64,
    pub label: &'static str,
    pub role: NodeRole,
}

#[derive(Clone)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub label: &'static str,
    pub base_radius: f64,
    pub radius: f64,
    pub min_radius: f64,
    pub max_radius: f64,
    pub editable: bool,
}

impl Edge {
    pub fn is_blocked(&self) -> bool {
        self.radius <= 0.05
    }
}

#[derive(Clone)]
pub struct Puzzle {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub objective: &'static str,
    pub par_edits: usize,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub initial_edges: Vec<Edge>,
    pub target_options: [f64; 2],
    pub active_target_index: usize,
}

impl Puzzle {
    pub fn active_target(&self) -> f64 {
        self.target_options[self.active_target_index]
    }

    pub fn baseline_snapshot(&self) -> Self {
        let mut snapshot = self.clone();
        snapshot.edges = snapshot.initial_edges.clone();
        snapshot.active_target_index = self.active_target_index;
        snapshot
    }
}

pub struct AppState {
    puzzles: Vec<Puzzle>,
    current_index: usize,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            puzzles: load_puzzles(),
            current_index: 0,
        }
    }

    pub fn current_index(&self) -> usize {
        self.current_index
    }

    pub fn puzzle_count(&self) -> usize {
        self.puzzles.len()
    }

    pub fn current_puzzle(&self) -> &Puzzle {
        &self.puzzles[self.current_index]
    }

    pub fn current_puzzle_mut(&mut self) -> &mut Puzzle {
        &mut self.puzzles[self.current_index]
    }

    pub fn reset_current(&mut self) {
        let puzzle = self.current_puzzle_mut();
        puzzle.edges = puzzle.initial_edges.clone();
        puzzle.active_target_index = 0;
    }

    pub fn next_puzzle(&mut self) {
        self.current_index = (self.current_index + 1) % self.puzzles.len();
    }

    pub fn toggle_probe(&mut self) {
        let puzzle = self.current_puzzle_mut();
        puzzle.active_target_index = (puzzle.active_target_index + 1) % puzzle.target_options.len();
    }

    pub fn set_edge_radius(&mut self, edge_index: usize, radius: f64) {
        if let Some(edge) = self.current_puzzle_mut().edges.get_mut(edge_index) {
            if !edge.editable {
                return;
            }
            edge.radius = radius.clamp(edge.min_radius, edge.max_radius);
        }
    }

    pub fn cycle_edge(&mut self, edge_index: usize) {
        let Some(edge) = self.current_puzzle_mut().edges.get_mut(edge_index) else {
            return;
        };
        if !edge.editable {
            return;
        }

        let next = (edge.radius * 10.0).round() / 10.0 + 0.2;
        edge.radius = if next > edge.max_radius + 1e-6 {
            edge.min_radius
        } else {
            next.clamp(edge.min_radius, edge.max_radius)
        };
    }
}
