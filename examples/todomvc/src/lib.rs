use log::trace;
use mogwai::prelude::*;
use std::panic;
use wasm_bindgen::prelude::*;

mod store;
mod utils;

mod app;
use app::{App, In};


// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;


fn fresh_app(msgs: Vec<In>) -> Result<(), JsValue> {
    let app: Gizmo<App> = Gizmo::from(App::new());
    msgs.into_iter().for_each(|msg| {
        app.update(&msg);
    });

    let Gizmo { view: app_view, .. } = app;
    app_view.run()
}


#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    //console_log::init_with_level(Level::Trace).expect("could not init console_log");

    if cfg!(debug_assertions) {
        trace!("Hello from debug mogwai-todo");
    } else {
        trace!("Hello from release mogwai-todo");
    }

    // Get the any items stored from a previous visit
    let mut msgs = store::read_items()?
        .into_iter()
        .map(|item| In::NewTodo(item.title, item.completed))
        .collect::<Vec<_>>();

    // Get the hash for "routing"
    let hash = window().location().hash()?;

    App::url_to_filter_msg(hash)
        .into_iter()
        .for_each(|msg| msgs.push(msg));

    fresh_app(msgs)
}
