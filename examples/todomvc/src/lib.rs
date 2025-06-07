use mogwai::web::prelude::*;
use std::panic;
use wasm_bindgen::prelude::*;

mod app;
mod item;
mod store;
mod utils;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Trace).expect("could not init console_log");

    let items = app::Items::default();
    JsDom::try_from(items.viewbuilder()).unwrap().run().unwrap();

    Ok(())
}
