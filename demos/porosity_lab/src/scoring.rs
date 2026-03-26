use crate::state::Puzzle;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    Open,
    Bottlenecked,
    Blocked,
}

impl Classification {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "Open pathway",
            Self::Bottlenecked => "Bottlenecked",
            Self::Blocked => "Blocked",
        }
    }

    pub fn rank(self) -> i32 {
        match self {
            Self::Blocked => 0,
            Self::Bottlenecked => 1,
            Self::Open => 2,
        }
    }
}

pub struct Analysis {
    pub classification: Classification,
    pub accessibility_score: f64,
    pub best_bottleneck: Option<f64>,
    pub modification_cost: f64,
    pub changed_edges: usize,
    pub best_path_edges: Vec<usize>,
    pub limiting_edge: Option<usize>,
    pub largest_reachable_cavity: f64,
    pub largest_reachable_node: Option<usize>,
    pub summary_line: String,
    pub threshold_margin: f64,
}

pub fn analyze(puzzle: &Puzzle) -> Analysis {
    let start = 0usize;
    let goal = puzzle.nodes.len() - 1;
    let node_count = puzzle.nodes.len();

    let reachable = reachable_nodes(puzzle, start);
    let path_continuity = reachable.iter().filter(|&&seen| seen).count() as f64 / node_count as f64;

    let (best_bottleneck, best_path_edges) = widest_path(puzzle, start, goal);
    let route_exists = best_bottleneck.is_some();
    let target = puzzle.active_target();
    let (largest_reachable_cavity, largest_reachable_node) =
        largest_reachable_cavity(puzzle, &reachable);

    let classification = match best_bottleneck {
        Some(value) if value + 1e-6 >= target => Classification::Open,
        Some(_) => Classification::Bottlenecked,
        None => Classification::Blocked,
    };

    let limiting_edge = best_path_edges.iter().copied().min_by(|left, right| {
        puzzle.edges[*left]
            .radius
            .partial_cmp(&puzzle.edges[*right].radius)
            .unwrap()
    });

    let editable_edges = puzzle
        .edges
        .iter()
        .filter(|edge| edge.editable)
        .count()
        .max(1) as f64;
    let modification_cost = puzzle
        .edges
        .iter()
        .filter(|edge| edge.editable)
        .map(|edge| {
            let span = (edge.max_radius - edge.min_radius).max(0.2);
            (edge.radius - edge.base_radius).abs() / span
        })
        .sum::<f64>()
        / editable_edges;

    let changed_edges = puzzle
        .edges
        .iter()
        .filter(|edge| (edge.radius - edge.base_radius).abs() > 0.04)
        .count();

    let target_fit = best_bottleneck
        .map(|value| (value / target).clamp(0.0, 1.25))
        .unwrap_or(0.0);
    let continuity_component = if route_exists {
        0.75 + 0.25 * path_continuity
    } else {
        path_continuity * 0.55
    };
    let intervention_component = (1.0 - modification_cost).clamp(0.0, 1.0);
    let accessibility_score = (100.0
        * (0.55 * target_fit + 0.25 * continuity_component + 0.20 * intervention_component))
        .clamp(0.0, 100.0);

    let summary_line = match classification {
        Classification::Open => format!(
            "Run clear. Tight gate {:.2}.",
            best_bottleneck.unwrap_or(0.0)
        ),
        Classification::Bottlenecked => format!(
            "Route found, but {:.2} misses {:.1}.",
            best_bottleneck.unwrap_or(0.0),
            target
        ),
        Classification::Blocked => "No continuous run.".to_string(),
    };

    Analysis {
        classification,
        accessibility_score,
        best_bottleneck,
        modification_cost,
        changed_edges,
        best_path_edges,
        limiting_edge,
        largest_reachable_cavity,
        largest_reachable_node,
        summary_line,
        threshold_margin: best_bottleneck
            .map(|value| value - target)
            .unwrap_or(-target),
    }
}

fn largest_reachable_cavity(puzzle: &Puzzle, reachable: &[bool]) -> (f64, Option<usize>) {
    let mut best = 0.0;
    let mut best_node = None;

    for (node_index, seen) in reachable.iter().enumerate() {
        if !seen {
            continue;
        }

        let local_gate = puzzle
            .edges
            .iter()
            .filter(|edge| !edge.is_blocked() && (edge.from == node_index || edge.to == node_index))
            .map(|edge| edge.radius)
            .fold(0.0, f64::max);

        if local_gate <= 0.0 {
            continue;
        }

        let cavity = local_gate + 0.35;
        if cavity > best {
            best = cavity;
            best_node = Some(node_index);
        }
    }

    (best, best_node)
}

fn reachable_nodes(puzzle: &Puzzle, start: usize) -> Vec<bool> {
    let mut seen = vec![false; puzzle.nodes.len()];
    let mut stack = vec![start];
    seen[start] = true;

    while let Some(node) = stack.pop() {
        for edge in &puzzle.edges {
            if edge.is_blocked() {
                continue;
            }
            let next = if edge.from == node {
                edge.to
            } else if edge.to == node {
                edge.from
            } else {
                continue;
            };

            if !seen[next] {
                seen[next] = true;
                stack.push(next);
            }
        }
    }

    seen
}

fn widest_path(puzzle: &Puzzle, start: usize, goal: usize) -> (Option<f64>, Vec<usize>) {
    let n = puzzle.nodes.len();
    let mut best = vec![0.0; n];
    let mut visited = vec![false; n];
    let mut prev_node = vec![None; n];
    let mut prev_edge = vec![None; n];

    best[start] = f64::INFINITY;

    for _ in 0..n {
        let mut current = None;
        let mut current_best = 0.0;
        for index in 0..n {
            if !visited[index] && best[index] > current_best {
                current_best = best[index];
                current = Some(index);
            }
        }

        let Some(node) = current else {
            break;
        };

        if node == goal {
            break;
        }

        visited[node] = true;

        for (edge_index, edge) in puzzle.edges.iter().enumerate() {
            if edge.is_blocked() {
                continue;
            }
            let next = if edge.from == node {
                edge.to
            } else if edge.to == node {
                edge.from
            } else {
                continue;
            };

            let candidate = best[node].min(edge.radius);
            if candidate > best[next] {
                best[next] = candidate;
                prev_node[next] = Some(node);
                prev_edge[next] = Some(edge_index);
            }
        }
    }

    if best[goal] <= 0.0 {
        return (None, Vec::new());
    }

    let mut path_edges = Vec::new();
    let mut cursor = goal;
    while cursor != start {
        let Some(edge_index) = prev_edge[cursor] else {
            break;
        };
        path_edges.push(edge_index);
        cursor = prev_node[cursor].unwrap_or(start);
    }
    path_edges.reverse();

    (Some(best[goal]), path_edges)
}
