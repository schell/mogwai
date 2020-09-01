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

    // Create our app's view by hydrating a gizmo from an initial state
    let app: Gizmo<App> = match Gizmo::hydrate(App::new()) {
        Err(err) => panic!("{}", err),
        Ok(app) => app,
    };

    // Send our gizmo all the initial messages it needs to populate
    // the stored todos.
    msgs.into_iter().for_each(|msg| {
        // notice how this doesn't mutate the app gizmo -
        // under the hood we're simply queueing these messages
        app.update(&msg);
    });

    // Unravel the gizmo because all we need is the view -
    // we can disregard the message terminals and the shared state
    // (the view already has clones of these things).
    let Gizmo { view: app_view, .. } = app;
    // Hand the app's view ownership to the window so it never
    // goes out of scope
    app_view.forget()
}
