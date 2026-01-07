use std::cell::RefCell; use std::rc::Rc; use wasm_bindgen::prelude::*; use wasm_bindgen::Clamped; use wasm_bindgen::JsCast; use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, MouseEvent, ImageData}; use rand::prelude::*;
const W:usize=300; const H:usize=300; const Q:u8=64;
struct GrainSim{grid:Vec<u8>,colors:Vec<(u8,u8,u8)>}
impl GrainSim{
    fn new()->Self{let mut r=SmallRng::from_entropy();let g=(0..W*H).map(|_|r.gen_range(0..Q)).collect();let c=(0..Q).map(|_| (r.gen_range(50..255),r.gen_range(50..255),r.gen_range(50..255))).collect();Self{grid:g,colors:c}}
    fn update(&mut self){let mut r=SmallRng::from_entropy();for _ in 0..W*H{let i=r.gen_range(0..W*H);let x=i%W;let y=i/W;let d=r.gen_range(0..4);let n=match d{0=>if x>0{i-1}else{i},1=>if x<W-1{i+1}else{i},2=>if y>0{i-W}else{i},_=>if y<H-1{i+W}else{i}};if i==n{continue;}let s1=self.grid[i];let s2=self.grid[n];if s1==s2{continue;}if self.en(i,s2)<=self.en(i,s1){self.grid[i]=s2;}else if r.gen_bool(0.1){self.grid[i]=s2;}}}
    fn en(&self,i:usize,s:u8)->u8{let x=i%W;let y=i/W;let mut e=0;if x>0&&self.grid[i-1]!=s{e+=1;}if x<W-1&&self.grid[i+1]!=s{e+=1;}if y>0&&self.grid[i-W]!=s{e+=1;}if y<H-1&&self.grid[i+W]!=s{e+=1;}e}
}
#[wasm_bindgen(start)]
pub fn start()->Result<(),JsValue>{
    console_error_panic_hook::set_once(); let w=web_sys::window().unwrap(); let d=w.document().unwrap();
    let c=d.get_element_by_id("canvas").unwrap().dyn_into::<HtmlCanvasElement>()?; c.set_width(W as u32); c.set_height(H as u32);
    let ctx=c.get_context("2d")?.unwrap().dyn_into::<CanvasRenderingContext2d>()?;
    let sim=Rc::new(RefCell::new(GrainSim::new()));
    
    // Explicit Clone for Closure
    {
        let s=sim.clone(); 
        let c_clone = c.clone(); 
        let cl=Closure::wrap(Box::new(move|e:MouseEvent|{
            if e.buttons()==1{
                let rect=c_clone.get_bounding_client_rect();
                let x=((e.client_x() as f64-rect.left())*(W as f64/rect.width())) as usize;
                let y=((e.client_y() as f64-rect.top())*(H as f64/rect.height())) as usize;
                if x<W&&y<H{
                    let mut sm=s.borrow_mut();
                    let n=rand::thread_rng().gen_range(0..Q);
                    for dy in 0..5{for dx in 0..5{let i=(y+dy)*W+(x+dx);if i<W*H{sm.grid[i]=n;}}}
                }
            }
        }) as Box<dyn FnMut(_)>);
        c.add_event_listener_with_callback("mousedown",cl.as_ref().unchecked_ref())?; 
        c.add_event_listener_with_callback("mousemove",cl.as_ref().unchecked_ref())?; 
        cl.forget();
    }
    
    {let s=sim.clone();let b=d.get_element_by_id("reset").unwrap();let cb=Closure::wrap(Box::new(move||{*s.borrow_mut()=GrainSim::new();}) as Box<dyn FnMut()>);b.add_event_listener_with_callback("click",cb.as_ref().unchecked_ref())?;cb.forget();}
    
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g=f.clone();let mut px=vec![0u8;W*H*4];
    
    *g.borrow_mut()=Some(Closure::wrap(Box::new(move||{
        let mut s=sim.borrow_mut();s.update();
        for i in 0..W*H{let (r,g,b)=s.colors[s.grid[i] as usize];let p=i*4;px[p]=r;px[p+1]=g;px[p+2]=b;px[p+3]=255;}
        let d=Clamped(&px[..]);let img=ImageData::new_with_u8_clamped_array_and_sh(d,W as u32,H as u32).unwrap();
        ctx.put_image_data(&img,0.,0.).unwrap();
        request_animation_frame(f.borrow().as_ref().unwrap());
    }) as Box<dyn FnMut()>));
    request_animation_frame(g.borrow().as_ref().unwrap()); Ok(())
}
fn request_animation_frame(f:&Closure<dyn FnMut()>){web_sys::window().unwrap().request_animation_frame(f.as_ref().unchecked_ref()).unwrap();}