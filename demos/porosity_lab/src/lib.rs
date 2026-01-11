use std::cell::RefCell;
use std::rc::Rc;

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;

use wasm_bindgen::prelude::*;
use wasm_bindgen::Clamped;
use wasm_bindgen::JsCast;

use web_sys::{
    CanvasRenderingContext2d, Document, Event, HtmlAnchorElement, HtmlCanvasElement, HtmlInputElement,
    ImageData, Window,
};

const W: usize = 400;
const H: usize = 300;

struct PorositySim {
    grid: Vec<u8>,
    labels: Vec<u8>,
    target_p: f64,
    scale: f64,
    seed: u64,
}

impl PorositySim {
    fn new() -> Self {
        let mut s = Self {
            grid: vec![0; W * H],
            labels: vec![0; W * H],
            target_p: 0.40,
            scale: 20.0,
            seed: 42,
        };
        s.generate();
        s
    }

    fn reset_defaults(&mut self) {
        self.target_p = 0.40;
        self.scale = 20.0;
        self.seed = 42;
        self.generate();
    }

    fn generate(&mut self) {
        let mut rng = ChaCha8Rng::seed_from_u64(self.seed);

        let low_w = (W as f64 / self.scale).ceil() as usize;
        let low_h = (H as f64 / self.scale).ceil() as usize;

        let mut low_grid = vec![0.0; low_w * low_h];
        for v in &mut low_grid {
            *v = rng.gen::<f64>();
        }

        for y in 0..H {
            for x in 0..W {
                let gx = x as f64 / self.scale;
                let gy = y as f64 / self.scale;

                let x0 = gx.floor() as usize;
                let y0 = gy.floor() as usize;
                let x1 = (x0 + 1).min(low_w - 1);
                let y1 = (y0 + 1).min(low_h - 1);

                let tx = gx - x0 as f64;
                let ty = gy - y0 as f64;

                let v00 = low_grid[y0 * low_w + x0];
                let v10 = low_grid[y0 * low_w + x1];
                let v01 = low_grid[y1 * low_w + x0];
                let v11 = low_grid[y1 * low_w + x1];

                let val = (v00 * (1.0 - tx) + v10 * tx) * (1.0 - ty)
                    + (v01 * (1.0 - tx) + v11 * tx) * ty;

                self.grid[y * W + x] = if val < self.target_p { 1 } else { 0 };
            }
        }

        self.analyze();
    }

    fn analyze(&mut self) {
        self.labels.fill(0);
        let mut stack: Vec<(usize, usize)> = Vec::new();

        // "Accessible" = connected to top boundary (toy model)
        for x in 0..W {
            let idx = x; // y=0
            if self.grid[idx] == 1 {
                stack.push((x, 0));
                self.labels[idx] = 1;
            }
        }

        while let Some((cx, cy)) = stack.pop() {
            let neighbors = [
                (cx.wrapping_sub(1), cy),
                (cx + 1, cy),
                (cx, cy.wrapping_sub(1)),
                (cx, cy + 1),
            ];

            for (nx, ny) in neighbors {
                if nx < W && ny < H {
                    let idx = ny * W + nx;
                    if self.grid[idx] == 1 && self.labels[idx] == 0 {
                        self.labels[idx] = 1;
                        stack.push((nx, ny));
                    }
                }
            }
        }

        // Any void not marked accessible is isolated
        for i in 0..W * H {
            if self.grid[i] == 1 && self.labels[i] == 0 {
                self.labels[i] = 2;
            }
        }
    }

    fn metrics(&self) -> (f64, f64, f64) {
        let total = (W * H) as f64;
        let mut v = 0.0;
        let mut a = 0.0;
        let mut iso = 0.0;

        for i in 0..W * H {
            if self.grid[i] == 1 {
                v += 1.0;
                if self.labels[i] == 1 {
                    a += 1.0;
                } else {
                    iso += 1.0;
                }
            }
        }

        (v / total, a / total, iso / total)
    }
}

fn window() -> Window {
    web_sys::window().expect("no global `window` exists")
}

fn document() -> Document {
    window().document().expect("should have a document on window")
}

fn set_text(doc: &Document, id: &str, text: &str) {
    if let Some(el) = doc.get_element_by_id(id) {
        el.set_text_content(Some(text));
    }
}

fn set_slider_value(doc: &Document, id: &str, v: &str) {
    if let Some(el) = doc.get_element_by_id(id) {
        if let Ok(input) = el.dyn_into::<HtmlInputElement>() {
            input.set_value(v);
        }
    }
}

/// Hide loader: calls window.setWasmReady() if present.
fn set_wasm_ready() {
    let w = window();
    if let Ok(f) = js_sys::Reflect::get(&w, &JsValue::from_str("setWasmReady")) {
        if f.is_function() {
            let _ = js_sys::Function::from(f).call0(&JsValue::NULL);
        }
    }
}

/// Skip expensive redraws when tab is hidden.
fn document_visible() -> bool {
    web_sys::window()
        .and_then(|w| w.document())
        .map(|d| !d.hidden())
        .unwrap_or(true)
}

fn render(
    ctx: &CanvasRenderingContext2d,
    px: &mut [u8],
    sim: &PorositySim,
) -> Result<(), JsValue> {
    for i in 0..W * H {
        let idx = i * 4;
        match sim.labels[i] {
            // accessible void: blue
            1 => {
                px[idx] = 100;
                px[idx + 1] = 120;
                px[idx + 2] = 255;
                px[idx + 3] = 255;
            }
            // isolated void: red
            2 => {
                px[idx] = 255;
                px[idx + 1] = 120;
                px[idx + 2] = 120;
                px[idx + 3] = 255;
            }
            // solid/background
            _ => {
                let v = if sim.grid[i] == 0 { 54 } else { 0 };
                px[idx] = v;
                px[idx + 1] = v;
                px[idx + 2] = v;
                px[idx + 3] = 255;
            }
        }
    }

    // ✅ ImageData expects Clamped<&[u8]>
    let clamped = Clamped(&px[..]);
    let img = ImageData::new_with_u8_clamped_array_and_sh(clamped, W as u32, H as u32)?;
    ctx.put_image_data(&img, 0.0, 0.0)?;
    Ok(())
}

fn snapshot_png(canvas: &HtmlCanvasElement) {
    if let Ok(url) = canvas.to_data_url_with_type("image/png") {
        let doc = document();
        if let Ok(a) = doc.create_element("a") {
            if let Ok(a) = a.dyn_into::<HtmlAnchorElement>() {
                a.set_href(&url);
                a.set_download("porosity_lab.png");
                let _ = a.click();
            }
        }
    }
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let doc = document();

    // Canvas
    let canvas = doc
        .get_element_by_id("canvas")
        .ok_or_else(|| JsValue::from_str("Missing #canvas"))?
        .dyn_into::<HtmlCanvasElement>()?;

    canvas.set_width(W as u32);
    canvas.set_height(H as u32);

    let ctx = canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("No 2D context"))?
        .dyn_into::<CanvasRenderingContext2d>()?;

    // State + pixel buffer
    let sim = Rc::new(RefCell::new(PorositySim::new()));
    let px = Rc::new(RefCell::new(vec![0u8; W * H * 4]));

    // Dirty flag: we only redraw when something changes
    let dirty = Rc::new(RefCell::new(true));

    // Shared redraw closure (updates metrics + draws if dirty and visible)
    let redraw = {
        let doc = doc.clone();
        let sim = sim.clone();
        let px = px.clone();
        let ctx = ctx.clone();
        let dirty = dirty.clone();

        Rc::new(move || {
            // Always update metrics if something changed
            if !*dirty.borrow() {
                return;
            }
            *dirty.borrow_mut() = false;

            let s = sim.borrow();
            let (tot, acc, iso) = s.metrics();
            set_text(&doc, "stat-total", &format!("{:.1}%", tot * 100.0));
            set_text(&doc, "stat-access", &format!("{:.1}%", acc * 100.0));
            set_text(&doc, "stat-iso", &format!("{:.1}%", iso * 100.0));

            if !document_visible() {
                return;
            }

            let mut buf = px.borrow_mut();
            let _ = render(&ctx, &mut buf, &s);
        })
    };

    // Initialise UI values (match HTML defaults)
    set_text(&doc, "val-p", "0.40");
    set_text(&doc, "val-s", "20");
    set_slider_value(&doc, "input-p", "0.40");
    set_slider_value(&doc, "input-s", "20");

    // First draw
    {
        *dirty.borrow_mut() = true;
        redraw();
    }

    // Bind: target porosity slider
    {
        let doc = doc.clone();
        let sim = sim.clone();
        let dirty = dirty.clone();
        let redraw = redraw.clone();

        let el = doc
            .get_element_by_id("input-p")
            .ok_or_else(|| JsValue::from_str("Missing #input-p"))?
            .dyn_into::<HtmlInputElement>()?;

        let cb = Closure::wrap(Box::new(move |e: Event| {
            let t = e
                .target()
                .unwrap()
                .dyn_into::<HtmlInputElement>()
                .unwrap();

            let v = t.value().parse::<f64>().unwrap_or(0.40);
            set_text(&doc, "val-p", &format!("{:.2}", v));

            {
                let mut s = sim.borrow_mut();
                s.target_p = v;
                s.generate();
            }

            *dirty.borrow_mut() = true;
            redraw();
        }) as Box<dyn FnMut(_)>);

        el.add_event_listener_with_callback("input", cb.as_ref().unchecked_ref())?;
        cb.forget();
    }

    // Bind: scale slider
    {
        let doc = doc.clone();
        let sim = sim.clone();
        let dirty = dirty.clone();
        let redraw = redraw.clone();

        let el = doc
            .get_element_by_id("input-s")
            .ok_or_else(|| JsValue::from_str("Missing #input-s"))?
            .dyn_into::<HtmlInputElement>()?;

        let cb = Closure::wrap(Box::new(move |e: Event| {
            let t = e
                .target()
                .unwrap()
                .dyn_into::<HtmlInputElement>()
                .unwrap();

            let v = t.value().parse::<f64>().unwrap_or(20.0);
            set_text(&doc, "val-s", &format!("{:.0}", v));

            {
                let mut s = sim.borrow_mut();
                s.scale = v;
                s.generate();
            }

            *dirty.borrow_mut() = true;
            redraw();
        }) as Box<dyn FnMut(_)>);

        el.add_event_listener_with_callback("input", cb.as_ref().unchecked_ref())?;
        cb.forget();
    }

    // Bind: Regenerate
    {
        let sim = sim.clone();
        let dirty = dirty.clone();
        let redraw = redraw.clone();

        let btn = doc
            .get_element_by_id("regen")
            .ok_or_else(|| JsValue::from_str("Missing #regen"))?;

        let cb = Closure::wrap(Box::new(move || {
            {
                let mut s = sim.borrow_mut();
                s.seed = rand::thread_rng().next_u64();
                s.generate();
            }
            *dirty.borrow_mut() = true;
            redraw();
        }) as Box<dyn FnMut()>);

        btn.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref())?;
        cb.forget();
    }

    // Bind: Reset defaults
    {
        let doc = doc.clone();
        let sim = sim.clone();
        let dirty = dirty.clone();
        let redraw = redraw.clone();

        let btn = doc
            .get_element_by_id("btn-reset")
            .ok_or_else(|| JsValue::from_str("Missing #btn-reset"))?;

        let cb = Closure::wrap(Box::new(move || {
            {
                let mut s = sim.borrow_mut();
                s.reset_defaults();
            }

            // reflect defaults in UI
            set_text(&doc, "val-p", "0.40");
            set_text(&doc, "val-s", "20");
            set_slider_value(&doc, "input-p", "0.40");
            set_slider_value(&doc, "input-s", "20");

            *dirty.borrow_mut() = true;
            redraw();
        }) as Box<dyn FnMut()>);

        btn.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref())?;
        cb.forget();
    }

    // Bind: Snapshot
    {
        let canvas = canvas.clone();
        let btn = doc
            .get_element_by_id("btn-snapshot")
            .ok_or_else(|| JsValue::from_str("Missing #btn-snapshot"))?;

        let cb = Closure::wrap(Box::new(move || {
            snapshot_png(&canvas);
        }) as Box<dyn FnMut()>);

        btn.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref())?;
        cb.forget();
    }

    // Hide loader now that everything is wired
    set_wasm_ready();

    Ok(())
}