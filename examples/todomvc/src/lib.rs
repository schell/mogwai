use mogwai::prelude::*;
use std::panic;
use wasm_bindgen::prelude::*;

mod store;
mod utils;

mod app;
use app::App;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Trace).expect("could not init console_log");

    if cfg!(debug_assertions) {
        log::trace!("Hello from debug mogwai-todo");
    } else {
        log::trace!("Hello from release mogwai-todo");
    }

    spawn(async {
        let (app, component) = App::new();
        let view: Dom = component.build().unwrap();
        view.run().unwrap();

        // Get the any items stored from a previous visit and add them
        // to the app.
        for item in store::read_items().unwrap() {
            app.add_item(item).await;
        }

        // Get the hash for "routing"
        let hash = mogwai::dom::utils::window().location().hash().unwrap();
        if let Some(filter) = app::url_to_filter(hash) {
            app.filter(filter).await;
        }
    });

    Ok(())
}
