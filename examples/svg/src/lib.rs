#![allow(unused_braces)]
use log::Level;
use mogwai::prelude::*;
use std::panic;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// Create an SVG circle using the xmlns attribute and the SVG namespace.
fn my_circle() -> ViewBuilder<JsDom> {
    let ns = "http://www.w3.org/2000/svg";
    html! {
        <svg xmlns=ns width="100" height="100">
            <circle xmlns=ns
                cx="50"
                cy="50"
                r="40"
                stroke="green"
                stroke-width="4"
                fill="yellow" />
        </svg>
    }
}

#[wasm_bindgen]
pub fn main(parent_id: Option<String>) {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    mogwai::spawn(async {
        let dom = my_circle().build().unwrap();

        if let Some(id) = parent_id {
            let parent = mogwai::dom::utils::document()
                .clone_as::<web_sys::Document>()
                .unwrap()
                .get_element_by_id(&id)
                .map(Dom::wrap_js)
                .unwrap();
            dom.run_in_container(&parent)
        } else {
            dom.run()
        }
    });
}
