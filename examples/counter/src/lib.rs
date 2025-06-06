use log::Level;
use mogwai_futura::web::prelude::*;
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
    clicks: Proxy<V, u32>,
}

impl<V: View> Default for ButtonClick<V> {
    fn default() -> Self {
        let proxy = Proxy::default();

        rsx! {
            let wrapper = button(
                style:cursor = "pointer",
                on:click = on_click
            ) {
                {proxy(clicks => match *clicks {
                    1 => "Click again.".into_text::<V>(),
                    n => format!("Clicked {n} times.").into_text::<V>(),
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
        mogwai_futura::web::document()
            .get_element_by_id(&id)
            .unwrap_throw()
            .append_child(&view);
    } else {
        mogwai_futura::web::body().append_child(&view);
    }

    wasm_bindgen_futures::spawn_local(async move {
        loop {
            let _ev = view.on_click.next().await;
            view.clicks.set(*view.clicks + 1);
        }
    });
}
