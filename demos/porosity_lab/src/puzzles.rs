use crate::state::{Edge, Node, NodeRole, Puzzle};

fn base_nodes() -> Vec<Node> {
    vec![
        Node {
            x: 92.0,
            y: 260.0,
            label: "Inlet",
            role: NodeRole::Inlet,
        },
        Node {
            x: 236.0,
            y: 138.0,
            label: "Upper pore",
            role: NodeRole::Internal,
        },
        Node {
            x: 236.0,
            y: 382.0,
            label: "Lower pore",
            role: NodeRole::Internal,
        },
        Node {
            x: 420.0,
            y: 260.0,
            label: "Central cage",
            role: NodeRole::Internal,
        },
        Node {
            x: 604.0,
            y: 138.0,
            label: "Upper outlet pore",
            role: NodeRole::Internal,
        },
        Node {
            x: 604.0,
            y: 382.0,
            label: "Lower outlet pore",
            role: NodeRole::Internal,
        },
        Node {
            x: 748.0,
            y: 260.0,
            label: "Outlet",
            role: NodeRole::Outlet,
        },
    ]
}

fn edge(
    from: usize,
    to: usize,
    label: &'static str,
    radius: f64,
    min_radius: f64,
    max_radius: f64,
) -> Edge {
    Edge {
        from,
        to,
        label,
        base_radius: radius,
        radius,
        min_radius,
        max_radius,
        editable: true,
    }
}

fn puzzle(
    title: &'static str,
    subtitle: &'static str,
    objective: &'static str,
    par_edits: usize,
    target_options: [f64; 2],
    edges: Vec<Edge>,
) -> Puzzle {
    Puzzle {
        title,
        subtitle,
        objective,
        par_edits,
        nodes: base_nodes(),
        initial_edges: edges.clone(),
        edges,
        target_options,
        active_target_index: 0,
    }
}

pub fn load_puzzles() -> Vec<Puzzle> {
    vec![
        puzzle(
            "Gate Sprint",
            "One choke point controls the run.",
            "Open one fast route.",
            2,
            [1.2, 1.4],
            vec![
                edge(0, 1, "upper inlet throat", 1.30, 0.60, 1.70),
                edge(0, 2, "lower inlet throat", 1.10, 0.50, 1.60),
                edge(1, 3, "upper central aperture", 0.85, 0.30, 1.50),
                edge(2, 3, "lower central aperture", 1.18, 0.40, 1.60),
                edge(3, 4, "upper outlet throat", 1.15, 0.50, 1.70),
                edge(3, 5, "lower outlet throat", 0.95, 0.30, 1.50),
                edge(4, 6, "upper exit aperture", 1.28, 0.60, 1.70),
                edge(5, 6, "lower exit aperture", 1.05, 0.40, 1.50),
            ],
        ),
        puzzle(
            "Split Dash",
            "Two repairs compete.",
            "Restore flow in the fewest moves.",
            2,
            [1.1, 1.3],
            vec![
                edge(0, 1, "upper inlet throat", 0.00, 0.00, 1.40),
                edge(0, 2, "lower inlet throat", 1.22, 0.50, 1.60),
                edge(1, 3, "upper central aperture", 1.08, 0.40, 1.50),
                edge(2, 3, "lower central aperture", 1.02, 0.30, 1.50),
                edge(3, 4, "upper outlet throat", 1.14, 0.50, 1.70),
                edge(3, 5, "lower outlet throat", 0.82, 0.20, 1.40),
                edge(4, 6, "upper exit aperture", 1.12, 0.40, 1.60),
                edge(5, 6, "lower exit aperture", 1.20, 0.50, 1.60),
            ],
        ),
        puzzle(
            "Needle Thread",
            "Several routes almost clear.",
            "Push one route just over the line.",
            3,
            [1.3, 1.5],
            vec![
                edge(0, 1, "upper inlet throat", 1.26, 0.60, 1.70),
                edge(0, 2, "lower inlet throat", 1.24, 0.60, 1.70),
                edge(1, 3, "upper central aperture", 1.18, 0.50, 1.60),
                edge(2, 3, "lower central aperture", 1.21, 0.50, 1.60),
                edge(3, 4, "upper outlet throat", 1.29, 0.60, 1.70),
                edge(3, 5, "lower outlet throat", 1.11, 0.40, 1.60),
                edge(4, 6, "upper exit aperture", 1.31, 0.60, 1.80),
                edge(5, 6, "lower exit aperture", 1.27, 0.50, 1.70),
            ],
        ),
    ]
}
