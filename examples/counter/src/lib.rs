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

#[wasm_bindgen]
pub fn main(parent_id: Option<String>) -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    mogwai::spawn(async {
        let (to_logic, mut from_view) = broadcast::bounded::<()>(1);
        let (to_view, from_logic) = broadcast::bounded::<String>(1);
        let bldr: ViewBuilder<JsDom> = html! {
            <button
             style:cursor = "pointer"
             on:click=to_logic.clone().with(|_| async{Ok(())})
             >
                {("Click me!", from_logic)}
            </button>
        };

        let view = bldr.build().unwrap();
        if let Some(id) = parent_id {
            let parent = mogwai::dom::utils::document()
                .visit_js(|t: web_sys::Document| t.get_element_by_id(&id))
                .map(Dom::wrap_js)
                .unwrap();
            view.run_in_container(&parent)
        } else {
            view.run()
        }
        .unwrap();

        let mut clicks: u32 = 0;
        loop {
            match from_view.next().await {
                Some(_ev) => {
                    clicks += 1;
                    to_view
                        .broadcast(match clicks {
                            1 => "Click again.".to_string(),
                            n => format!("Clicked {} times", n),
                        })
                        .await
                        .unwrap();
                }
                None => break,
            }
        }
    });

    Ok(())
}
