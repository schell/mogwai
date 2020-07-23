use log::{Level, trace};
use std::panic;
use mogwai::prelude::*;
use wasm_bindgen::prelude::*;

mod utils;
mod store;

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
  console_log::init_with_level(Level::Trace)
    .expect("could not init console_log");

  if cfg!(debug_assertions) {
    trace!("Hello from debug mogwai-todo");
  } else {
    trace!("Hello from release mogwai-todo");
  }

  // Get the any items stored from a previous visit
  let mut msgs =
    store::read_items()?
    .into_iter()
    .map(|item| In::NewTodo(item.title, item.completed))
    .collect::<Vec<_>>();

  // Get the hash for "routing"
  let hash =
    window()
    .location()
    .hash()?;

  App::url_to_filter_msg(hash)
    .into_iter()
    .for_each(|msg| msgs.push(msg));

  App::new()
    .into_component()
    .run_init(msgs)?;

  // The footer has no relation to the rest of the app and is simply a view
  // attached to the body
  footer()
    .class("info")
    .with(
      p()
        .text("Double click to edit a todo")
    )
    .with(
      p()
        .text("Written by ")
        .with(
          a()
            .attribute("href", "https://github.com/schell")
            .text("Schell Scivally")
        )
    )
    .with(
      p()
        .text("Part of ")
        .with(
          a()
            .attribute("href", "http://todomvc.com")
            .text("TodoMVC")
        )
    )
    .run()

}
