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
    use mogwai_futura::web::prelude::*;

    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Trace).expect("could not init console_log");
    let app = App::default();
    let view = AppView::<Web>::default();
    view.init();
    wasm_bindgen_futures::spawn_local(app.run(view));
}
