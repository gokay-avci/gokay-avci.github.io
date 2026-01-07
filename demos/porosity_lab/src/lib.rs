use std::cell::RefCell; use std::rc::Rc; use wasm_bindgen::prelude::*; use wasm_bindgen::Clamped; use wasm_bindgen::JsCast; use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlInputElement, Event, ImageData}; use rand::prelude::*; use rand_chacha::ChaCha8Rng;
const W: usize = 400; const H: usize = 300;
struct PorositySim { grid: Vec<u8>, labels: Vec<u8>, target_p: f64, scale: f64, seed: u64 }
impl PorositySim {
    fn new() -> Self { let mut s=Self{grid:vec![0;W*H],labels:vec![0;W*H],target_p:0.40,scale:20.0,seed:42}; s.generate(); s }
    fn generate(&mut self) {
        let mut rng = ChaCha8Rng::seed_from_u64(self.seed);
        let low_w=(W as f64/self.scale).ceil() as usize; let low_h=(H as f64/self.scale).ceil() as usize;
        let mut low_grid=vec![0.0; low_w*low_h]; for i in 0..low_grid.len(){low_grid[i]=rng.gen::<f64>();}
        for y in 0..H { for x in 0..W {
            let gx=x as f64/self.scale; let gy=y as f64/self.scale;
            let x0=gx.floor() as usize; let y0=gy.floor() as usize;
            let x1=(x0+1).min(low_w-1); let y1=(y0+1).min(low_h-1);
            let tx=gx-x0 as f64; let ty=gy-y0 as f64;
            let v00=low_grid[y0*low_w+x0]; let v10=low_grid[y0*low_w+x1];
            let v01=low_grid[y1*low_w+x0]; let v11=low_grid[y1*low_w+x1];
            let val=(v00*(1.0-tx)+v10*tx)*(1.0-ty)+(v01*(1.0-tx)+v11*tx)*ty;
            self.grid[y*W+x]=if val<self.target_p{1}else{0};
        }}
        self.analyze();
    }
    fn analyze(&mut self) {
        self.labels.fill(0);
        let mut stack: Vec<(usize, usize)> = Vec::new();
        for x in 0..W { if self.grid[x]==1 { stack.push((x,0)); self.labels[x]=1; } }
        while let Some((cx,cy))=stack.pop() {
            let neighbors=[(cx.wrapping_sub(1),cy),(cx+1,cy),(cx,cy.wrapping_sub(1)),(cx,cy+1)];
            for (nx,ny) in neighbors {
                if nx<W && ny<H {
                    let idx=ny*W+nx;
                    if self.grid[idx]==1 && self.labels[idx]==0 { self.labels[idx]=1; stack.push((nx,ny)); }
                }
            }
        }
        for i in 0..W*H { if self.grid[i]==1 && self.labels[i]==0 { self.labels[i]=2; } }
    }
    fn get_metrics(&self)->(f64,f64,f64){
        let total=(W*H) as f64; let mut v=0.; let mut a=0.; let mut i=0.;
        for idx in 0..W*H { if self.grid[idx]==1 { v+=1.; if self.labels[idx]==1 { a+=1.; } else { i+=1.; } } }
        (v/total, a/total, i/total)
    }
}
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let w=web_sys::window().unwrap(); let d=w.document().unwrap();
    let c=d.get_element_by_id("canvas").unwrap().dyn_into::<HtmlCanvasElement>()?;
    c.set_width(W as u32); c.set_height(H as u32);
    let ctx=c.get_context("2d")?.unwrap().dyn_into::<CanvasRenderingContext2d>()?;
    let sim=Rc::new(RefCell::new(PorositySim::new()));
    let update_ui={let s=sim.clone(); let d=d.clone(); Rc::new(move||{
        let sm=s.borrow(); let (tot,acc,iso)=sm.get_metrics();
        d.get_element_by_id("stat-total").unwrap().set_inner_html(&format!("{:.1}%",tot*100.));
        d.get_element_by_id("stat-access").unwrap().set_inner_html(&format!("{:.1}%",acc*100.));
        d.get_element_by_id("stat-iso").unwrap().set_inner_html(&format!("{:.1}%",iso*100.));
    })};
    update_ui();
    // Bindings
    {
        let s=sim.clone(); let u=update_ui.clone(); let d=d.clone();
        let el=d.get_element_by_id("input-p").unwrap().dyn_into::<HtmlInputElement>().unwrap();
        let cb=Closure::wrap(Box::new(move|e:Event|{
            let t=e.target().unwrap().dyn_into::<HtmlInputElement>().unwrap();
            let v=t.value().parse::<f64>().unwrap();
            let txt=format!("{:.2}",v);
            d.get_element_by_id("val-p").unwrap().set_inner_html(&txt);
            {let mut sm=s.borrow_mut(); sm.target_p=v; sm.generate();} u();
        }) as Box<dyn FnMut(_)>);
        el.add_event_listener_with_callback("input",cb.as_ref().unchecked_ref()).unwrap(); cb.forget();
    }
    {
        let s=sim.clone(); let u=update_ui.clone(); let d=d.clone();
        let el=d.get_element_by_id("input-s").unwrap().dyn_into::<HtmlInputElement>().unwrap();
        let cb=Closure::wrap(Box::new(move|e:Event|{
            let t=e.target().unwrap().dyn_into::<HtmlInputElement>().unwrap();
            let v=t.value().parse::<f64>().unwrap();
            let txt=format!("{:.0}",v);
            d.get_element_by_id("val-s").unwrap().set_inner_html(&txt);
            {let mut sm=s.borrow_mut(); sm.scale=v; sm.generate();} u();
        }) as Box<dyn FnMut(_)>);
        el.add_event_listener_with_callback("input",cb.as_ref().unchecked_ref()).unwrap(); cb.forget();
    }
    {
        let s=sim.clone(); let u=update_ui.clone(); let d=d.clone();
        let btn=d.get_element_by_id("regen").unwrap();
        let cb=Closure::wrap(Box::new(move||{
            {let mut sm=s.borrow_mut(); sm.seed=rand::thread_rng().next_u64(); sm.generate();} u();
        }) as Box<dyn FnMut()>);
        btn.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref()).unwrap();
        cb.forget();
    }
    let f=Rc::new(RefCell::new(None)); let g=f.clone(); let mut px=vec![0u8;W*H*4];
    let sim_render = sim.clone();
    *g.borrow_mut()=Some(Closure::wrap(Box::new(move||{
        let s=sim_render.borrow();
        for i in 0..W*H {
            let idx=i*4;
            match s.labels[i] {
                1 => { px[idx]=100; px[idx+1]=100; px[idx+2]=255; px[idx+3]=255; },
                2 => { px[idx]=255; px[idx+1]=100; px[idx+2]=100; px[idx+3]=255; },
                _ => { let v=if s.grid[i]==0{50}else{0}; px[idx]=v; px[idx+1]=v; px[idx+2]=v; px[idx+3]=255; }
            }
        }
        let d=Clamped(&px[..]); let img=ImageData::new_with_u8_clamped_array_and_sh(d,W as u32,H as u32).unwrap();
        ctx.put_image_data(&img,0.,0.).unwrap();
        request_animation_frame(f.borrow().as_ref().unwrap());
    }) as Box<dyn FnMut()>));
    request_animation_frame(g.borrow().as_ref().unwrap()); Ok(())
}
fn request_animation_frame(f:&Closure<dyn FnMut()>){web_sys::window().unwrap().request_animation_frame(f.as_ref().unchecked_ref()).unwrap();}