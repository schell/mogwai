use futures::future::FutureExt;
use log::Level;
use mogwai::web::prelude::*;
use std::panic;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// ANCHOR: cookbook_components_counter
#[derive(ViewChild)]
struct Counter<V: View> {
    #[child]
    wrapper: V::Element,
    on_click: V::EventListener,
    text: V::Text,
    clicks: usize,
}

impl<V: View> Default for Counter<V> {
    fn default() -> Self {
        rsx! {
            let wrapper = button(on:click = on_click, style:cursor = "pointer") {
                let text = "Click me."
            }
        }
        Self {
            wrapper,
            on_click,
            text,
            clicks: 0,
        }
    }
}

impl<V: View> Counter<V> {
    fn update_text(&self) {
        self.text.set_text(match self.clicks {
            1 => "Clicked one time.".into(),
            n => format!("Clicked {n} times."),
        });
    }

    async fn run_until_click(&mut self) {
        let _event = self.on_click.next().await;
        self.clicks += 1;
        self.update_text();
    }

    fn reset(&mut self) {
        self.clicks = 0;
        self.update_text();
    }
}
// ANCHOR_END: cookbook_components_counter

// ANCHOR: cookbook_components_app
#[derive(ViewChild)]
struct App<V: View> {
    #[child]
    wrapper: V::Element,
    counter: Counter<V>,
    on_click_reset: V::EventListener,
}

impl<V: View> Default for App<V> {
    fn default() -> Self {
        rsx! {
            let wrapper = div() {
                "Application"
                br(){}
                let counter = {Counter::default()}
                button(on:click = on_click_reset, style:cursor = "pointer") {
                    "Click to reset."
                }
            }
        }
        Self {
            wrapper,
            counter,
            on_click_reset,
        }
    }
}

impl<V: View> App<V> {
    async fn run_until_event(&mut self) {
        futures::select! {
            _counter_clicked = self.counter.run_until_click().fuse() => {}
            _reset_clicked = self.on_click_reset.next().fuse() => {
                self.counter.reset();
            }
        }
    }
}
// ANCHOR_END: cookbook_components_app

#[wasm_bindgen]
pub fn run(parent_id: Option<String>) -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    let mut app = App::<Web>::default();

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
            app.run_until_event().await;
        }
    });

    Ok(())
}
