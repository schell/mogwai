mod api;
mod app;
mod components;
mod routes;

use crate::app::App;
use mogwai::prelude::*;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Trace).expect("could not init console_log");

    if cfg!(debug_assertions) {
        log::trace!("Hello from debug mogwai-multipage");
    } else {
        log::trace!("Hello from release mogwai-multipage");
    }

    // Create our app's view by hydrating a gizmo from an initial state
    let root: Gizmo<App> = Gizmo::new(App::new());

    // Hand the app's view ownership to the window so it never
    // goes out of scope
    View::from(root).run()
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
