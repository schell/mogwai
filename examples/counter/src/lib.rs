use log::Level;
use mogwai::web::prelude::*;
use std::panic;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[derive(ViewChild)]
pub struct ButtonClick<V: View> {
    #[child]
    wrapper: V::Element,
    on_click: V::EventListener,
    clicks: Proxy<u32>,
}

impl<V: View> Default for ButtonClick<V> {
    fn default() -> Self {
        let mut proxy = Proxy::default();

        rsx! {
            let wrapper = button(
                style:cursor = "pointer",
                on:click = on_click
            ) {
                // When `proxy` is updated
                {proxy(clicks => match *clicks {
                    1 => "Click again.".to_string(),
                    n => format!("Clicked {n} times."),
                })}
            }
        }

        Self {
            wrapper,
            clicks: proxy,
            on_click,
        }
    }
}

#[wasm_bindgen]
pub fn run(parent_id: Option<String>) {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    let mut view = ButtonClick::<Web>::default();
    if let Some(id) = parent_id {
        mogwai::web::document()
            .get_element_by_id(&id)
            .unwrap_throw()
            .append_child(&view);
    } else {
        mogwai::web::body().append_child(&view);
    }

    wasm_bindgen_futures::spawn_local(async move {
        loop {
            let _ev = view.on_click.next().await;
            view.clicks.set(*view.clicks + 1);
        }
    });
}
