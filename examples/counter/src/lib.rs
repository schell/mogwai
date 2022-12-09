#![allow(unused_braces)]
use log::Level;
use mogwai_dom::prelude::*;
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

    mogwai_dom::view::js::spawn_local(async {
        let clicked = Output::<()>::default();
        let mut message = Input::<String>::default();

        let bldr: ViewBuilder = rsx! {
            button(
             style:cursor = "pointer",
             on:click = clicked.sink().contra_map(|_: JsDomEvent| ())
            ) {
                {("Click me!", message.stream().unwrap())}
            }
        }
        .with_task(async move {
            let mut clicks: u32 = 0;
            loop {
                match clicked.get().await {
                    Some(_ev) => {
                        clicks += 1;
                        message
                            .set(match clicks {
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

        let view = JsDom::try_from(bldr).unwrap();
        if let Some(id) = parent_id {
            let doc = mogwai_dom::utils::document();
            let parent = doc
                .visit_as::<web_sys::Document, JsDom>(|doc| {
                    JsDom::from_jscast(&doc.get_element_by_id(&id).unwrap())
                })
                .unwrap();
            view.run_in_container(&parent)
        } else {
            view.run()
        }
        .unwrap();
    });

    Ok(())
}
