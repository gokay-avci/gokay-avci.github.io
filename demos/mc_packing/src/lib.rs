use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Array, Reflect};
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use web_sys::{
    Blob, CanvasRenderingContext2d, Document, HtmlAnchorElement, HtmlButtonElement, HtmlCanvasElement,
    HtmlInputElement, HtmlSelectElement, Location, Navigator, Url, UrlSearchParams, Window,
};

const TAU: f64 = std::f64::consts::TAU;

// ==============================
// Types
// ==============================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShapeKind {
    Disk,
    Rect,
    Tri,
    Square,
    Hex,
    Oct,
}

impl ShapeKind {
    fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "disk" => Self::Disk,
            "rect" => Self::Rect,
            "tri" => Self::Tri,
            "triangle" => Self::Tri,
            "square" => Self::Square,
            "hex" => Self::Hex,
            "hexagon" => Self::Hex,
            "oct" => Self::Oct,
            "octagon" => Self::Oct,
            _ => Self::Disk,
        }
    }
    fn as_str(&self) -> &'static str {
        match self {
            Self::Disk => "disk",
            Self::Rect => "rect",
            Self::Tri => "tri",
            Self::Square => "square",
            Self::Hex => "hex",
            Self::Oct => "oct",
        }
    }
    fn polygon_sides(&self) -> Option<usize> {
        match self {
            Self::Tri => Some(3),
            Self::Square => Some(4),
            Self::Hex => Some(6),
            Self::Oct => Some(8),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeometryKind {
    Box,
    Circle,
}
impl GeometryKind {
    fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "circle" => Self::Circle,
            _ => Self::Box,
        }
    }
    fn as_str(&self) -> &'static str {
        match self {
            Self::Box => "box",
            Self::Circle => "circle",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Pt {
    x: f64,
    y: f64,
}

#[derive(Clone, Debug)]
struct ShapeDef {
    kind: ShapeKind,
    // For convex polygon shapes, vertices are stored as UNIT vertices (circumradius=1)
    // Then we scale by (r_eff) at runtime.
    unit_poly: Option<Vec<Pt>>,
    // For rect, we store unit rect with minor half-extent=1, major half-extent=aspect.
    rect_aspect: f64,

    // Render tint
    rgba: (u8, u8, u8),
}

impl ShapeDef {
    fn unit_for(kind: ShapeKind, aspect: f64) -> Self {
        let rgba = match kind {
            ShapeKind::Disk => (160, 200, 255),
            ShapeKind::Rect => (255, 200, 160),
            ShapeKind::Tri => (210, 255, 170),
            ShapeKind::Square => (255, 240, 170),
            ShapeKind::Hex => (190, 170, 255),
            ShapeKind::Oct => (170, 240, 255),
        };

        let unit_poly = match kind {
            ShapeKind::Disk => None,
            ShapeKind::Rect => Some(unit_rect(aspect.max(1.0))),
            other => other.polygon_sides().map(unit_regular_polygon),
        };

        Self {
            kind,
            unit_poly,
            rect_aspect: aspect.max(1.0),
            rgba,
        }
    }

    fn is_rotatable(&self) -> bool { self.kind != ShapeKind::Disk }

    fn world_poly(&self, center: Pt, theta: f64, r_eff: f64) -> Option<Vec<Pt>> {
        let unit = self.unit_poly.as_ref()?;
        Some(unit.iter().map(|&p| add(rot(mul(p, r_eff), theta), center)).collect())
    }

    fn area(&self, r_eff: f64) -> f64 {
        match self.kind {
            ShapeKind::Disk => std::f64::consts::PI * r_eff * r_eff,
            ShapeKind::Rect => {
                // unit rect points represent (±aspect, ±1) scaled by r_eff
                let hx = self.rect_aspect * r_eff;
                let hy = 1.0 * r_eff;
                (2.0 * hx) * (2.0 * hy)
            }
            _ => {
                let n = self.kind.polygon_sides().unwrap_or(6) as f64;
                // circumradius r_eff:
                0.5 * n * r_eff * r_eff * (TAU / n).sin()
            }
        }
    }

    fn bounding_radius(&self, r_eff: f64) -> f64 {
        match self.kind {
            ShapeKind::Disk => r_eff,
            ShapeKind::Rect => {
                // half-diagonal
                let hx = self.rect_aspect * r_eff;
                let hy = 1.0 * r_eff;
                (hx*hx + hy*hy).sqrt()
            }
            _ => r_eff, // polygon circumradius
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Particle {
    pos: Pt,
    theta: f64,
    shape_id: usize,
    // Optional: per-particle size factor (kept at 1.0 unless you want polydispersity)
    size_factor: f64,
}

#[derive(Clone, Debug)]
struct Config {
    seed: u64,
    n: usize,

    // global base radius (final)
    r: f64,
    // for rect (unit minor half-extent=1, major=aspect); affects rect + (optional) mix rect
    aspect: f64,

    // shrink schedule: start larger and shrink toward final
    shrink_enabled: bool,
    r0_mult: f64,          // starting radius multiplier (e.g. 2.5)
    shrink_tau: f64,       // progress scale in moves (larger = slower shrink)

    // MC step sizes
    step_t: f64,
    step_r: f64,
    vol_step: f64,

    // thermodynamic knobs
    beta: f64,
    pressure: f64,
    anneal_hardness: bool,
    hardness_tau: f64,

    // geometry
    geom: GeometryKind,
    pbc: bool,

    // shapes: single or mix
    mix_shapes: bool,
    primary_shape: ShapeKind,

    // weights if mixing (non-negative; if all zero, fallback to primary)
    w_disk: f64,
    w_rect: f64,
    w_tri: f64,
    w_square: f64,
    w_hex: f64,
    w_oct: f64,

    // box in sim units
    lx: f64,
    ly: f64,

    // replay counter (URL)
    iters: u64,
}

impl Config {
    fn defaults(canvas_w: u32, canvas_h: u32) -> Self {
        Self {
            seed: 42,
            n: 220,
            r: 6.0,
            aspect: 2.0,

            shrink_enabled: true,
            r0_mult: 2.0,
            shrink_tau: 250_000.0,

            step_t: 8.0,
            step_r: 0.25,
            vol_step: 0.02,

            beta: 2.0,
            pressure: 2.5,
            anneal_hardness: true,
            hardness_tau: 250_000.0,

            geom: GeometryKind::Box,
            pbc: true,

            mix_shapes: false,
            primary_shape: ShapeKind::Disk,

            w_disk: 1.0,
            w_rect: 0.0,
            w_tri: 0.0,
            w_square: 0.0,
            w_hex: 0.0,
            w_oct: 0.0,

            lx: canvas_w as f64,
            ly: canvas_h as f64,

            iters: 0,
        }
    }

    fn clamp(&mut self) {
        self.n = self.n.clamp(1, 6000);
        self.r = self.r.clamp(1.0, 80.0);
        self.aspect = self.aspect.clamp(1.0, 10.0);

        self.r0_mult = self.r0_mult.clamp(1.0, 10.0);
        self.shrink_tau = self.shrink_tau.clamp(1_000.0, 5_000_000.0);
        self.hardness_tau = self.hardness_tau.clamp(1_000.0, 5_000_000.0);

        self.step_t = self.step_t.clamp(0.0, 250.0);
        self.step_r = self.step_r.clamp(0.0, 2.0);
        self.vol_step = self.vol_step.clamp(0.0, 0.25);

        self.beta = self.beta.clamp(0.01, 80.0);
        self.pressure = self.pressure.clamp(0.0, 80.0);

        self.lx = self.lx.max(120.0);
        self.ly = self.ly.max(120.0);

        if self.geom == GeometryKind::Circle {
            self.pbc = false;
        }

        // weights non-negative
        self.w_disk = self.w_disk.max(0.0);
        self.w_rect = self.w_rect.max(0.0);
        self.w_tri = self.w_tri.max(0.0);
        self.w_square = self.w_square.max(0.0);
        self.w_hex = self.w_hex.max(0.0);
        self.w_oct = self.w_oct.max(0.0);
    }
}

// ==============================
// Small math helpers
// ==============================

fn dot(a: Pt, b: Pt) -> f64 { a.x*b.x + a.y*b.y }
fn add(a: Pt, b: Pt) -> Pt { Pt { x: a.x+b.x, y: a.y+b.y } }
fn sub(a: Pt, b: Pt) -> Pt { Pt { x: a.x-b.x, y: a.y-b.y } }
fn mul(a: Pt, s: f64) -> Pt { Pt { x: a.x*s, y: a.y*s } }
fn len2(a: Pt) -> f64 { dot(a,a) }
fn clamp01(x: f64) -> f64 { x.max(0.0).min(1.0) }

fn wrap_angle(mut t: f64) -> f64 {
    while t < -std::f64::consts::PI { t += TAU; }
    while t >  std::f64::consts::PI { t -= TAU; }
    t
}

fn rot(p: Pt, theta: f64) -> Pt {
    let c = theta.cos();
    let s = theta.sin();
    Pt { x: c*p.x - s*p.y, y: s*p.x + c*p.y }
}

fn ease_smoothstep(t: f64) -> f64 {
    let t = clamp01(t);
    t*t*(3.0 - 2.0*t)
}

// ==============================
// Shape generators (unit space)
// ==============================

fn unit_regular_polygon(n: usize) -> Vec<Pt> {
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let a = TAU * (i as f64) / (n as f64);
        v.push(Pt { x: a.cos(), y: a.sin() });
    }
    v
}

// unit rect with minor half-extent = 1, major half-extent = aspect
fn unit_rect(aspect: f64) -> Vec<Pt> {
    let a = aspect.max(1.0);
    vec![
        Pt { x: -a, y: -1.0 },
        Pt { x:  a, y: -1.0 },
        Pt { x:  a, y:  1.0 },
        Pt { x: -a, y:  1.0 },
    ]
}

// ==============================
// Collision via SAT (poly/poly, circle/poly)
// ==============================

fn polygon_axes(poly: &[Pt]) -> Vec<Pt> {
    let mut axes = Vec::with_capacity(poly.len());
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let e = sub(b, a);
        // perpendicular
        axes.push(Pt { x: -e.y, y: e.x });
    }
    axes
}

fn project_poly(poly: &[Pt], axis: Pt) -> (f64, f64) {
    let mut minv = dot(poly[0], axis);
    let mut maxv = minv;
    for &p in poly.iter().skip(1) {
        let v = dot(p, axis);
        if v < minv { minv = v; }
        if v > maxv { maxv = v; }
    }
    (minv, maxv)
}

fn project_circle(center: Pt, radius: f64, axis: Pt) -> (f64, f64) {
    let c = dot(center, axis);
    let axis_len = (axis.x*axis.x + axis.y*axis.y).sqrt();
    if axis_len <= 1e-12 {
        return (c, c);
    }
    let r = radius * axis_len;
    (c - r, c + r)
}

fn overlap_1d(a: (f64, f64), b: (f64, f64)) -> f64 {
    let lo = a.0.max(b.0);
    let hi = a.1.min(b.1);
    (hi - lo).max(0.0)
}

fn sat_penetration_poly_poly(a: &[Pt], b: &[Pt]) -> f64 {
    let mut min_depth = f64::INFINITY;

    for axis in polygon_axes(a).into_iter().chain(polygon_axes(b)) {
        let pa = project_poly(a, axis);
        let pb = project_poly(b, axis);
        let ov = overlap_1d(pa, pb);
        if ov <= 0.0 {
            return 0.0;
        }
        let axis_len = (axis.x*axis.x + axis.y*axis.y).sqrt();
        if axis_len > 1e-12 {
            let depth = ov / axis_len;
            if depth < min_depth { min_depth = depth; }
        }
    }

    if min_depth.is_finite() { min_depth } else { 0.0 }
}

fn closest_vertex_axis(center: Pt, poly: &[Pt]) -> Option<Pt> {
    let mut best = None;
    let mut best_d2 = f64::INFINITY;
    for &v in poly {
        let d = sub(center, v);
        let d2 = len2(d);
        if d2 < best_d2 {
            best_d2 = d2;
            best = Some(d);
        }
    }
    best
}

fn sat_penetration_circle_poly(c: Pt, r: f64, poly: &[Pt]) -> f64 {
    let mut min_depth = f64::INFINITY;

    // axes: poly normals
    for axis in polygon_axes(poly) {
        let pc = project_circle(c, r, axis);
        let pp = project_poly(poly, axis);
        let ov = overlap_1d(pc, pp);
        if ov <= 0.0 {
            return 0.0;
        }
        let axis_len = (axis.x*axis.x + axis.y*axis.y).sqrt();
        if axis_len > 1e-12 {
            let depth = ov / axis_len;
            if depth < min_depth { min_depth = depth; }
        }
    }

    // plus axis to closest vertex (important for circle-poly SAT)
    if let Some(axis) = closest_vertex_axis(c, poly) {
        let axis_len = (axis.x*axis.x + axis.y*axis.y).sqrt();
        if axis_len > 1e-12 {
            let pc = project_circle(c, r, axis);
            let pp = project_poly(poly, axis);
            let ov = overlap_1d(pc, pp);
            if ov <= 0.0 {
                return 0.0;
            }
            let depth = ov / axis_len;
            if depth < min_depth { min_depth = depth; }
        }
    }

    if min_depth.is_finite() { min_depth } else { 0.0 }
}

// ==============================
// Sim
// ==============================

struct Sim {
    cfg: Config,
    rng: ChaCha8Rng,

    shapes: Vec<ShapeDef>,
    parts: Vec<Particle>,

    // acceptance bookkeeping
    att_t: u64, acc_t: u64,
    att_r: u64, acc_r: u64,
    att_v: u64, acc_v: u64,

    running: bool,
    status: String,

    // internal progress (0..1) for shrink/hardness
    shrink_prog: f64,
    hard_prog: f64,
}

impl Sim {
    fn new(mut cfg: Config) -> Self {
        cfg.clamp();

        let mut s = Self {
            rng: ChaCha8Rng::seed_from_u64(cfg.seed),
            cfg,
            shapes: Vec::new(),
            parts: Vec::new(),

            att_t: 0, acc_t: 0,
            att_r: 0, acc_r: 0,
            att_v: 0, acc_v: 0,

            running: true,
            status: String::new(),

            shrink_prog: 0.0,
            hard_prog: 0.0,
        };
        s.rebuild_shapes();
        s.reset();
        s
    }

    fn set_status<S: Into<String>>(&mut self, s: S) {
        self.status = s.into();
    }

    fn rebuild_shapes(&mut self) {
        self.shapes.clear();

        // Always build defs we might need; mixture selects from these.
        // Note: rect_aspect uses cfg.aspect.
        let rect_aspect = self.cfg.aspect.max(1.0);

        let defs = [
            ShapeDef::unit_for(ShapeKind::Disk, rect_aspect),
            ShapeDef::unit_for(ShapeKind::Rect, rect_aspect),
            ShapeDef::unit_for(ShapeKind::Tri, rect_aspect),
            ShapeDef::unit_for(ShapeKind::Square, rect_aspect),
            ShapeDef::unit_for(ShapeKind::Hex, rect_aspect),
            ShapeDef::unit_for(ShapeKind::Oct, rect_aspect),
        ];
        self.shapes.extend(defs);
    }

    fn reset(&mut self) {
        self.cfg.clamp();
        self.rng = ChaCha8Rng::seed_from_u64(self.cfg.seed);

        self.parts.clear();
        self.cfg.iters = 0;

        self.att_t = 0; self.acc_t = 0;
        self.att_r = 0; self.acc_r = 0;
        self.att_v = 0; self.acc_v = 0;

        self.shrink_prog = 0.0;
        self.hard_prog = 0.0;

        self.place_initial();
    }

    fn area_box(&self) -> f64 { self.cfg.lx * self.cfg.ly }

    fn eff_r_scale(&self) -> f64 {
        // shrink from r0_mult -> 1.0
        if !self.cfg.shrink_enabled {
            return 1.0;
        }
        let t = ease_smoothstep(self.shrink_prog);
        self.cfg.r0_mult * (1.0 - t) + 1.0 * t
    }

    fn eff_r(&self, p: &Particle) -> f64 {
        self.cfg.r * self.eff_r_scale() * p.size_factor
    }

    fn hardness_params(&self) -> (f64, f64) {
        // soft-permeable -> hard-opaque
        if !self.cfg.anneal_hardness {
            return (40.0, 3.0);
        }
        let t = ease_smoothstep(self.hard_prog);
        // k: 0.6 -> 65, exponent: 1.3 -> 3.0
        let k = 0.6 + t * 64.4;
        let p = 1.3 + t * 1.7;
        (k, p)
    }

    fn particle_area(&self, p: &Particle) -> f64 {
        let def = &self.shapes[p.shape_id];
        def.area(self.eff_r(p))
    }

    fn packing_fraction(&self) -> f64 {
        let a = self.area_box();
        if a <= 0.0 { return 0.0; }
        let sum = self.parts.iter().map(|p| self.particle_area(p)).sum::<f64>();
        sum / a
    }

    fn random_position_inside(&mut self) -> Pt {
        match self.cfg.geom {
            GeometryKind::Box => Pt {
                x: self.rng.gen_range(0.0..self.cfg.lx),
                y: self.rng.gen_range(0.0..self.cfg.ly),
            },
            GeometryKind::Circle => {
                let cx = self.cfg.lx * 0.5;
                let cy = self.cfg.ly * 0.5;
                let rmax = 0.5 * self.cfg.lx.min(self.cfg.ly);
                let u: f64 = self.rng.gen();
                let rr = rmax * u.sqrt();
                let ang = self.rng.gen_range(0.0..TAU);
                Pt { x: cx + rr*ang.cos(), y: cy + rr*ang.sin() }
            }
        }
    }

    fn confine(&self, p: &mut Particle) {
        match self.cfg.geom {
            GeometryKind::Box => {
                if self.cfg.pbc {
                    if p.pos.x < 0.0 { p.pos.x += self.cfg.lx; }
                    if p.pos.x >= self.cfg.lx { p.pos.x -= self.cfg.lx; }
                    if p.pos.y < 0.0 { p.pos.y += self.cfg.ly; }
                    if p.pos.y >= self.cfg.ly { p.pos.y -= self.cfg.ly; }
                } else {
                    p.pos.x = p.pos.x.clamp(0.0, self.cfg.lx);
                    p.pos.y = p.pos.y.clamp(0.0, self.cfg.ly);
                }
            }
            GeometryKind::Circle => {
                let cx = self.cfg.lx * 0.5;
                let cy = self.cfg.ly * 0.5;
                let rmax = 0.5 * self.cfg.lx.min(self.cfg.ly);
                let v = Pt { x: p.pos.x - cx, y: p.pos.y - cy };
                let r = (v.x*v.x + v.y*v.y).sqrt();
                if r > rmax {
                    let s = rmax / r;
                    p.pos.x = cx + v.x*s;
                    p.pos.y = cy + v.y*s;
                }
            }
        }
    }

    fn min_image(&self, mut d: Pt) -> Pt {
        if self.cfg.pbc && self.cfg.geom == GeometryKind::Box {
            let lx = self.cfg.lx;
            let ly = self.cfg.ly;
            if d.x >  0.5*lx { d.x -= lx; }
            if d.x < -0.5*lx { d.x += lx; }
            if d.y >  0.5*ly { d.y -= ly; }
            if d.y < -0.5*ly { d.y += ly; }
        }
        d
    }

    fn choose_shape_id(&mut self) -> usize {
        // Shapes vector order:
        // 0 disk, 1 rect, 2 tri, 3 square, 4 hex, 5 oct
        if !self.cfg.mix_shapes {
            // map primary to index
            return match self.cfg.primary_shape {
                ShapeKind::Disk => 0,
                ShapeKind::Rect => 1,
                ShapeKind::Tri => 2,
                ShapeKind::Square => 3,
                ShapeKind::Hex => 4,
                ShapeKind::Oct => 5,
            };
        }

        let weights = [
            self.cfg.w_disk,
            self.cfg.w_rect,
            self.cfg.w_tri,
            self.cfg.w_square,
            self.cfg.w_hex,
            self.cfg.w_oct,
        ];
        let sum: f64 = weights.iter().sum();
        if sum <= 0.0 {
            // fallback to primary
            return match self.cfg.primary_shape {
                ShapeKind::Disk => 0,
                ShapeKind::Rect => 1,
                ShapeKind::Tri => 2,
                ShapeKind::Square => 3,
                ShapeKind::Hex => 4,
                ShapeKind::Oct => 5,
            };
        }

        let mut r = self.rng.gen::<f64>() * sum;
        for (i, &w) in weights.iter().enumerate() {
            if w <= 0.0 { continue; }
            if r <= w { return i; }
            r -= w;
        }
        0
    }

    fn place_initial(&mut self) {
        let mut placed = 0usize;
        let max_tries = 20_000usize;

        // Place in a soft-start regime so even “too-large” particles can settle via shrink+hardening.
        // If both shrink and hardness anneal are off, we try to avoid overlap at init (best-effort).
        let allow_overlap = self.cfg.shrink_enabled || self.cfg.anneal_hardness;

        for _ in 0..self.cfg.n {
            let shape_id = self.choose_shape_id();
            let rot = self.shapes[shape_id].is_rotatable();

            let mut ok = false;
            for _ in 0..max_tries {
                let pos = self.random_position_inside();
                let theta = if rot { self.rng.gen_range(-std::f64::consts::PI..=std::f64::consts::PI) } else { 0.0 };

                let cand = Particle {
                    pos,
                    theta,
                    shape_id,
                    size_factor: 1.0,
                };

                if allow_overlap {
                    self.parts.push(cand);
                    placed += 1;
                    ok = true;
                    break;
                } else {
                    if self.max_penetration_with(&cand, None) <= 0.0 {
                        self.parts.push(cand);
                        placed += 1;
                        ok = true;
                        break;
                    }
                }
            }
            if !ok { break; }
        }

        self.set_status(format!(
            "Init placed {}/{}. mix_shapes={}, shrink={}, hardness_anneal={}.",
            placed, self.cfg.n,
            self.cfg.mix_shapes as u8,
            self.cfg.shrink_enabled as u8,
            self.cfg.anneal_hardness as u8
        ));
    }

    fn bounding_early_reject(&self, a: &Particle, b: &Particle, d: Pt) -> bool {
        let ra = self.shapes[a.shape_id].bounding_radius(self.eff_r(a));
        let rb = self.shapes[b.shape_id].bounding_radius(self.eff_r(b));
        let dist2 = d.x*d.x + d.y*d.y;
        dist2 >= (ra + rb) * (ra + rb)
    }

    fn pair_penetration(&self, a: &Particle, b: &Particle) -> f64 {
        let d = self.min_image(sub(a.pos, b.pos));

        if self.bounding_early_reject(a, b, d) {
            return 0.0;
        }

        let da = &self.shapes[a.shape_id];
        let db = &self.shapes[b.shape_id];

        let ra = self.eff_r(a);
        let rb = self.eff_r(b);

        // Bring b to the nearest image of a (important for polygon transforms)
        let b_center = add(a.pos, mul(d, -1.0)); // b in a's image frame

        match (da.kind, db.kind) {
            (ShapeKind::Disk, ShapeKind::Disk) => {
                let dist = (d.x*d.x + d.y*d.y).sqrt();
                (ra + rb - dist).max(0.0)
            }
            (ShapeKind::Disk, _) => {
                let poly = db.world_poly(b_center, b.theta, rb).unwrap();
                // circle center is a.pos in same frame
                sat_penetration_circle_poly(a.pos, ra, &poly)
            }
            (_, ShapeKind::Disk) => {
                let poly = da.world_poly(a.pos, a.theta, ra).unwrap();
                sat_penetration_circle_poly(b_center, rb, &poly)
            }
            _ => {
                let poly_a = da.world_poly(a.pos, a.theta, ra).unwrap();
                let poly_b = db.world_poly(b_center, b.theta, rb).unwrap();
                sat_penetration_poly_poly(&poly_a, &poly_b)
            }
        }
    }

    fn energy_and_overlap_stats(&self) -> (f64, f64, f64) {
        let (k, pexp) = self.hardness_params();

        let mut u = 0.0;
        let mut overlap_pairs = 0u64;
        let mut max_pen = 0.0;

        let n = self.parts.len();
        for i in 0..n {
            for j in (i+1)..n {
                let depth = self.pair_penetration(&self.parts[i], &self.parts[j]);
                if depth > 0.0 {
                    overlap_pairs += 1;
                    if depth > max_pen { max_pen = depth; }
                    u += k * depth.powf(pexp);
                }
            }
        }

        let total_pairs = (n as u64) * ((n as u64).saturating_sub(1)) / 2;
        let frac = if total_pairs == 0 { 0.0 } else { overlap_pairs as f64 / total_pairs as f64 };
        (u, frac, max_pen)
    }

    fn max_penetration_with(&self, cand: &Particle, skip: Option<usize>) -> f64 {
        let mut max_pen = 0.0;
        for (j, b) in self.parts.iter().enumerate() {
            if Some(j) == skip { continue; }
            let depth = self.pair_penetration(cand, b);
            if depth > max_pen { max_pen = depth; }
        }
        max_pen
    }

    fn metropolis_accept(&mut self, delta_energy: f64, jacobian_term: f64) -> bool {
        if delta_energy <= 0.0 {
            return true;
        }
        let logp = -self.cfg.beta * delta_energy + jacobian_term;
        let u: f64 = self.rng.gen();
        u.ln() < logp
    }

    fn propose_translate(&mut self) {
        if self.parts.is_empty() { return; }
        self.att_t += 1;
        self.cfg.iters += 1;

        let i = self.rng.gen_range(0..self.parts.len());
        let old = self.parts[i];

        let dx = self.rng.gen_range(-self.cfg.step_t..=self.cfg.step_t);
        let dy = self.rng.gen_range(-self.cfg.step_t..=self.cfg.step_t);

        let mut cand = old;
        cand.pos.x += dx;
        cand.pos.y += dy;
        self.confine(&mut cand);

        let (k, pexp) = self.hardness_params();
        let mut delta_energy = 0.0;

        for (j, b) in self.parts.iter().enumerate() {
            if j == i { continue; }
            let old_depth = self.pair_penetration(&old, b);
            let new_depth = self.pair_penetration(&cand, b);
            delta_energy += k * (new_depth.powf(pexp) - old_depth.powf(pexp));
        }

        if self.metropolis_accept(delta_energy, 0.0) {
            self.parts[i] = cand;
            self.acc_t += 1;
        }
    }

    fn propose_rotate(&mut self) {
        if self.parts.is_empty() { return; }
        let shape_id = self.parts[self.rng.gen_range(0..self.parts.len())].shape_id;
        if !self.shapes[shape_id].is_rotatable() {
            // skip rotation moves for disks
            self.cfg.iters += 1;
            return;
        }

        self.att_r += 1;
        self.cfg.iters += 1;

        let i = self.rng.gen_range(0..self.parts.len());
        let old = self.parts[i];
        if !self.shapes[old.shape_id].is_rotatable() { return; }

        let dtheta = self.rng.gen_range(-self.cfg.step_r..=self.cfg.step_r);

        let mut cand = old;
        cand.theta = wrap_angle(cand.theta + dtheta);

        let (k, pexp) = self.hardness_params();
        let mut delta_energy = 0.0;

        for (j, b) in self.parts.iter().enumerate() {
            if j == i { continue; }
            let old_depth = self.pair_penetration(&old, b);
            let new_depth = self.pair_penetration(&cand, b);
            delta_energy += k * (new_depth.powf(pexp) - old_depth.powf(pexp));
        }

        if self.metropolis_accept(delta_energy, 0.0) {
            self.parts[i] = cand;
            self.acc_r += 1;
        }
    }

    fn propose_volume_move(&mut self) {
        if self.cfg.geom != GeometryKind::Box { return; }
        self.att_v += 1;
        self.cfg.iters += 1;

        let delta = self.rng.gen_range(-self.cfg.vol_step..=self.cfg.vol_step);
        let scale = delta.exp();

        let old_lx = self.cfg.lx;
        let old_ly = self.cfg.ly;
        let new_lx = (old_lx * scale).max(120.0);
        let new_ly = (old_ly * scale).max(120.0);

        let mut new_parts = self.parts.clone();
        for p in &mut new_parts {
            p.pos.x *= new_lx / old_lx;
            p.pos.y *= new_ly / old_ly;
        }

        let old_cfg = self.cfg.clone();
        let (old_u, _, _) = self.energy_and_overlap_stats();

        self.cfg.lx = new_lx;
        self.cfg.ly = new_ly;
        let old_parts = std::mem::replace(&mut self.parts, new_parts);
        let (new_u, _, _) = self.energy_and_overlap_stats();

        let delta_energy = new_u - old_u;

        let old_a = old_lx * old_ly;
        let new_a = new_lx * new_ly;
        let p = self.cfg.pressure;

        let delta_enthalpy = delta_energy + p * (new_a - old_a);
        let jac = (self.parts.len() as f64) * (new_a / old_a).ln();

        let ok = self.metropolis_accept(delta_enthalpy, jac);

        if ok {
            self.acc_v += 1;
        } else {
            self.parts = old_parts;
            self.cfg = old_cfg;
        }
    }

    fn adapt_steps(&mut self) {
        fn acc(att: u64, ok: u64) -> f64 {
            if att < 100 { return 0.0; }
            ok as f64 / att as f64
        }

        let at = acc(self.att_t, self.acc_t);
        if self.att_t > 500 {
            if at > 0.55 { self.cfg.step_t *= 1.02; }
            if at < 0.25 { self.cfg.step_t *= 0.98; }
            self.cfg.step_t = self.cfg.step_t.clamp(0.2, 400.0);
        }

        let ar = acc(self.att_r, self.acc_r);
        if self.att_r > 500 {
            if ar > 0.55 { self.cfg.step_r *= 1.02; }
            if ar < 0.25 { self.cfg.step_r *= 0.98; }
            self.cfg.step_r = self.cfg.step_r.clamp(0.01, 2.0);
        }

        let av = acc(self.att_v, self.acc_v);
        if self.att_v > 250 {
            if av > 0.55 { self.cfg.vol_step *= 1.02; }
            if av < 0.25 { self.cfg.vol_step *= 0.98; }
            self.cfg.vol_step = self.cfg.vol_step.clamp(0.001, 0.25);
        }
    }

    fn update_progress(&mut self) {
        // decouple shrink/hardness schedules so you can tune independently
        let it = self.cfg.iters as f64;

        if self.cfg.shrink_enabled {
            self.shrink_prog = clamp01(it / self.cfg.shrink_tau);
        } else {
            self.shrink_prog = 1.0;
        }

        if self.cfg.anneal_hardness {
            self.hard_prog = clamp01(it / self.cfg.hardness_tau);
        } else {
            self.hard_prog = 1.0;
        }
    }

    fn mc_sweep(&mut self, moves: u32) {
        self.update_progress();

        for _ in 0..moves {
            let u: f64 = self.rng.gen();
            if u < 0.80 {
                self.propose_translate();
            } else if u < 0.95 {
                self.propose_rotate();
            } else {
                if self.cfg.geom == GeometryKind::Box {
                    self.propose_volume_move();
                } else {
                    self.propose_translate();
                }
            }
        }

        self.adapt_steps();
    }
}

// ==============================
// DOM helpers (defensive + select support)
// ==============================

fn window() -> Window { web_sys::window().expect("no window") }
fn document() -> Document { window().document().expect("no document") }

fn set_text(id: &str, s: &str) {
    if let Some(el) = document().get_element_by_id(id) {
        el.set_text_content(Some(s));
    }
}

fn get_canvas(id: &str) -> Result<HtmlCanvasElement, JsValue> {
    document()
        .get_element_by_id(id)
        .ok_or_else(|| JsValue::from_str("Canvas not found"))?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| JsValue::from_str("Element is not a canvas"))
}

fn get_input(id: &str) -> Option<HtmlInputElement> {
    document().get_element_by_id(id)?.dyn_into::<HtmlInputElement>().ok()
}

fn get_select(id: &str) -> Option<HtmlSelectElement> {
    document().get_element_by_id(id)?.dyn_into::<HtmlSelectElement>().ok()
}

fn get_button(id: &str) -> Option<HtmlButtonElement> {
    document().get_element_by_id(id)?.dyn_into::<HtmlButtonElement>().ok()
}

/// Read "value" from either <input> or <select>.
fn get_value(id: &str) -> Option<String> {
    if let Some(i) = get_input(id) {
        return Some(i.value());
    }
    if let Some(s) = get_select(id) {
        return Some(s.value());
    }
    None
}

fn get_checked(id: &str) -> Option<bool> {
    get_input(id).map(|i| i.checked())
}

fn parse_query_u64(params: &UrlSearchParams, key: &str, default: u64, min: u64, max: u64) -> u64 {
    params.get(key).and_then(|v| v.parse::<u64>().ok()).map(|v| v.clamp(min, max)).unwrap_or(default)
}
fn parse_query_usize(params: &UrlSearchParams, key: &str, default: usize, min: usize, max: usize) -> usize {
    params.get(key).and_then(|v| v.parse::<usize>().ok()).map(|v| v.clamp(min, max)).unwrap_or(default)
}
fn parse_query_f64(params: &UrlSearchParams, key: &str, default: f64, min: f64, max: f64) -> f64 {
    params.get(key).and_then(|v| v.parse::<f64>().ok()).map(|v| v.clamp(min, max)).unwrap_or(default)
}
fn parse_query_bool(params: &UrlSearchParams, key: &str, default: bool) -> bool {
    params.get(key)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
        .unwrap_or(default)
}
fn parse_query_str(params: &UrlSearchParams, key: &str, default: &str) -> String {
    params.get(key).unwrap_or_else(|| default.to_string())
}

// ==============================
// Rendering (no deprecated setters)
// ==============================

fn set_ctx_style(ctx: &CanvasRenderingContext2d, prop: &str, value: &str) -> Result<(), JsValue> {
    Reflect::set(ctx.as_ref(), &JsValue::from_str(prop), &JsValue::from_str(value))?;
    Ok(())
}

fn render(ctx: &CanvasRenderingContext2d, cw: f64, ch: f64, sim: &Sim) -> Result<(), JsValue> {
    let sx = cw / sim.cfg.lx;
    let sy = ch / sim.cfg.ly;
    let pix_scale = sx.min(sy);

    // Clear to transparent then subtle plate
    set_ctx_style(ctx, "fillStyle", "rgba(0,0,0,0)")?;
    ctx.fill_rect(0.0, 0.0, cw, ch);

    set_ctx_style(ctx, "fillStyle", "rgba(12,12,14,0.70)")?;
    ctx.fill_rect(0.0, 0.0, cw, ch);

    // boundary
    if sim.cfg.geom == GeometryKind::Circle {
        set_ctx_style(ctx, "strokeStyle", "rgba(255,255,255,0.14)")?;
        ctx.set_line_width(2.0);
        let cx = 0.5 * cw;
        let cy = 0.5 * ch;
        let r = 0.5 * cw.min(ch) - 2.0;
        ctx.begin_path();
        ctx.arc(cx, cy, r, 0.0, TAU)?;
        ctx.stroke();
    }

    let hard = ease_smoothstep(sim.hard_prog);
    let alpha = 0.55 + 0.35 * hard;

    // Draw each particle using its own shape definition => shapes *visibly* differ.
    // For mixed shapes, we also color-code by kind.
    for p in &sim.parts {
        let def = &sim.shapes[p.shape_id];
        let r_eff = sim.eff_r(p);

        let (rr, gg, bb) = def.rgba;
        set_ctx_style(ctx, "fillStyle", &format!("rgba({},{},{},{:.3})", rr, gg, bb, alpha))?;
        set_ctx_style(ctx, "strokeStyle", "rgba(255,255,255,0.18)")?;
        ctx.set_line_width(1.0);

        let x = p.pos.x * sx;
        let y = p.pos.y * sy;

        match def.kind {
            ShapeKind::Disk => {
                let rpx = r_eff * pix_scale;
                ctx.begin_path();
                ctx.arc(x, y, rpx, 0.0, TAU)?;
                ctx.fill();
                ctx.stroke();
            }
            _ => {
                let world = def.world_poly(p.pos, p.theta, r_eff).unwrap();
                ctx.begin_path();
                let first = world[0];
                ctx.move_to(first.x * sx, first.y * sy);
                for &v in world.iter().skip(1) {
                    ctx.line_to(v.x * sx, v.y * sy);
                }
                ctx.close_path();
                ctx.fill();
                ctx.stroke();
            }
        }
    }

    // Top-left tiny legend
    set_ctx_style(ctx, "fillStyle", "rgba(255,255,255,0.80)")?;
    ctx.set_font("12px ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace");
    let legend = format!(
        "φ={:.4}  r×{:.2}  hard={:.2}",
        sim.packing_fraction(),
        sim.eff_r_scale(),
        hard
    );
    let _ = ctx.fill_text(&legend, 14.0, ch - 14.0);

    Ok(())
}

fn update_ui(sim: &Sim) {
    let (u, overlap, maxpen) = sim.energy_and_overlap_stats();

    set_text("stat-phi", &format!("{:.4}", sim.packing_fraction()));
    set_text("stat-energy", &format!("{:.3}", u));
    set_text("stat-overlap", &format!("{:.4}", overlap));
    set_text("stat-maxpen", &format!("{:.3}", maxpen));

    let acc_t = if sim.att_t == 0 { 0.0 } else { sim.acc_t as f64 / sim.att_t as f64 };
    let acc_r = if sim.att_r == 0 { 0.0 } else { sim.acc_r as f64 / sim.att_r as f64 };
    let acc_v = if sim.att_v == 0 { 0.0 } else { sim.acc_v as f64 / sim.att_v as f64 };

    set_text("stat-accT", &format!("{:.1}%", acc_t * 100.0));
    set_text("stat-accR", &format!("{:.1}%", acc_r * 100.0));
    set_text("stat-accV", &format!("{:.1}%", acc_v * 100.0));
    set_text("stat-iters", &format!("{}", sim.cfg.iters));

    set_text("stat-status", &sim.status);
}

// ==============================
// Export helpers (text + png)
// ==============================

fn download_text(filename: &str, text: &str) -> Result<(), JsValue> {
    let parts = Array::new();
    parts.push(&JsValue::from_str(text));
    let blob = Blob::new_with_str_sequence(&parts)?;
    let url = Url::create_object_url_with_blob(&blob)?;

    let a = document().create_element("a")?.dyn_into::<HtmlAnchorElement>()?;
    a.set_href(&url);
    a.set_download(filename);
    a.set_attribute("style", "display:none")?;
    document().body().unwrap().append_child(&a)?;
    a.click();
    a.remove();

    Url::revoke_object_url(&url)?;
    Ok(())
}

fn download_canvas_png(canvas: &HtmlCanvasElement, filename: &str) -> Result<(), JsValue> {
    let data_url = canvas.to_data_url_with_type("image/png")?;
    let a = document().create_element("a")?.dyn_into::<HtmlAnchorElement>()?;
    a.set_href(&data_url);
    a.set_download(filename);
    a.set_attribute("style", "display:none")?;
    document().body().unwrap().append_child(&a)?;
    a.click();
    a.remove();
    Ok(())
}

fn build_share_url(sim: &Sim) -> Result<String, JsValue> {
    let loc = window().location();
    let origin = loc.origin()?;
    let path = loc.pathname()?;

    // For mixture, encode weights too.
    let qs = format!(
        "seed={}&n={}&shape={}&mix={}&wd={:.3}&wr={:.3}&wt={:.3}&ws={:.3}&wh={:.3}&wo={:.3}\
&r={:.3}&aspect={:.3}&shrink={}&r0={:.3}&stau={:.0}&hard={}&htau={:.0}\
&step={:.3}&rotstep={:.3}&beta={:.3}&pressure={:.3}&volstep={:.4}&pbc={}&geom={}&iters={}",
        sim.cfg.seed,
        sim.cfg.n,
        sim.cfg.primary_shape.as_str(),
        if sim.cfg.mix_shapes {1} else {0},
        sim.cfg.w_disk, sim.cfg.w_rect, sim.cfg.w_tri, sim.cfg.w_square, sim.cfg.w_hex, sim.cfg.w_oct,
        sim.cfg.r,
        sim.cfg.aspect,
        if sim.cfg.shrink_enabled {1} else {0},
        sim.cfg.r0_mult,
        sim.cfg.shrink_tau,
        if sim.cfg.anneal_hardness {1} else {0},
        sim.cfg.hardness_tau,
        sim.cfg.step_t,
        sim.cfg.step_r,
        sim.cfg.beta,
        sim.cfg.pressure,
        sim.cfg.vol_step,
        if sim.cfg.pbc {1} else {0},
        sim.cfg.geom.as_str(),
        sim.cfg.iters
    );

    Ok(format!("{origin}{path}?{qs}"))
}

fn copy_to_clipboard_best_effort(text: &str) {
    let nav: Navigator = window().navigator();

    // This requires web-sys features: Navigator + Clipboard
    // If unavailable or denied, fail silently.
    #[allow(unused_must_use)]
    {
        nav.clipboard().write_text(text);
    }
}

// ==============================
// Parse config from URL
// ==============================

fn read_config_from_url(canvas_w: u32, canvas_h: u32) -> Config {
    let loc: Location = window().location();
    let search = loc.search().unwrap_or_default();
    let params = UrlSearchParams::new_with_str(&search).unwrap_or_else(|_| UrlSearchParams::new().unwrap());

    let mut cfg = Config::defaults(canvas_w, canvas_h);

    cfg.seed = parse_query_u64(&params, "seed", cfg.seed, 0, u64::MAX);
    cfg.n = parse_query_usize(&params, "n", cfg.n, 1, 6000);

    cfg.primary_shape = ShapeKind::from_str(&parse_query_str(&params, "shape", cfg.primary_shape.as_str()));
    cfg.mix_shapes = parse_query_bool(&params, "mix", cfg.mix_shapes);

    cfg.w_disk = parse_query_f64(&params, "wd", cfg.w_disk, 0.0, 1000.0);
    cfg.w_rect = parse_query_f64(&params, "wr", cfg.w_rect, 0.0, 1000.0);
    cfg.w_tri = parse_query_f64(&params, "wt", cfg.w_tri, 0.0, 1000.0);
    cfg.w_square = parse_query_f64(&params, "ws", cfg.w_square, 0.0, 1000.0);
    cfg.w_hex = parse_query_f64(&params, "wh", cfg.w_hex, 0.0, 1000.0);
    cfg.w_oct = parse_query_f64(&params, "wo", cfg.w_oct, 0.0, 1000.0);

    cfg.r = parse_query_f64(&params, "r", cfg.r, 1.0, 80.0);
    cfg.aspect = parse_query_f64(&params, "aspect", cfg.aspect, 1.0, 10.0);

    cfg.shrink_enabled = parse_query_bool(&params, "shrink", cfg.shrink_enabled);
    cfg.r0_mult = parse_query_f64(&params, "r0", cfg.r0_mult, 1.0, 10.0);
    cfg.shrink_tau = parse_query_f64(&params, "stau", cfg.shrink_tau, 1_000.0, 5_000_000.0);

    cfg.anneal_hardness = parse_query_bool(&params, "hard", cfg.anneal_hardness);
    cfg.hardness_tau = parse_query_f64(&params, "htau", cfg.hardness_tau, 1_000.0, 5_000_000.0);

    cfg.step_t = parse_query_f64(&params, "step", cfg.step_t, 0.0, 250.0);
    cfg.step_r = parse_query_f64(&params, "rotstep", cfg.step_r, 0.0, 2.0);
    cfg.vol_step = parse_query_f64(&params, "volstep", cfg.vol_step, 0.0, 0.25);

    cfg.beta = parse_query_f64(&params, "beta", cfg.beta, 0.01, 80.0);
    cfg.pressure = parse_query_f64(&params, "pressure", cfg.pressure, 0.0, 80.0);

    cfg.pbc = parse_query_bool(&params, "pbc", cfg.pbc);
    cfg.geom = GeometryKind::from_str(&parse_query_str(&params, "geom", cfg.geom.as_str()));

    cfg.iters = parse_query_u64(&params, "iters", 0, 0, 1_000_000_000);

    cfg.clamp();
    cfg
}

// ==============================
// WASM Entry
// ==============================

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    // Canvas
    let canvas = get_canvas("canvas")?;
    let cw = canvas.width().max(900) as f64;
    let ch = canvas.height().max(650) as f64;

    let cfg = read_config_from_url(cw as u32, ch as u32);

    // Populate UI values (best effort)
    if let Some(i) = get_input("input-seed") { i.set_value(&cfg.seed.to_string()); }
    if let Some(i) = get_input("input-n") { i.set_value(&cfg.n.to_string()); }

    // These might be <select>, so use get_select when available
    if let Some(s) = get_select("input-shape") { s.set_value(cfg.primary_shape.as_str()); }
    if let Some(s) = get_select("input-geometry") { s.set_value(cfg.geom.as_str()); }

    if let Some(i) = get_input("input-r") { i.set_value(&format!("{:.3}", cfg.r)); }
    if let Some(i) = get_input("input-aspect") { i.set_value(&format!("{:.3}", cfg.aspect)); }

    if let Some(i) = get_input("input-step") { i.set_value(&format!("{:.3}", cfg.step_t)); }
    if let Some(i) = get_input("input-rotstep") { i.set_value(&format!("{:.3}", cfg.step_r)); }
    if let Some(i) = get_input("input-volstep") { i.set_value(&format!("{:.4}", cfg.vol_step)); }

    if let Some(i) = get_input("input-beta") { i.set_value(&format!("{:.3}", cfg.beta)); }
    if let Some(i) = get_input("input-pressure") { i.set_value(&format!("{:.3}", cfg.pressure)); }

    if let Some(i) = get_input("input-pbc") { i.set_checked(cfg.pbc); }

    // Optional toggles for shrink/hardness/mix
    if let Some(i) = get_input("input-shrink") { i.set_checked(cfg.shrink_enabled); }
    if let Some(i) = get_input("input-hard") { i.set_checked(cfg.anneal_hardness); }
    if let Some(i) = get_input("input-mix") { i.set_checked(cfg.mix_shapes); }

    // Optional weights
    if let Some(i) = get_input("input-wdisk") { i.set_value(&format!("{:.3}", cfg.w_disk)); }
    if let Some(i) = get_input("input-wrect") { i.set_value(&format!("{:.3}", cfg.w_rect)); }
    if let Some(i) = get_input("input-wtri") { i.set_value(&format!("{:.3}", cfg.w_tri)); }
    if let Some(i) = get_input("input-wsquare") { i.set_value(&format!("{:.3}", cfg.w_square)); }
    if let Some(i) = get_input("input-whex") { i.set_value(&format!("{:.3}", cfg.w_hex)); }
    if let Some(i) = get_input("input-woct") { i.set_value(&format!("{:.3}", cfg.w_oct)); }

    // Optional shrink params
    if let Some(i) = get_input("input-r0") { i.set_value(&format!("{:.3}", cfg.r0_mult)); }
    if let Some(i) = get_input("input-stau") { i.set_value(&format!("{:.0}", cfg.shrink_tau)); }
    if let Some(i) = get_input("input-htau") { i.set_value(&format!("{:.0}", cfg.hardness_tau)); }

    // Context
    let ctx = canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("No 2d context"))?
        .dyn_into::<CanvasRenderingContext2d>()?;

    let sim = Rc::new(RefCell::new(Sim::new(cfg)));

    // Replay from URL (bounded)
    {
        let mut s = sim.borrow_mut();
        let requested = s.cfg.iters;
        if requested > 0 {
            s.cfg.iters = 0;
            let cap = 300_000u64;
            let run = requested.min(cap);
            let mut rem = run;
            while rem > 0 {
                let chunk = rem.min(20_000) as u32;
                s.mc_sweep(chunk);
                rem -= chunk as u64;
            }
            s.set_status(format!("Replayed {} moves from URL (cap {}).", run, cap));
        }
    }

    update_ui(&sim.borrow());
    render(&ctx, cw, ch, &sim.borrow())?;

    // Apply/reset handler
    if let Some(btn) = get_button("btn-apply") {
        let sim = sim.clone();
        let cb = Closure::<dyn FnMut()>::new(move || {
            let mut s = sim.borrow_mut();

            // Read values from either <input> or <select>
            let seed = get_value("input-seed").and_then(|v| v.parse::<u64>().ok()).unwrap_or(s.cfg.seed);
            let n = get_value("input-n").and_then(|v| v.parse::<usize>().ok()).unwrap_or(s.cfg.n);

            let shape = get_value("input-shape")
                .map(|v| ShapeKind::from_str(&v))
                .unwrap_or(s.cfg.primary_shape);

            let geom = get_value("input-geometry")
                .map(|v| GeometryKind::from_str(&v))
                .unwrap_or(s.cfg.geom);

            let r = get_value("input-r").and_then(|v| v.parse::<f64>().ok()).unwrap_or(s.cfg.r);
            let aspect = get_value("input-aspect").and_then(|v| v.parse::<f64>().ok()).unwrap_or(s.cfg.aspect);

            let step = get_value("input-step").and_then(|v| v.parse::<f64>().ok()).unwrap_or(s.cfg.step_t);
            let rotstep = get_value("input-rotstep").and_then(|v| v.parse::<f64>().ok()).unwrap_or(s.cfg.step_r);
            let volstep = get_value("input-volstep").and_then(|v| v.parse::<f64>().ok()).unwrap_or(s.cfg.vol_step);

            let beta = get_value("input-beta").and_then(|v| v.parse::<f64>().ok()).unwrap_or(s.cfg.beta);
            let pressure = get_value("input-pressure").and_then(|v| v.parse::<f64>().ok()).unwrap_or(s.cfg.pressure);

            let pbc = get_checked("input-pbc").unwrap_or(s.cfg.pbc);

            let shrink = get_checked("input-shrink").unwrap_or(s.cfg.shrink_enabled);
            let hard = get_checked("input-hard").unwrap_or(s.cfg.anneal_hardness);
            let mix = get_checked("input-mix").unwrap_or(s.cfg.mix_shapes);

            let r0 = get_value("input-r0").and_then(|v| v.parse::<f64>().ok()).unwrap_or(s.cfg.r0_mult);
            let stau = get_value("input-stau").and_then(|v| v.parse::<f64>().ok()).unwrap_or(s.cfg.shrink_tau);
            let htau = get_value("input-htau").and_then(|v| v.parse::<f64>().ok()).unwrap_or(s.cfg.hardness_tau);

            let wd = get_value("input-wdisk").and_then(|v| v.parse::<f64>().ok()).unwrap_or(s.cfg.w_disk);
            let wr = get_value("input-wrect").and_then(|v| v.parse::<f64>().ok()).unwrap_or(s.cfg.w_rect);
            let wt = get_value("input-wtri").and_then(|v| v.parse::<f64>().ok()).unwrap_or(s.cfg.w_tri);
            let wsq = get_value("input-wsquare").and_then(|v| v.parse::<f64>().ok()).unwrap_or(s.cfg.w_square);
            let wh = get_value("input-whex").and_then(|v| v.parse::<f64>().ok()).unwrap_or(s.cfg.w_hex);
            let wo = get_value("input-woct").and_then(|v| v.parse::<f64>().ok()).unwrap_or(s.cfg.w_oct);

            s.cfg.seed = seed;
            s.cfg.n = n;

            s.cfg.primary_shape = shape;
            s.cfg.geom = geom;

            s.cfg.r = r;
            s.cfg.aspect = aspect;

            s.cfg.step_t = step;
            s.cfg.step_r = rotstep;
            s.cfg.vol_step = volstep;

            s.cfg.beta = beta;
            s.cfg.pressure = pressure;

            s.cfg.pbc = pbc;

            s.cfg.shrink_enabled = shrink;
            s.cfg.anneal_hardness = hard;
            s.cfg.mix_shapes = mix;

            s.cfg.r0_mult = r0;
            s.cfg.shrink_tau = stau;
            s.cfg.hardness_tau = htau;

            s.cfg.w_disk = wd;
            s.cfg.w_rect = wr;
            s.cfg.w_tri = wt;
            s.cfg.w_square = wsq;
            s.cfg.w_hex = wh;
            s.cfg.w_oct = wo;

            s.cfg.clamp();
            s.rebuild_shapes();
            s.reset();

            s.set_status("Applied settings and reset.");
            update_ui(&s);
        });
        btn.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref()).ok();
        cb.forget();
    }

    // Run/pause
    if let Some(btn) = get_button("btn-run") {
        let sim = sim.clone();
        let cb = Closure::<dyn FnMut()>::new(move || {
            let mut s = sim.borrow_mut();
            s.running = !s.running;

            if let Some(b) = get_button("btn-run") {
                b.set_text_content(Some(if s.running { "Pause" } else { "Run" }));
            }

            let msg = if s.running { "Running." } else { "Paused." };
            s.set_status(msg);
            update_ui(&s);
        });
        btn.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref()).ok();
        cb.forget();
    }

    // Step
    if let Some(btn) = get_button("btn-step") {
        let sim = sim.clone();
        let cb = Closure::<dyn FnMut()>::new(move || {
            let mut s = sim.borrow_mut();
            s.mc_sweep(25_000);
            s.set_status("Stepped 25k moves.");
            update_ui(&s);
        });
        btn.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref()).ok();
        cb.forget();
    }

    // Copy link
    if let Some(btn) = get_button("btn-copylink") {
        let sim = sim.clone();
        let cb = Closure::<dyn FnMut()>::new(move || {
            let s = sim.borrow();
            match build_share_url(&s) {
                Ok(url) => {
                    copy_to_clipboard_best_effort(&url);
                    set_text("stat-status", "Copied share URL (or attempted).");
                }
                Err(_) => set_text("stat-status", "Failed to build share URL."),
            }
        });
        btn.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref()).ok();
        cb.forget();
    }

    // Export PNG
    if let Some(btn) = get_button("btn-png") {
        let canvas = canvas.clone();
        let cb = Closure::<dyn FnMut()>::new(move || { let _ = download_canvas_png(&canvas, "mc_packing.png"); });
        btn.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref()).ok();
        cb.forget();
    }

    // Export CSV
    if let Some(btn) = get_button("btn-csv") {
        let sim = sim.clone();
        let cb = Closure::<dyn FnMut()>::new(move || {
            let s = sim.borrow();
            let (u, overlap, maxpen) = s.energy_and_overlap_stats();
            let mut out = String::new();

            out.push_str("# MC packing export\n");
            out.push_str(&format!(
                "# seed={}, geom={}, pbc={}, mix_shapes={}, iters={}\n",
                s.cfg.seed, s.cfg.geom.as_str(), s.cfg.pbc as u8, s.cfg.mix_shapes as u8, s.cfg.iters
            ));
            out.push_str(&format!(
                "# r_final={:.6}, r0_mult={:.3}, shrink_tau={:.0}, hardness_tau={:.0}\n",
                s.cfg.r, s.cfg.r0_mult, s.cfg.shrink_tau, s.cfg.hardness_tau
            ));
            out.push_str(&format!(
                "# beta={:.6}, pressure={:.6}, phi={:.8}, energy={:.8}, overlap_frac={:.8}, max_pen={:.6}\n",
                s.cfg.beta, s.cfg.pressure, s.packing_fraction(), u, overlap, maxpen
            ));

            out.push_str("i,shape_id,shape,x,y,theta,r_eff\n");
            for (i, p) in s.parts.iter().enumerate() {
                let def = &s.shapes[p.shape_id];
                out.push_str(&format!(
                    "{},{},{},{:.6},{:.6},{:.6},{:.6}\n",
                    i, p.shape_id, def.kind.as_str(), p.pos.x, p.pos.y, p.theta, s.eff_r(p)
                ));
            }

            let _ = download_text("mc_packing.csv", &out);
        });
        btn.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref()).ok();
        cb.forget();
    }

    // Export JSON (lightweight)
    if let Some(btn) = get_button("btn-json") {
        let sim = sim.clone();
        let cb = Closure::<dyn FnMut()>::new(move || {
            let s = sim.borrow();
            let (u, overlap, maxpen) = s.energy_and_overlap_stats();

            let mut parts = String::from("[");
            for (i, p) in s.parts.iter().enumerate() {
                if i > 0 { parts.push(','); }
                parts.push_str(&format!(
                    r#"{{"x":{:.6},"y":{:.6},"theta":{:.6},"shape":"{}","shape_id":{}}}"#,
                    p.pos.x, p.pos.y, p.theta, s.shapes[p.shape_id].kind.as_str(), p.shape_id
                ));
            }
            parts.push(']');

            let json = format!(
                r#"{{
  "config": {{
    "seed": {},
    "n": {},
    "geom": "{}",
    "pbc": {},
    "primary_shape": "{}",
    "mix_shapes": {},
    "weights": {{"disk":{:.3},"rect":{:.3},"tri":{:.3},"square":{:.3},"hex":{:.3},"oct":{:.3}}},
    "r_final": {:.6},
    "aspect": {:.6},
    "shrink_enabled": {},
    "r0_mult": {:.6},
    "shrink_tau": {:.0},
    "hardness_anneal": {},
    "hardness_tau": {:.0},
    "beta": {:.6},
    "pressure": {:.6},
    "step_t": {:.6},
    "step_r": {:.6},
    "vol_step": {:.6},
    "iters": {}
  }},
  "metrics": {{
    "phi": {:.10},
    "energy": {:.10},
    "overlap_fraction": {:.10},
    "max_penetration": {:.10}
  }},
  "state": {{
    "particles": {}
  }}
}}"#,
                s.cfg.seed,
                s.cfg.n,
                s.cfg.geom.as_str(),
                if s.cfg.pbc {1} else {0},
                s.cfg.primary_shape.as_str(),
                if s.cfg.mix_shapes {true} else {false},
                s.cfg.w_disk, s.cfg.w_rect, s.cfg.w_tri, s.cfg.w_square, s.cfg.w_hex, s.cfg.w_oct,
                s.cfg.r,
                s.cfg.aspect,
                if s.cfg.shrink_enabled {true} else {false},
                s.cfg.r0_mult,
                s.cfg.shrink_tau,
                if s.cfg.anneal_hardness {true} else {false},
                s.cfg.hardness_tau,
                s.cfg.beta,
                s.cfg.pressure,
                s.cfg.step_t,
                s.cfg.step_r,
                s.cfg.vol_step,
                s.cfg.iters,
                s.packing_fraction(),
                u,
                overlap,
                maxpen,
                parts
            );

            let _ = download_text("mc_packing.json", &json);
        });
        btn.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref()).ok();
        cb.forget();
    }

    // RAF loop
    let raf: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let raf_handle = raf.clone();
    let sim_render = sim.clone();

    *raf_handle.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        // pause work when hidden
        if !document().hidden() {
            {
                let mut s = sim_render.borrow_mut();
                if s.running {
                    s.mc_sweep(2_500);
                }
                update_ui(&s);
                let _ = render(&ctx, cw, ch, &s);
            }
        }

        let _ = window().request_animation_frame(
            raf.borrow().as_ref().unwrap().as_ref().unchecked_ref()
        );
    }) as Box<dyn FnMut()>));

    window().request_animation_frame(
        raf_handle.borrow().as_ref().unwrap().as_ref().unchecked_ref()
    )?;

    Ok(())
}
