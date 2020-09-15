use mogwai::prelude::*;

use wasm_bindgen::prelude::*;

use log::Level;
use std::panic;

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap_throw();

    let root = game();
    root.run()
}

fn game() -> View<HtmlElement> {
    view! {
        <div class="game">
           "Hello"
        </div>
    }
}