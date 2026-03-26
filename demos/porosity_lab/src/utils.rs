use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::{Document, Window};

pub fn window() -> Result<Window, JsValue> {
    web_sys::window().ok_or_else(|| JsValue::from_str("window unavailable"))
}

pub fn document() -> Result<Document, JsValue> {
    window()?
        .document()
        .ok_or_else(|| JsValue::from_str("document unavailable"))
}

pub fn set_text(document: &Document, id: &str, text: &str) {
    if let Some(element) = document.get_element_by_id(id) {
        element.set_text_content(Some(text));
    }
}

pub fn set_data_state(document: &Document, id: &str, value: &str) {
    if let Some(element) = document.get_element_by_id(id) {
        let _ = element.set_attribute("data-state", value);
    }
}

pub fn set_style(document: &Document, id: &str, property: &str, value: &str) {
    if let Some(element) = document.get_element_by_id(id) {
        if let Some(html) = element.dyn_ref::<web_sys::HtmlElement>() {
            let _ = html.style().set_property(property, value);
        }
    }
}

pub fn set_wasm_ready() -> Result<(), JsValue> {
    let window = window()?;
    let callback = js_sys::Reflect::get(&window, &JsValue::from_str("setWasmReady"))?;
    if callback.is_function() {
        callback
            .dyn_into::<js_sys::Function>()?
            .call0(&JsValue::NULL)?;
    }
    Ok(())
}
