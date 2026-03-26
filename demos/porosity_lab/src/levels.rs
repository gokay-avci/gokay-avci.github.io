pub const BOARD_W: usize = 10;
pub const BOARD_H: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Matrix,
    Wall,
    Channel(u8),
    Source,
    Outlet,
    Pocket,
}

impl Cell {
    pub fn is_open(self) -> bool {
        matches!(self, Cell::Channel(_) | Cell::Source | Cell::Outlet | Cell::Pocket)
    }

    pub fn blocks_piece(self) -> bool {
        !matches!(self, Cell::Matrix)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PieceKind {
    Tetra,
    Cube,
    Octa,
    Dodeca,
    Icosa,
}

impl PieceKind {
    pub fn label(self) -> &'static str {
        match self {
            PieceKind::Tetra => "Tetra",
            PieceKind::Cube => "Cube",
            PieceKind::Octa => "Octa",
            PieceKind::Dodeca => "Dodeca",
            PieceKind::Icosa => "Icosa",
        }
    }

    pub fn palette_index(self) -> u8 {
        match self {
            PieceKind::Tetra => 0,
            PieceKind::Cube => 1,
            PieceKind::Octa => 2,
            PieceKind::Dodeca => 3,
            PieceKind::Icosa => 4,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ActivePiece {
    pub kind: PieceKind,
    pub rotation: usize,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy)]
pub struct Candidate {
    pub kind: PieceKind,
    pub rotation: usize,
    pub badge: &'static str,
    pub origin: &'static str,
    pub fitness: u8,
    pub mutation: u8,
}

#[derive(Clone, Copy)]
pub struct GeneratorKnobs {
    pub cavern: f64,
    pub throat: f64,
    pub branch: f64,
}

#[derive(Clone, Copy)]
pub enum ObjectiveKind {
    AccessibleVolume { target: usize },
    ProbeToVent { tier: usize },
    AdsorbPockets { target: usize },
}

pub struct LevelSpec {
    pub name: &'static str,
    pub subtitle: &'static str,
    pub objective: &'static str,
    pub phase_badge: &'static str,
    pub cue_title: &'static str,
    pub cue_text: &'static str,
    pub concept_title: &'static str,
    pub concept_text: &'static str,
    pub objective_kind: ObjectiveKind,
    pub turn_limit: usize,
    pub sources: &'static [(i32, i32)],
    pub outlets: &'static [(i32, i32)],
    pub pockets: &'static [(i32, i32)],
    pub walls: &'static [(i32, i32)],
    pub seeds: &'static [(i32, i32)],
}

pub fn piece_cells(kind: PieceKind, rotation: usize) -> &'static [(i32, i32)] {
    match kind {
        PieceKind::Tetra => match rotation % 4 {
            0 => &[(1, 0), (0, 1), (1, 1), (2, 1)],
            1 => &[(0, 0), (0, 1), (1, 1), (0, 2)],
            2 => &[(0, 0), (1, 0), (2, 0), (1, 1)],
            _ => &[(1, 0), (0, 1), (1, 1), (1, 2)],
        },
        PieceKind::Cube => &[(0, 0), (1, 0), (0, 1), (1, 1)],
        PieceKind::Octa => match rotation % 4 {
            0 => &[(1, 0), (0, 1), (1, 1), (2, 1), (1, 2)],
            1 => &[(1, 0), (0, 1), (1, 1), (2, 1), (1, 2)],
            2 => &[(1, 0), (0, 1), (1, 1), (2, 1), (1, 2)],
            _ => &[(1, 0), (0, 1), (1, 1), (2, 1), (1, 2)],
        },
        PieceKind::Dodeca => match rotation % 4 {
            0 => &[(1, 0), (0, 1), (1, 1), (2, 1), (2, 2)],
            1 => &[(1, 0), (1, 1), (2, 1), (0, 2), (1, 2)],
            2 => &[(0, 0), (0, 1), (1, 1), (2, 1), (1, 2)],
            _ => &[(1, 0), (0, 1), (1, 1), (1, 2), (2, 2)],
        },
        PieceKind::Icosa => match rotation % 4 {
            0 => &[(0, 1), (1, 0), (1, 1), (2, 1), (3, 1)],
            1 => &[(1, 0), (1, 1), (1, 2), (0, 2), (2, 2)],
            2 => &[(0, 0), (1, 0), (2, 0), (2, 1), (3, 0)],
            _ => &[(1, 0), (0, 1), (1, 1), (1, 2), (1, 3)],
        },
    }
}

pub fn poly_points(kind: PieceKind, rotation: usize) -> &'static [(f64, f64)] {
    match kind {
        PieceKind::Tetra => match rotation % 4 {
            0 => &[(0.5, 0.06), (0.08, 0.78), (0.92, 0.78)],
            1 => &[(0.14, 0.2), (0.78, 0.1), (0.45, 0.9)],
            2 => &[(0.22, 0.12), (0.9, 0.54), (0.18, 0.88)],
            _ => &[(0.12, 0.5), (0.76, 0.14), (0.7, 0.9)],
        },
        PieceKind::Cube => &[(0.18, 0.28), (0.56, 0.14), (0.84, 0.34), (0.46, 0.48)],
        PieceKind::Octa => &[(0.5, 0.06), (0.12, 0.36), (0.5, 0.94), (0.88, 0.36)],
        PieceKind::Dodeca => &[
            (0.5, 0.04),
            (0.18, 0.2),
            (0.08, 0.52),
            (0.26, 0.84),
            (0.74, 0.84),
            (0.92, 0.52),
            (0.82, 0.2),
        ],
        PieceKind::Icosa => &[
            (0.5, 0.02),
            (0.12, 0.26),
            (0.04, 0.58),
            (0.28, 0.9),
            (0.72, 0.9),
            (0.96, 0.58),
            (0.88, 0.26),
        ],
    }
}

pub fn initial_board(level: &LevelSpec) -> Vec<Cell> {
    let mut board = vec![Cell::Matrix; BOARD_W * BOARD_H];

    for &(x, y) in level.walls {
        if let Some(index) = index(x, y) {
            board[index] = Cell::Wall;
        }
    }
    for &(x, y) in level.seeds {
        if let Some(index) = index(x, y) {
            board[index] = Cell::Channel(0);
        }
    }
    for &(x, y) in level.sources {
        if let Some(index) = index(x, y) {
            board[index] = Cell::Source;
        }
    }
    for &(x, y) in level.outlets {
        if let Some(index) = index(x, y) {
            board[index] = Cell::Outlet;
        }
    }
    for &(x, y) in level.pockets {
        if let Some(index) = index(x, y) {
            board[index] = Cell::Pocket;
        }
    }

    board
}

pub fn index(x: i32, y: i32) -> Option<usize> {
    if x < 0 || y < 0 || x >= BOARD_W as i32 || y >= BOARD_H as i32 {
        return None;
    }
    Some(y as usize * BOARD_W + x as usize)
}

const NO_COORDS: [(i32, i32); 0] = [];

const LEVEL_ONE_SOURCES: [(i32, i32); 2] = [(4, 15), (5, 15)];
const LEVEL_ONE_SEEDS: [(i32, i32); 2] = [(4, 14), (5, 14)];

const LEVEL_TWO_SOURCES: [(i32, i32); 2] = [(4, 15), (5, 15)];
const LEVEL_TWO_SEEDS: [(i32, i32); 10] = [
    (4, 14),
    (5, 14),
    (4, 13),
    (5, 13),
    (4, 12),
    (5, 12),
    (4, 11),
    (5, 11),
    (4, 10),
    (5, 10),
];
const LEVEL_TWO_OUTLETS: [(i32, i32); 2] = [(4, 4), (5, 4)];

const LEVEL_THREE_SOURCES: [(i32, i32); 2] = [(4, 15), (5, 15)];
const LEVEL_THREE_SEEDS: [(i32, i32); 8] = [
    (4, 14),
    (5, 14),
    (4, 13),
    (5, 13),
    (4, 12),
    (5, 12),
    (4, 11),
    (5, 11),
];
const LEVEL_THREE_POCKETS: [(i32, i32); 3] = [(2, 9), (7, 9), (5, 5)];

pub fn levels() -> Vec<LevelSpec> {
    vec![
        LevelSpec {
            name: "Phase 1 · Variation",
            subtitle: "Breed open space from diverse crystal seeds.",
            objective: "Reach 22 connected cells.",
            phase_badge: "Variation",
            cue_title: "See it",
            cue_text: "Only pore space touching the cyan inlet lights up and starts to shimmer.",
            concept_title: "Idea",
            concept_text: "Evolution starts with variation. Different shapes create different pore volumes.",
            objective_kind: ObjectiveKind::AccessibleVolume { target: 22 },
            turn_limit: 5,
            sources: &LEVEL_ONE_SOURCES,
            outlets: &NO_COORDS,
            pockets: &NO_COORDS,
            walls: &NO_COORDS,
            seeds: &LEVEL_ONE_SEEDS,
        },
        LevelSpec {
            name: "Phase 2 · Selection",
            subtitle: "Let narrow passages fail and wide throats survive.",
            objective: "Send the amber probe to the vent.",
            phase_badge: "Selection",
            cue_title: "See it",
            cue_text: "Teal gas finds any route. Amber only survives a repeated wide throat.",
            concept_title: "Idea",
            concept_text: "Selection pressure keeps only structures that pass the target guest.",
            objective_kind: ObjectiveKind::ProbeToVent { tier: 2 },
            turn_limit: 5,
            sources: &LEVEL_TWO_SOURCES,
            outlets: &LEVEL_TWO_OUTLETS,
            pockets: &NO_COORDS,
            walls: &NO_COORDS,
            seeds: &LEVEL_TWO_SEEDS,
        },
        LevelSpec {
            name: "Phase 3 · Inheritance",
            subtitle: "Carry strong traits forward and branch into traps.",
            objective: "Light any 2 gold pockets.",
            phase_badge: "Inheritance",
            cue_title: "See it",
            cue_text: "A pocket only flashes gold if the evolved network can truly reach it.",
            concept_title: "Idea",
            concept_text: "Useful traits persist when inherited shapes can reach more adsorption sites.",
            objective_kind: ObjectiveKind::AdsorbPockets { target: 2 },
            turn_limit: 6,
            sources: &LEVEL_THREE_SOURCES,
            outlets: &NO_COORDS,
            pockets: &LEVEL_THREE_POCKETS,
            walls: &NO_COORDS,
            seeds: &LEVEL_THREE_SEEDS,
        },
    ]
}
