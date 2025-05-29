//! The mogwai-dom js-framework-benchmark application.
#[cfg(feature = "entrypoint")]
use wasm_bindgen::prelude::*;

mod app;
mod data;
mod row;

pub use app::{App, AppView};

#[cfg(feature = "entrypoint")]
#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init().expect("could not init console_log");
    app::App::init()
}
