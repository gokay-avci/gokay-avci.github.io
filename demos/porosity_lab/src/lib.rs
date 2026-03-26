mod levels;
mod render;
mod sim;
mod utils;

use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::Rc;

use levels::{
    index, initial_board, levels, piece_cells, ActivePiece, Candidate, Cell, GeneratorKnobs,
    LevelSpec, ObjectiveKind, PieceKind, BOARD_H, BOARD_W,
};
use render::draw;
use sim::{analyze, Simulation};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    CanvasRenderingContext2d, Document, HtmlCanvasElement, HtmlInputElement, KeyboardEvent,
    MouseEvent,
};

const CANVAS_WIDTH: u32 = 840;
const CANVAS_HEIGHT: u32 = 520;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    AwaitBreed,
    AwaitPick,
    Falling,
    Won,
    Lost,
    CampaignWon,
}

#[derive(Clone, Copy)]
enum Action {
    Left,
    Right,
    Rotate,
    Drop,
    Breed,
    Restart,
    Advance,
    SelectCandidate(usize),
}

struct Game {
    levels: Vec<LevelSpec>,
    current_level: usize,
    board: Vec<Cell>,
    sim: Simulation,
    active: Option<ActivePiece>,
    candidates: [Option<Candidate>; 3],
    selected_candidate: Option<Candidate>,
    elite: Option<Candidate>,
    knobs: GeneratorKnobs,
    generation: usize,
    turns_used: usize,
    phase: Phase,
    time_ms: f64,
    drop_accumulator: f64,
    ai_pick: Option<usize>,
}

impl Game {
    fn new() -> Self {
        let levels = levels();
        let board = initial_board(&levels[0]);
        let sim = analyze(&board, &levels[0]);
        let knobs = default_knobs(&levels[0]);

        let mut game = Self {
            levels,
            current_level: 0,
            board,
            sim,
            active: None,
            candidates: [None, None, None],
            selected_candidate: None,
            elite: None,
            knobs,
            generation: 0,
            turns_used: 0,
            phase: Phase::AwaitBreed,
            time_ms: 0.0,
            drop_accumulator: 0.0,
            ai_pick: None,
        };
        game.breed_candidates();
        game
    }

    fn level(&self) -> &LevelSpec {
        &self.levels[self.current_level]
    }

    fn load_level(&mut self, index: usize) {
        self.current_level = index;
        self.board = initial_board(&self.levels[index]);
        self.sim = analyze(&self.board, &self.levels[index]);
        self.active = None;
        self.candidates = [None, None, None];
        self.selected_candidate = None;
        self.elite = None;
        self.knobs = default_knobs(&self.levels[index]);
        self.generation = 0;
        self.turns_used = 0;
        self.phase = Phase::AwaitBreed;
        self.drop_accumulator = 0.0;
        self.ai_pick = None;
        self.breed_candidates();
    }

    fn tick(&mut self, dt_ms: f64) {
        self.time_ms += dt_ms;
        if self.phase != Phase::Falling {
            return;
        }

        self.drop_accumulator += dt_ms;
        if self.drop_accumulator > 460.0 {
            self.drop_accumulator = 0.0;
            if !self.try_move(0, 1) {
                self.lock_piece();
            }
        }
    }

    fn breed_candidates(&mut self) {
        if matches!(self.phase, Phase::Won | Phase::Lost | Phase::CampaignWon) || self.active.is_some()
        {
            return;
        }

        self.generation += 1;
        let objective_axis = self.objective_axis();
        let elite_kind = self.elite.map(|candidate| candidate.kind);
        let archetype = phase_archetype(self.level());

        let origins = ["elite", "crossover", "mutant"];
        let mutation_bases = [18_u8, 34_u8, 58_u8];
        let mut candidates = [None, None, None];

        for slot in 0..3 {
            let kind = choose_kind(elite_kind, archetype, slot, self.knobs);
            let rotation = choose_rotation(kind, self.knobs, slot);
            let traits = features(kind, rotation);
            let mutation = (mutation_bases[slot] as f64 + js_sys::Math::random() * 16.0) as u8;
            let fitness = (60.0
                + trait_score(objective_axis, self.knobs, traits) * 34.0
                + novelty_bonus(elite_kind, kind) * 12.0
                - (mutation as f64 * 0.08))
                .clamp(0.0, 99.0) as u8;

            candidates[slot] = Some(Candidate {
                kind,
                rotation,
                badge: feature_badge(traits),
                origin: origins[slot],
                fitness,
                mutation,
            });
        }

        let mut ranked = candidates.into_iter().flatten().collect::<Vec<_>>();
        ranked.sort_by(|a, b| {
            b.fitness
                .cmp(&a.fitness)
                .then_with(|| b.mutation.cmp(&a.mutation))
                .then(Ordering::Equal)
        });

        self.candidates = [None, None, None];
        for (index, candidate) in ranked.into_iter().take(3).enumerate() {
            self.candidates[index] = Some(candidate);
        }
        self.ai_pick = Some(0);
        self.phase = Phase::AwaitPick;
    }

    fn spawn_candidate(&mut self, index: usize) {
        if self.phase != Phase::AwaitPick {
            return;
        }

        let Some(candidate) = self.candidates[index] else {
            return;
        };

        let piece = ActivePiece {
            kind: candidate.kind,
            rotation: candidate.rotation,
            x: centered_spawn_x(candidate.kind, candidate.rotation),
            y: -2,
        };

        if self.collides(piece) {
            self.phase = Phase::Lost;
            return;
        }

        self.selected_candidate = Some(candidate);
        self.active = Some(piece);
        self.candidates = [None, None, None];
        self.ai_pick = None;
        self.phase = Phase::Falling;
        self.drop_accumulator = 0.0;
    }

    fn try_move(&mut self, dx: i32, dy: i32) -> bool {
        let Some(active) = self.active else {
            return false;
        };
        let shifted = ActivePiece {
            x: active.x + dx,
            y: active.y + dy,
            ..active
        };
        if self.collides(shifted) {
            return false;
        }
        self.active = Some(shifted);
        true
    }

    fn rotate(&mut self) {
        let Some(active) = self.active else {
            return;
        };
        let rotated = ActivePiece {
            rotation: active.rotation + 1,
            ..active
        };

        for kick in [0, -1, 1, -2, 2] {
            let tested = ActivePiece {
                x: rotated.x + kick,
                ..rotated
            };
            if !self.collides(tested) {
                self.active = Some(tested);
                return;
            }
        }
    }

    fn hard_drop(&mut self) {
        if self.phase != Phase::Falling {
            return;
        }
        while self.try_move(0, 1) {}
        self.lock_piece();
    }

    fn lock_piece(&mut self) {
        let Some(active) = self.active.take() else {
            return;
        };

        for &(dx, dy) in piece_cells(active.kind, active.rotation) {
            let x = active.x + dx;
            let y = active.y + dy;
            if let Some(cell_index) = index(x, y) {
                self.board[cell_index] = Cell::Channel(active.kind.palette_index());
            }
        }

        self.turns_used += 1;
        self.elite = self.selected_candidate.take();
        self.sim = analyze(&self.board, self.level());

        if self.objective_met() {
            self.phase = if self.current_level + 1 == self.levels.len() {
                Phase::CampaignWon
            } else {
                Phase::Won
            };
        } else if self.turns_used >= self.level().turn_limit {
            self.phase = Phase::Lost;
        } else {
            self.phase = Phase::AwaitBreed;
        }
    }

    fn collides(&self, piece: ActivePiece) -> bool {
        for &(dx, dy) in piece_cells(piece.kind, piece.rotation) {
            let x = piece.x + dx;
            let y = piece.y + dy;

            if x < 0 || x >= BOARD_W as i32 || y >= BOARD_H as i32 {
                return true;
            }

            if y < 0 {
                continue;
            }

            if let Some(cell_index) = index(x, y) {
                if self.board[cell_index].blocks_piece() {
                    return true;
                }
            }
        }
        false
    }

    fn ghost_piece(&self) -> Option<ActivePiece> {
        let mut ghost = self.active?;
        while !self.collides(ActivePiece { y: ghost.y + 1, ..ghost }) {
            ghost.y += 1;
        }
        Some(ghost)
    }

    fn objective_met(&self) -> bool {
        match self.level().objective_kind {
            ObjectiveKind::AccessibleVolume { target } => self.sim.accessible_count >= target,
            ObjectiveKind::ProbeToVent { tier } => self.sim.probe_pass[tier - 1],
            ObjectiveKind::AdsorbPockets { target } => self.sim.adsorbed_sites >= target,
        }
    }

    fn objective_axis(&self) -> usize {
        match self.level().objective_kind {
            ObjectiveKind::AccessibleVolume { .. } => 0,
            ObjectiveKind::ProbeToVent { .. } => 1,
            ObjectiveKind::AdsorbPockets { .. } => 2,
        }
    }

    fn objective_ratio(&self) -> f64 {
        match self.level().objective_kind {
            ObjectiveKind::AccessibleVolume { target } => self.sim.accessible_count as f64 / target as f64,
            ObjectiveKind::ProbeToVent { tier } => self.sim.best_probe_tier as f64 / tier as f64,
            ObjectiveKind::AdsorbPockets { target } => self.sim.adsorbed_sites as f64 / target as f64,
        }
        .clamp(0.0, 1.0)
    }

    fn phase_badge(&self) -> &'static str {
        self.level().phase_badge
    }

    fn probe_label(&self) -> &'static str {
        match self.sim.best_probe_tier {
            0 => "None",
            1 => "Small",
            2 => "Amber",
            _ => "Large",
        }
    }

    fn status_badge(&self) -> &'static str {
        match self.phase {
            Phase::AwaitBreed => "Breed",
            Phase::AwaitPick => "Select",
            Phase::Falling => "Place",
            Phase::Won | Phase::CampaignWon => "Evolved",
            Phase::Lost => "Extinct",
        }
    }

    fn status_state(&self) -> &'static str {
        match self.phase {
            Phase::Falling | Phase::Won | Phase::CampaignWon => "open",
            Phase::Lost => "blocked",
            _ => "bottlenecked",
        }
    }

    fn status_detail(&self) -> &'static str {
        match self.phase {
            Phase::AwaitBreed => "Turn the three biases and grow a fresh generation.",
            Phase::AwaitPick => "Pick a survivor. The AI highlight is only a suggestion.",
            Phase::Falling => "Steer the seed through space and snap it into the lattice.",
            Phase::Won => "This phase is solved. Carry the lesson forward.",
            Phase::CampaignWon => "Variation, selection, and inheritance are all visible on one board.",
            Phase::Lost => "That lineage stalled. Try a new pressure profile.",
        }
    }

    fn selection_line(&self) -> String {
        match self.phase {
            Phase::AwaitBreed => "Bias the foundry toward cavern, throat, or branch.".to_string(),
            Phase::AwaitPick => format!("Generation {} is ready. Choose one genome.", self.generation),
            Phase::Falling => "Arrow keys move, up rotates, space drops.".to_string(),
            Phase::Won => "Advance to the next pressure test.".to_string(),
            Phase::CampaignWon => "You built an evolving porous material.".to_string(),
            Phase::Lost => "Reset and try a different evolutionary path.".to_string(),
        }
    }

    fn scientific_flag(&self) -> (&'static str, &'static str, bool) {
        match self.level().objective_kind {
            ObjectiveKind::AccessibleVolume { target } => {
                if target.saturating_sub(self.sim.accessible_count) <= 4 {
                    ("Diverse pool", "You are one strong mutation away from a big volume jump.", true)
                } else {
                    ("Low diversity", "Current shapes are not opening enough shared void.", false)
                }
            }
            ObjectiveKind::ProbeToVent { tier } => {
                if self.sim.best_probe_tier + 1 >= tier {
                    ("Selection edge", "The route is nearly fit enough for the amber guest.", true)
                } else {
                    ("Weak throat", "The lineage still collapses at narrow passages.", false)
                }
            }
            ObjectiveKind::AdsorbPockets { target } => {
                if target.saturating_sub(self.sim.adsorbed_sites) <= 1 {
                    ("Inherited win", "Branching traits are almost reaching a second pocket.", true)
                } else {
                    ("Poor inheritance", "Useful pathways are not spreading into the side traps.", false)
                }
            }
        }
    }

    fn star_count(&self) -> usize {
        if !matches!(self.phase, Phase::Won | Phase::CampaignWon) {
            return 0;
        }
        let remaining = self.level().turn_limit.saturating_sub(self.turns_used);
        if remaining >= 2 {
            3
        } else if remaining == 1 {
            2
        } else {
            1
        }
    }
}

struct App {
    document: Document,
    ctx: CanvasRenderingContext2d,
    game: Game,
}

impl App {
    fn new(document: Document, _canvas: HtmlCanvasElement, ctx: CanvasRenderingContext2d) -> Result<Self, JsValue> {
        let mut app = Self {
            document,
            ctx,
            game: Game::new(),
        };
        app.sync_knobs()?;
        app.refresh()?;
        Ok(app)
    }

    fn tick(&mut self, dt_ms: f64) -> Result<(), JsValue> {
        self.game.tick(dt_ms);
        self.redraw()
    }

    fn redraw(&mut self) -> Result<(), JsValue> {
        draw(
            &self.ctx,
            &self.game.board,
            self.game.active,
            self.game.ghost_piece(),
            &self.game.sim,
            self.game.level(),
            &self.game.candidates,
            self.game.knobs,
            self.game.generation,
            self.game.turns_used,
            self.game.level().turn_limit,
            self.game.elite,
            self.game.ai_pick,
            self.game.time_ms,
        )
    }

    fn refresh(&mut self) -> Result<(), JsValue> {
        let level = self.game.level();
        let stars_text = if self.game.star_count() == 0 {
            "☆☆☆".to_string()
        } else {
            format!(
                "{}{}",
                "★".repeat(self.game.star_count()),
                "☆".repeat(3 - self.game.star_count())
            )
        };
        let (flag_title, flag_text, interesting) = self.game.scientific_flag();

        utils::set_text(
            &self.document,
            "puzzle-index",
            &format!("{}/3", self.game.current_level + 1),
        );
        utils::set_text(
            &self.document,
            "par-text",
            &format!("Gen {} · {}/{}", self.game.generation, self.game.turns_used, level.turn_limit),
        );
        utils::set_text(&self.document, "puzzle-title", level.name);
        utils::set_text(&self.document, "puzzle-subtitle", level.subtitle);
        utils::set_text(&self.document, "probe-button", self.game.phase_badge());
        utils::set_text(&self.document, "objective-text", level.objective);
        utils::set_text(&self.document, "stars-text", &stars_text);
        utils::set_text(&self.document, "status-badge", self.game.status_badge());
        utils::set_text(&self.document, "status-detail", self.game.status_detail());
        utils::set_text(
            &self.document,
            "score-total",
            &format!("{:.0}", self.game.objective_ratio() * 100.0),
        );
        utils::set_text(&self.document, "score-bottleneck", self.game.probe_label());
        utils::set_text(
            &self.document,
            "score-cavity",
            &format!("{}/{}", self.game.sim.adsorbed_sites, level.pockets.len()),
        );
        utils::set_text(
            &self.document,
            "score-cost",
            &format!("{}/{}", self.game.turns_used, level.turn_limit),
        );
        utils::set_text(&self.document, "hint-title", level.cue_title);
        utils::set_text(&self.document, "hint-text", level.cue_text);
        utils::set_text(&self.document, "candidate-title", flag_title);
        utils::set_text(&self.document, "candidate-text", flag_text);
        utils::set_text(&self.document, "selection-text", &self.game.selection_line());
        utils::set_text(&self.document, "concept-title", level.concept_title);
        utils::set_text(&self.document, "concept-text", level.concept_text);
        utils::set_text(
            &self.document,
            "knob-volume-value",
            &format!("{:.0}", self.game.knobs.cavern * 100.0),
        );
        utils::set_text(
            &self.document,
            "knob-throat-value",
            &format!("{:.0}", self.game.knobs.throat * 100.0),
        );
        utils::set_text(
            &self.document,
            "knob-branch-value",
            &format!("{:.0}", self.game.knobs.branch * 100.0),
        );
        utils::set_text(
            &self.document,
            "threshold-label",
            match level.objective_kind {
                ObjectiveKind::AccessibleVolume { .. } => "Open void",
                ObjectiveKind::ProbeToVent { .. } => "Guest fit",
                ObjectiveKind::AdsorbPockets { .. } => "Trap reach",
            },
        );

        utils::set_data_state(&self.document, "status-badge", self.game.status_state());
        utils::set_data_state(
            &self.document,
            "candidate-card",
            if interesting { "interesting" } else { "routine" },
        );
        utils::set_style(
            &self.document,
            "score-fill",
            "width",
            &format!("{:.0}%", self.game.objective_ratio() * 100.0),
        );
        utils::set_style(
            &self.document,
            "threshold-fill",
            "width",
            &format!("{:.0}%", secondary_fill(&self.game) * 100.0),
        );

        for index in 0..3 {
            let button_id = format!("candidate-{}", index + 1);
            if let Some(candidate) = self.game.candidates[index] {
                utils::set_text(
                    &self.document,
                    &button_id,
                    &format!(
                        "{} · {} · fit {}",
                        candidate.kind.label(),
                        candidate.origin,
                        candidate.fitness
                    ),
                );
            } else {
                utils::set_text(&self.document, &button_id, &format!("Genome {}", index + 1));
            }
        }

        utils::set_text(
            &self.document,
            "action-button",
            match self.game.phase {
                Phase::Won => "Next phase",
                Phase::CampaignWon => "Restart run",
                Phase::Lost => "Retry phase",
                _ => "",
            },
        );
        utils::set_style(
            &self.document,
            "action-button",
            "display",
            if matches!(self.game.phase, Phase::Won | Phase::Lost | Phase::CampaignWon) {
                "inline-flex"
            } else {
                "none"
            },
        );

        self.redraw()
    }

    fn sync_knobs(&self) -> Result<(), JsValue> {
        set_range_value(&self.document, "knob-volume", self.game.knobs.cavern)?;
        set_range_value(&self.document, "knob-throat", self.game.knobs.throat)?;
        set_range_value(&self.document, "knob-branch", self.game.knobs.branch)?;
        Ok(())
    }

    fn read_knobs(&mut self) -> Result<(), JsValue> {
        self.game.knobs = GeneratorKnobs {
            cavern: read_range_value(&self.document, "knob-volume")?,
            throat: read_range_value(&self.document, "knob-throat")?,
            branch: read_range_value(&self.document, "knob-branch")?,
        };
        Ok(())
    }

    fn handle_action(&mut self, action: Action) -> Result<(), JsValue> {
        match action {
            Action::Left if self.game.phase == Phase::Falling => {
                self.game.try_move(-1, 0);
            }
            Action::Right if self.game.phase == Phase::Falling => {
                self.game.try_move(1, 0);
            }
            Action::Rotate if self.game.phase == Phase::Falling => self.game.rotate(),
            Action::Drop if self.game.phase == Phase::Falling => self.game.hard_drop(),
            Action::Breed if matches!(self.game.phase, Phase::AwaitBreed | Phase::AwaitPick) => {
                self.read_knobs()?;
                self.game.breed_candidates();
            }
            Action::Restart => {
                let index = self.game.current_level;
                self.game.load_level(index);
                self.sync_knobs()?;
            }
            Action::Advance => match self.game.phase {
                Phase::Won => {
                    self.game.load_level(self.game.current_level + 1);
                    self.sync_knobs()?;
                }
                Phase::CampaignWon => {
                    self.game.load_level(0);
                    self.sync_knobs()?;
                }
                Phase::Lost => {
                    self.game.load_level(self.game.current_level);
                    self.sync_knobs()?;
                }
                _ => {}
            },
            Action::SelectCandidate(index) => self.game.spawn_candidate(index),
            _ => {}
        }
        self.refresh()
    }
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let document = utils::document()?;
    let canvas = document
        .get_element_by_id("structure-canvas")
        .ok_or_else(|| JsValue::from_str("canvas missing"))?
        .dyn_into::<HtmlCanvasElement>()?;
    canvas.set_width(CANVAS_WIDTH);
    canvas.set_height(CANVAS_HEIGHT);

    let ctx = canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("context missing"))?
        .dyn_into::<CanvasRenderingContext2d>()?;

    let app = Rc::new(RefCell::new(App::new(document.clone(), canvas, ctx)?));
    bind_buttons(&document, &app)?;
    bind_keyboard(&document, &app)?;
    start_loop(app)?;
    utils::set_wasm_ready()?;
    Ok(())
}

fn bind_buttons(document: &Document, app: &Rc<RefCell<App>>) -> Result<(), JsValue> {
    bind_click(document, app, "btn-left", Action::Left)?;
    bind_click(document, app, "btn-right", Action::Right)?;
    bind_click(document, app, "btn-rotate", Action::Rotate)?;
    bind_click(document, app, "btn-drop", Action::Drop)?;
    bind_click(document, app, "btn-generate", Action::Breed)?;
    bind_click(document, app, "btn-reset", Action::Restart)?;
    bind_click(document, app, "action-button", Action::Advance)?;
    bind_click(document, app, "candidate-1", Action::SelectCandidate(0))?;
    bind_click(document, app, "candidate-2", Action::SelectCandidate(1))?;
    bind_click(document, app, "candidate-3", Action::SelectCandidate(2))?;
    Ok(())
}

fn bind_click(
    document: &Document,
    app: &Rc<RefCell<App>>,
    id: &str,
    action: Action,
) -> Result<(), JsValue> {
    if let Some(element) = document.get_element_by_id(id) {
        let app = Rc::clone(app);
        let closure = Closure::wrap(Box::new(move |_event: MouseEvent| {
            let _ = app.borrow_mut().handle_action(action);
        }) as Box<dyn FnMut(_)>);
        element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }
    Ok(())
}

fn bind_keyboard(document: &Document, app: &Rc<RefCell<App>>) -> Result<(), JsValue> {
    let app = Rc::clone(app);
    let closure = Closure::wrap(Box::new(move |event: KeyboardEvent| {
        let action = match event.key().as_str() {
            "ArrowLeft" | "a" | "A" => Some(Action::Left),
            "ArrowRight" | "d" | "D" => Some(Action::Right),
            "ArrowUp" | "w" | "W" => Some(Action::Rotate),
            " " | "ArrowDown" | "s" | "S" => Some(Action::Drop),
            "g" | "G" => Some(Action::Breed),
            "r" | "R" => Some(Action::Restart),
            "Enter" => Some(Action::Advance),
            "1" => Some(Action::SelectCandidate(0)),
            "2" => Some(Action::SelectCandidate(1)),
            "3" => Some(Action::SelectCandidate(2)),
            _ => None,
        };
        if let Some(action) = action {
            event.prevent_default();
            let _ = app.borrow_mut().handle_action(action);
        }
    }) as Box<dyn FnMut(_)>);

    document.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
    closure.forget();
    Ok(())
}

fn start_loop(app: Rc<RefCell<App>>) -> Result<(), JsValue> {
    let raf_cb: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let raf_cb_clone = Rc::clone(&raf_cb);
    let last_time = Rc::new(RefCell::new(None::<f64>));
    let last_time_clone = Rc::clone(&last_time);

    *raf_cb_clone.borrow_mut() = Some(Closure::wrap(Box::new(move |time_ms: f64| {
        let dt = {
            let mut last = last_time_clone.borrow_mut();
            let delta = last.map(|previous| (time_ms - previous).clamp(0.0, 40.0)).unwrap_or(16.0);
            *last = Some(time_ms);
            delta
        };
        let _ = app.borrow_mut().tick(dt);
        if let Some(window) = web_sys::window() {
            let _ = window.request_animation_frame(
                raf_cb
                    .borrow()
                    .as_ref()
                    .expect("raf present")
                    .as_ref()
                    .unchecked_ref(),
            );
        }
    }) as Box<dyn FnMut(f64)>));

    if let Some(window) = web_sys::window() {
        window.request_animation_frame(
            raf_cb_clone
                .borrow()
                .as_ref()
                .expect("raf present")
                .as_ref()
                .unchecked_ref(),
        )?;
    }
    Ok(())
}

fn secondary_fill(game: &Game) -> f64 {
    match game.level().objective_kind {
        ObjectiveKind::AccessibleVolume { target } => game.sim.accessible_count as f64 / target as f64,
        ObjectiveKind::ProbeToVent { tier } => game.sim.best_probe_tier as f64 / tier as f64,
        ObjectiveKind::AdsorbPockets { target } => game.sim.adsorbed_sites as f64 / target as f64,
    }
    .clamp(0.0, 1.0)
}

fn phase_archetype(level: &LevelSpec) -> PieceKind {
    match level.objective_kind {
        ObjectiveKind::AccessibleVolume { .. } => PieceKind::Octa,
        ObjectiveKind::ProbeToVent { .. } => PieceKind::Cube,
        ObjectiveKind::AdsorbPockets { .. } => PieceKind::Dodeca,
    }
}

fn choose_kind(
    elite: Option<PieceKind>,
    archetype: PieceKind,
    slot: usize,
    knobs: GeneratorKnobs,
) -> PieceKind {
    let roll = js_sys::Math::random();
    match slot {
        0 => elite.unwrap_or(archetype),
        1 => {
            if roll < 0.45 {
                archetype
            } else {
                weighted_kind(knobs)
            }
        }
        _ => weighted_kind(knobs),
    }
}

fn weighted_kind(knobs: GeneratorKnobs) -> PieceKind {
    let weights = [
        (PieceKind::Tetra, knobs.branch * 0.6 + knobs.throat * 0.2),
        (PieceKind::Cube, knobs.throat * 0.7 + knobs.cavern * 0.2),
        (PieceKind::Octa, knobs.cavern * 0.7 + knobs.branch * 0.2),
        (PieceKind::Dodeca, knobs.branch * 0.6 + knobs.cavern * 0.3),
        (PieceKind::Icosa, knobs.cavern * 0.4 + knobs.throat * 0.3 + knobs.branch * 0.3),
    ];

    let total: f64 = weights.iter().map(|(_, weight)| *weight).sum();
    let mut cursor = js_sys::Math::random() * total;

    for (kind, weight) in weights {
        cursor -= weight;
        if cursor <= 0.0 {
            return kind;
        }
    }

    PieceKind::Icosa
}

fn choose_rotation(kind: PieceKind, knobs: GeneratorKnobs, slot: usize) -> usize {
    let base = match kind {
        PieceKind::Cube | PieceKind::Octa => 0,
        PieceKind::Tetra => {
            if knobs.branch > knobs.cavern {
                1
            } else {
                0
            }
        }
        PieceKind::Dodeca => {
            if knobs.branch > knobs.throat {
                3
            } else {
                0
            }
        }
        PieceKind::Icosa => {
            if knobs.throat > knobs.cavern {
                3
            } else {
                0
            }
        }
    };
    (base + slot + (js_sys::Math::random() * 2.0) as usize) % 4
}

fn features(kind: PieceKind, rotation: usize) -> (f64, f64, f64) {
    let cells = piece_cells(kind, rotation);
    let min_x = cells.iter().map(|(x, _)| *x).min().unwrap_or(0);
    let max_x = cells.iter().map(|(x, _)| *x).max().unwrap_or(0);
    let min_y = cells.iter().map(|(_, y)| *y).min().unwrap_or(0);
    let max_y = cells.iter().map(|(_, y)| *y).max().unwrap_or(0);

    let width = (max_x - min_x + 1) as f64;
    let height = (max_y - min_y + 1) as f64;
    let area = width * height;
    let fill = cells.len() as f64 / area.max(1.0);
    let cavern = (cells.len() as f64 / 5.0) * (1.1 - fill * 0.4);
    let throat = (width.min(height) / width.max(height)).clamp(0.0, 1.0);

    let mut branch_score: f64 = 0.0;
    for &(x, y) in cells {
        let degree = cells
            .iter()
            .filter(|(ox, oy)| (x - *ox).abs() + (y - *oy).abs() == 1)
            .count();
        if degree == 1 {
            branch_score += 0.42;
        } else if degree == 3 || degree == 4 {
            branch_score += 0.82;
        }
    }

    (cavern.clamp(0.0, 1.0), throat, (branch_score / 2.5).clamp(0.0, 1.0))
}

fn trait_score(objective_axis: usize, knobs: GeneratorKnobs, traits: (f64, f64, f64)) -> f64 {
    let knob_mix = knobs.cavern * traits.0 + knobs.throat * traits.1 + knobs.branch * traits.2;
    let pressure = match objective_axis {
        0 => traits.0 * 0.9 + traits.2 * 0.2,
        1 => traits.1 * 1.05 + traits.0 * 0.1,
        _ => traits.2 * 0.95 + traits.0 * 0.15,
    };
    (knob_mix + pressure).clamp(0.0, 2.0)
}

fn novelty_bonus(elite: Option<PieceKind>, kind: PieceKind) -> f64 {
    if elite == Some(kind) {
        0.35
    } else {
        0.85
    }
}

fn feature_badge(traits: (f64, f64, f64)) -> &'static str {
    if traits.0 > traits.1 && traits.0 > traits.2 {
        "cavernous"
    } else if traits.1 > traits.2 {
        "stable throat"
    } else {
        "branchy"
    }
}

fn default_knobs(level: &LevelSpec) -> GeneratorKnobs {
    match level.objective_kind {
        ObjectiveKind::AccessibleVolume { .. } => GeneratorKnobs {
            cavern: 0.82,
            throat: 0.34,
            branch: 0.44,
        },
        ObjectiveKind::ProbeToVent { .. } => GeneratorKnobs {
            cavern: 0.36,
            throat: 0.86,
            branch: 0.28,
        },
        ObjectiveKind::AdsorbPockets { .. } => GeneratorKnobs {
            cavern: 0.48,
            throat: 0.30,
            branch: 0.90,
        },
    }
}

fn centered_spawn_x(kind: PieceKind, rotation: usize) -> i32 {
    let max_x = piece_cells(kind, rotation)
        .iter()
        .map(|(x, _)| *x)
        .max()
        .unwrap_or(0);
    ((BOARD_W as i32 - (max_x + 1)) / 2).max(0)
}

fn read_range_value(document: &Document, id: &str) -> Result<f64, JsValue> {
    let input = document
        .get_element_by_id(id)
        .ok_or_else(|| JsValue::from_str("input missing"))?
        .dyn_into::<HtmlInputElement>()?;
    Ok((input.value().parse::<f64>().unwrap_or(50.0) / 100.0).clamp(0.0, 1.0))
}

fn set_range_value(document: &Document, id: &str, value: f64) -> Result<(), JsValue> {
    let input = document
        .get_element_by_id(id)
        .ok_or_else(|| JsValue::from_str("input missing"))?
        .dyn_into::<HtmlInputElement>()?;
    input.set_value(&format!("{:.0}", value * 100.0));
    Ok(())
}
