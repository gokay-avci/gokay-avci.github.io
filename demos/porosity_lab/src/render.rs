use wasm_bindgen::JsValue;
use web_sys::CanvasRenderingContext2d;

use crate::levels::{
    piece_cells, poly_points, ActivePiece, Candidate, Cell, GeneratorKnobs, LevelSpec, ObjectiveKind,
    PieceKind, BOARD_H, BOARD_W,
};
use crate::sim::Simulation;

const BOARD_X: f64 = 40.0;
const BOARD_Y: f64 = 40.0;
const CELL: f64 = 26.0;

pub fn draw(
    ctx: &CanvasRenderingContext2d,
    board: &[Cell],
    active: Option<ActivePiece>,
    ghost: Option<ActivePiece>,
    sim: &Simulation,
    level: &LevelSpec,
    candidates: &[Option<Candidate>; 3],
    knobs: GeneratorKnobs,
    generation: usize,
    turns_used: usize,
    turn_limit: usize,
    elite: Option<Candidate>,
    ai_pick: Option<usize>,
    time_ms: f64,
) -> Result<(), JsValue> {
    paint_background(ctx, time_ms);
    draw_shell(ctx)?;
    draw_board(ctx, board, active, ghost, sim, level, time_ms)?;
    draw_generation_strip(ctx, candidates, elite, generation, ai_pick, time_ms)?;
    draw_knob_strip(ctx, knobs)?;
    draw_sim_strip(ctx, sim, level, turns_used, turn_limit, time_ms)?;
    Ok(())
}

fn paint_background(ctx: &CanvasRenderingContext2d, time_ms: f64) {
    let pulse = 0.05 + 0.03 * (time_ms / 1500.0).sin();
    ctx.set_fill_style_str("#071018");
    ctx.fill_rect(0.0, 0.0, 840.0, 520.0);
    ctx.set_fill_style_str(&format!("rgba(56,189,248,{:.3})", 0.08 + pulse));
    ctx.begin_path();
    let _ = ctx.arc(740.0, 110.0, 180.0, 0.0, std::f64::consts::TAU);
    ctx.fill();
    ctx.set_fill_style_str("rgba(251,191,36,0.08)");
    ctx.begin_path();
    let _ = ctx.arc(110.0, 430.0, 160.0, 0.0, std::f64::consts::TAU);
    ctx.fill();
}

fn draw_shell(ctx: &CanvasRenderingContext2d) -> Result<(), JsValue> {
    round_rect(ctx, 20.0, 18.0, 800.0, 484.0, 28.0)?;
    ctx.set_fill_style_str("rgba(10,17,26,0.86)");
    ctx.fill();
    round_rect(ctx, 28.0, 26.0, 312.0, 468.0, 22.0)?;
    ctx.set_fill_style_str("rgba(12,22,32,0.92)");
    ctx.fill();
    Ok(())
}

fn draw_board(
    ctx: &CanvasRenderingContext2d,
    board: &[Cell],
    active: Option<ActivePiece>,
    ghost: Option<ActivePiece>,
    sim: &Simulation,
    level: &LevelSpec,
    time_ms: f64,
) -> Result<(), JsValue> {
    let board_w = BOARD_W as f64 * CELL;
    let board_h = BOARD_H as f64 * CELL;

    round_rect(ctx, BOARD_X - 8.0, BOARD_Y - 8.0, board_w + 16.0, board_h + 16.0, 18.0)?;
    ctx.set_fill_style_str("rgba(255,255,255,0.05)");
    ctx.fill();

    for y in 0..BOARD_H as i32 {
        for x in 0..BOARD_W as i32 {
            let px = BOARD_X + x as f64 * CELL;
            let py = BOARD_Y + y as f64 * CELL;
            let cell = board[y as usize * BOARD_W + x as usize];

            ctx.set_fill_style_str("#0c1823");
            ctx.fill_rect(px + 1.0, py + 1.0, CELL - 2.0, CELL - 2.0);

            match cell {
                Cell::Matrix => {
                    ctx.set_fill_style_str("rgba(255,255,255,0.025)");
                    ctx.fill_rect(px + 9.0, py + 9.0, CELL - 18.0, CELL - 18.0);
                }
                Cell::Wall => {
                    ctx.set_fill_style_str("#1d2937");
                    ctx.fill_rect(px + 2.0, py + 2.0, CELL - 4.0, CELL - 4.0);
                }
                Cell::Channel(kind_index) => {
                    let kind = kind_from_index(kind_index);
                    let reachable = sim.reachable[y as usize * BOARD_W + x as usize];
                    draw_channel_cell(
                        ctx,
                        px,
                        py,
                        kind,
                        reachable,
                        sim.clearance[y as usize * BOARD_W + x as usize],
                        time_ms,
                    );
                }
                Cell::Source => {
                    draw_anchor(ctx, px, py, "#38bdf8", true, time_ms);
                }
                Cell::Outlet => {
                    draw_anchor(ctx, px, py, "#f59e0b", sim.probe_pass[1], time_ms);
                }
                Cell::Pocket => {
                    let lit = sim.reachable[y as usize * BOARD_W + x as usize];
                    draw_anchor(ctx, px, py, "#fbbf24", lit, time_ms);
                }
            }
        }
    }

    draw_flow(ctx, sim, level, time_ms);

    if let Some(ghost_piece) = ghost {
        draw_piece(ctx, ghost_piece, "rgba(255,255,255,0.18)", false);
    }
    if let Some(active_piece) = active {
        draw_piece(ctx, active_piece, palette(active_piece.kind), true);
    }

    ctx.set_font("700 12px Avenir Next");
    ctx.set_fill_style_str("rgba(255,255,255,0.72)");
    ctx.fill_text("EVOLVING LATTICE", BOARD_X - 2.0, 22.0)?;
    Ok(())
}

fn draw_channel_cell(
    ctx: &CanvasRenderingContext2d,
    px: f64,
    py: f64,
    kind: PieceKind,
    reachable: bool,
    clearance: u8,
    time_ms: f64,
) {
    ctx.set_fill_style_str(palette(kind));
    ctx.fill_rect(px + 2.0, py + 2.0, CELL - 4.0, CELL - 4.0);

    let glow = if reachable {
        0.18 + 0.06 * ((time_ms / 260.0) + px * 0.03 + py * 0.02).sin()
    } else {
        0.04
    };
    ctx.set_fill_style_str(&format!("rgba(255,255,255,{:.3})", glow));
    ctx.fill_rect(px + 5.0, py + 5.0, CELL - 10.0, CELL - 10.0);

    ctx.set_stroke_style_str("rgba(255,255,255,0.28)");
    ctx.set_line_width(1.2);
    let ring = 4.0 + clearance as f64 * 1.6;
    ctx.begin_path();
    let _ = ctx.arc(px + CELL / 2.0, py + CELL / 2.0, ring, 0.0, std::f64::consts::TAU);
    ctx.stroke();
}

fn draw_anchor(ctx: &CanvasRenderingContext2d, px: f64, py: f64, color: &str, lit: bool, time_ms: f64) {
    ctx.set_fill_style_str(if lit { color } else { "rgba(255,255,255,0.12)" });
    ctx.fill_rect(px + 2.0, py + 2.0, CELL - 4.0, CELL - 4.0);
    ctx.set_stroke_style_str("rgba(255,255,255,0.55)");
    ctx.stroke_rect(px + 5.0, py + 5.0, CELL - 10.0, CELL - 10.0);
    if lit {
        ctx.set_fill_style_str(&format!("rgba(255,255,255,{:.3})", 0.18 + 0.1 * (time_ms / 280.0).sin()));
        ctx.fill_rect(px + 9.0, py + 9.0, CELL - 18.0, CELL - 18.0);
    }
}

fn draw_piece(ctx: &CanvasRenderingContext2d, piece: ActivePiece, color: &str, solid: bool) {
    let mut min_px = f64::MAX;
    let mut min_py = f64::MAX;
    let mut max_px = f64::MIN;
    let mut max_py = f64::MIN;

    for &(dx, dy) in piece_cells(piece.kind, piece.rotation) {
        let x = piece.x + dx;
        let y = piece.y + dy;
        if y < 0 {
            continue;
        }
        let px = BOARD_X + x as f64 * CELL;
        let py = BOARD_Y + y as f64 * CELL;
        min_px = min_px.min(px);
        min_py = min_py.min(py);
        max_px = max_px.max(px + CELL);
        max_py = max_py.max(py + CELL);

        if solid {
            ctx.set_fill_style_str(color);
            ctx.fill_rect(px + 2.0, py + 2.0, CELL - 4.0, CELL - 4.0);
            ctx.set_fill_style_str("rgba(255,255,255,0.18)");
            ctx.fill_rect(px + 7.0, py + 7.0, CELL - 14.0, CELL - 14.0);
        } else {
            ctx.set_stroke_style_str(color);
            ctx.set_line_width(2.0);
            ctx.stroke_rect(px + 4.0, py + 4.0, CELL - 8.0, CELL - 8.0);
        }
    }

    if min_px.is_finite() {
        draw_polyhedron_projection(
            ctx,
            piece.kind,
            piece.rotation,
            min_px + 4.0,
            min_py + 4.0,
            (max_px - min_px - 8.0).max(18.0),
            (max_py - min_py - 8.0).max(18.0),
            if solid { "rgba(255,255,255,0.75)" } else { color },
        );
    }
}

fn draw_flow(ctx: &CanvasRenderingContext2d, sim: &Simulation, level: &LevelSpec, time_ms: f64) {
    if level.outlets.is_empty() {
        return;
    }
    let colors = ["#67e8f9", "#f59e0b", "#fb7185"];
    let sizes = [4.0, 6.0, 8.0];
    for tier in 0..3 {
        let path = &sim.probe_paths[tier];
        if path.is_empty() {
            continue;
        }
        let progress = if sim.probe_pass[tier] {
            ((time_ms / 1100.0) + tier as f64 * 0.22).fract()
        } else {
            0.45 + 0.18 * ((time_ms / 260.0) + tier as f64).sin()
        };
        let (x, y) = point_on_path(path, progress);
        ctx.set_fill_style_str(colors[tier]);
        ctx.begin_path();
        let _ = ctx.arc(x, y, sizes[tier], 0.0, std::f64::consts::TAU);
        ctx.fill();
    }
}

fn draw_generation_strip(
    ctx: &CanvasRenderingContext2d,
    candidates: &[Option<Candidate>; 3],
    elite: Option<Candidate>,
    generation: usize,
    ai_pick: Option<usize>,
    time_ms: f64,
) -> Result<(), JsValue> {
    ctx.set_font("700 12px Avenir Next");
    ctx.set_fill_style_str("rgba(255,255,255,0.7)");
    ctx.fill_text("AI BREEDER", 366.0, 50.0)?;
    ctx.set_fill_style_str("#7dd3fc");
    ctx.fill_text(&format!("GEN {}", generation), 720.0, 50.0)?;

    for (index, candidate) in candidates.iter().enumerate() {
        let x = 360.0 + index as f64 * 146.0;
        let y = 66.0;
        round_rect(ctx, x, y, 132.0, 134.0, 18.0)?;
        ctx.set_fill_style_str(if ai_pick == Some(index) {
            "rgba(24,49,68,0.96)"
        } else {
            "rgba(18,33,48,0.92)"
        });
        ctx.fill();

        if let Some(candidate) = candidate {
            let sweep = 0.12 + 0.06 * ((time_ms / 420.0) + index as f64).sin();
            ctx.set_fill_style_str(&format!("rgba(255,255,255,{:.3})", sweep));
            round_rect(ctx, x + 8.0, y + 8.0, 116.0, 44.0, 12.0)?;
            ctx.fill();

            draw_polyhedron_projection(
                ctx,
                candidate.kind,
                candidate.rotation,
                x + 20.0,
                y + 58.0,
                40.0,
                38.0,
                "rgba(255,255,255,0.86)",
            );
            draw_preview_cells(ctx, candidate.kind, candidate.rotation, x + 70.0, y + 62.0);

            ctx.set_font("700 14px Avenir Next");
            ctx.set_fill_style_str("#f8fafc");
            ctx.fill_text(candidate.kind.label(), x + 14.0, y + 26.0)?;
            ctx.set_font("700 11px Avenir Next");
            ctx.set_fill_style_str("rgba(255,255,255,0.68)");
            ctx.fill_text(candidate.origin, x + 14.0, y + 42.0)?;
            ctx.fill_text(candidate.badge, x + 14.0, y + 112.0)?;
            ctx.fill_text(&format!("fit {}", candidate.fitness), x + 14.0, y + 128.0)?;
            ctx.fill_text(&format!("mut {}", candidate.mutation), x + 74.0, y + 128.0)?;
            if ai_pick == Some(index) {
                ctx.set_fill_style_str("#7dd3fc");
                ctx.fill_text("AI", x + 102.0, y + 26.0)?;
            }
        }
    }

    if let Some(elite) = elite {
        ctx.set_fill_style_str("rgba(255,255,255,0.64)");
        ctx.fill_text("elite parent", 364.0, 214.0)?;
        draw_polyhedron_projection(ctx, elite.kind, elite.rotation, 458.0, 204.0, 28.0, 28.0, "#f59e0b");
    }

    Ok(())
}

fn draw_knob_strip(ctx: &CanvasRenderingContext2d, knobs: GeneratorKnobs) -> Result<(), JsValue> {
    round_rect(ctx, 360.0, 232.0, 436.0, 88.0, 18.0)?;
    ctx.set_fill_style_str("rgba(18,33,48,0.9)");
    ctx.fill();
    ctx.set_font("700 12px Avenir Next");
    ctx.set_fill_style_str("rgba(255,255,255,0.7)");
    ctx.fill_text("SELECTION PRESSURE", 374.0, 254.0)?;
    draw_knob_meter(ctx, "Cavern", knobs.cavern, 374.0, 272.0, "#38bdf8")?;
    draw_knob_meter(ctx, "Throat", knobs.throat, 518.0, 272.0, "#f59e0b")?;
    draw_knob_meter(ctx, "Branch", knobs.branch, 662.0, 272.0, "#f472b6")?;
    Ok(())
}

fn draw_knob_meter(
    ctx: &CanvasRenderingContext2d,
    label: &str,
    value: f64,
    x: f64,
    y: f64,
    color: &str,
) -> Result<(), JsValue> {
    ctx.set_font("700 11px Avenir Next");
    ctx.set_fill_style_str("rgba(255,255,255,0.68)");
    ctx.fill_text(label, x, y)?;
    ctx.set_fill_style_str("rgba(255,255,255,0.10)");
    ctx.fill_rect(x, y + 10.0, 108.0, 12.0);
    ctx.set_fill_style_str(color);
    ctx.fill_rect(x, y + 10.0, value.clamp(0.0, 1.0) * 108.0, 12.0);
    Ok(())
}

fn draw_sim_strip(
    ctx: &CanvasRenderingContext2d,
    sim: &Simulation,
    level: &LevelSpec,
    turns_used: usize,
    turn_limit: usize,
    time_ms: f64,
) -> Result<(), JsValue> {
    round_rect(ctx, 360.0, 336.0, 436.0, 148.0, 18.0)?;
    ctx.set_fill_style_str("rgba(18,33,48,0.9)");
    ctx.fill();

    ctx.set_font("700 12px Avenir Next");
    ctx.set_fill_style_str("rgba(255,255,255,0.7)");
    ctx.fill_text("LIVE SCIENCE", 374.0, 360.0)?;

    ctx.set_fill_style_str("rgba(255,255,255,0.12)");
    ctx.fill_rect(374.0, 372.0, 168.0, 14.0);
    ctx.set_fill_style_str("#38bdf8");
    ctx.fill_rect(374.0, 372.0, 168.0 * sim.accessible_ratio.clamp(0.0, 1.0), 14.0);

    ctx.set_font("700 28px Avenir Next");
    ctx.set_fill_style_str("#f8fafc");
    ctx.fill_text(&sim.accessible_count.to_string(), 374.0, 420.0)?;
    ctx.set_font("700 12px Avenir Next");
    ctx.set_fill_style_str("rgba(255,255,255,0.64)");
    ctx.fill_text("connected cells", 420.0, 420.0)?;

    for tier in 0..3 {
        let lit = sim.probe_pass[tier] || sim.best_probe_tier > tier;
        ctx.set_fill_style_str(if lit {
            ["#67e8f9", "#f59e0b", "#fb7185"][tier]
        } else {
            "rgba(255,255,255,0.14)"
        });
        ctx.begin_path();
        let _ = ctx.arc(388.0 + tier as f64 * 42.0, 450.0, 8.0 + tier as f64 * 2.0, 0.0, std::f64::consts::TAU);
        ctx.fill();
    }

    for index in 0..level.pockets.len().max(3) {
        let lit = sim.adsorbed_sites > index;
        let glow = if lit {
            format!("rgba(251,191,36,{:.3})", 0.65 + 0.08 * (time_ms / 300.0).sin())
        } else {
            "rgba(255,255,255,0.12)".to_string()
        };
        ctx.set_fill_style_str(&glow);
        ctx.fill_rect(516.0 + index as f64 * 24.0, 438.0, 16.0, 16.0);
    }

    ctx.set_fill_style_str("rgba(255,255,255,0.7)");
    ctx.fill_text(&format!("turns {}/{}", turns_used, turn_limit), 648.0, 420.0)?;
    ctx.fill_text(
        match level.objective_kind {
            ObjectiveKind::AccessibleVolume { .. } => "maximize accessible void",
            ObjectiveKind::ProbeToVent { .. } => "pass the amber guest",
            ObjectiveKind::AdsorbPockets { .. } => "inherit paths into traps",
        },
        560.0,
        450.0,
    )?;
    Ok(())
}

fn draw_preview_cells(ctx: &CanvasRenderingContext2d, kind: PieceKind, rotation: usize, x: f64, y: f64) {
    for &(dx, dy) in piece_cells(kind, rotation) {
        ctx.set_fill_style_str(palette(kind));
        ctx.fill_rect(x + dx as f64 * 10.0, y + dy as f64 * 10.0, 8.0, 8.0);
    }
}

fn draw_polyhedron_projection(
    ctx: &CanvasRenderingContext2d,
    kind: PieceKind,
    rotation: usize,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    stroke: &str,
) {
    let points = poly_points(kind, rotation);
    if points.is_empty() {
        return;
    }

    ctx.set_stroke_style_str(stroke);
    ctx.set_line_width(1.6);
    ctx.begin_path();
    let (sx, sy) = points[0];
    ctx.move_to(x + sx * w, y + sy * h);
    for &(px, py) in points.iter().skip(1) {
        ctx.line_to(x + px * w, y + py * h);
    }
    ctx.close_path();
    ctx.stroke();

    let mid_y = y + h * 0.48;
    ctx.set_stroke_style_str("rgba(255,255,255,0.22)");
    ctx.begin_path();
    ctx.move_to(x + w * 0.18, mid_y);
    ctx.line_to(x + w * 0.82, mid_y);
    ctx.stroke();
}

fn point_on_path(path: &[(i32, i32)], progress: f64) -> (f64, f64) {
    if path.len() <= 1 {
        let (x, y) = path.first().copied().unwrap_or((0, 0));
        return cell_center(x, y);
    }

    let scaled = progress.clamp(0.0, 0.999) * (path.len() - 1) as f64;
    let index = scaled.floor() as usize;
    let frac = scaled - index as f64;
    let (ax, ay) = cell_center(path[index].0, path[index].1);
    let (bx, by) = cell_center(path[index + 1].0, path[index + 1].1);
    (ax + (bx - ax) * frac, ay + (by - ay) * frac)
}

fn cell_center(x: i32, y: i32) -> (f64, f64) {
    (
        BOARD_X + x as f64 * CELL + CELL / 2.0,
        BOARD_Y + y as f64 * CELL + CELL / 2.0,
    )
}

fn kind_from_index(index: u8) -> PieceKind {
    match index {
        0 => PieceKind::Tetra,
        1 => PieceKind::Cube,
        2 => PieceKind::Octa,
        3 => PieceKind::Dodeca,
        _ => PieceKind::Icosa,
    }
}

fn palette(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::Tetra => "#6ee7b7",
        PieceKind::Cube => "#f59e0b",
        PieceKind::Octa => "#60a5fa",
        PieceKind::Dodeca => "#f472b6",
        PieceKind::Icosa => "#a78bfa",
    }
}

fn round_rect(
    ctx: &CanvasRenderingContext2d,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    r: f64,
) -> Result<(), JsValue> {
    let r = r.min(w / 2.0).min(h / 2.0);
    ctx.begin_path();
    ctx.move_to(x + r, y);
    ctx.line_to(x + w - r, y);
    ctx.quadratic_curve_to(x + w, y, x + w, y + r);
    ctx.line_to(x + w, y + h - r);
    ctx.quadratic_curve_to(x + w, y + h, x + w - r, y + h);
    ctx.line_to(x + r, y + h);
    ctx.quadratic_curve_to(x, y + h, x, y + h - r);
    ctx.line_to(x, y + r);
    ctx.quadratic_curve_to(x, y, x + r, y);
    Ok(())
}
