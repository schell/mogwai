use log::{trace, Level};
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
    console_log::init_with_level(Level::Trace).expect("could not init console_log");

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

    // Create our app
    let app = App::new().into_gizmo();
    // Send our app all the initi messages it needs
    msgs.into_iter().for_each(|msg| {
        // notice how this doesn't mutate the app object -
        // under the hood we're simply queueing these messages
        app.update(&msg);
    });
    // run the app, giving up ownership to the window
    app.run().unwrap_throw();


    // The footer has no relation to the rest of the app and is simply a view
    // attached to the body
    view!(
        <footer class="info">
            <p>"Double click to edit a todo"</p>
            <p>
                "Written by "
                <a href="https://github.com/schell">"Schell Scivally"</a>
            </p>
            <p>
                "Part of "
                <a href="http://todomvc.com">"TodoMVC"</a>
            </p>
        </footer>
    )
    .run()
}
