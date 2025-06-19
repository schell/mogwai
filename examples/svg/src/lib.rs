use log::Level;
use mogwai::web::prelude::*;
use std::panic;
use wasm_bindgen::prelude::*;

#[derive(ViewChild)]
struct Circle<V: View> {
    #[child]
    wrapper: V::Element,
}

/// Create an SVG circle using the xmlns attribute and the SVG namespace.
impl<V: View> Default for Circle<V> {
    fn default() -> Self {
        let ns = "http://www.w3.org/2000/svg";
        rsx! {
            let wrapper = svg(xmlns=ns, width="100", height="100") {
                circle(
                    xmlns=ns,
                    cx="50",
                    cy="50",
                    r="40",
                    stroke="green",
                    stroke_width="4",
                    fill="yellow"
                ){}
            }
        }
        Circle { wrapper }
    }
}

#[wasm_bindgen]
pub fn run(parent_id: Option<String>) {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    let circle = Circle::<Web>::default();

    if let Some(id) = parent_id {
        let parent = mogwai::web::document()
            .get_element_by_id(&id)
            .unwrap_throw();
        parent.append_child(&circle);
    } else {
        mogwai::web::body().append_child(&circle);
    }
}
