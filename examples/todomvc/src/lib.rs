use mogwai::web::prelude::*;
use std::panic;
use wasm_bindgen::prelude::*;

mod app;
mod item;
mod store;
mod utils;

#[wasm_bindgen]
pub fn run(parent_id: Option<String>) {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Trace).expect("could not init console_log");

    let mut app = app::App::<Web>::default();
    app.add_items(store::read_items().unwrap_throw());

    if let Some(id) = parent_id {
        let parent = mogwai::web::document()
            .get_element_by_id(&id)
            .unwrap_throw();
        parent.append_child(&app);
    } else {
        mogwai::web::body().append_child(&app);
    }

    wasm_bindgen_futures::spawn_local(async move {
        loop {
            app.run_step().await;
        }
    });
}
