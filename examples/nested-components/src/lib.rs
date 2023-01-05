use log::Level;
use mogwai_dom::prelude::*;
use std::panic;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[derive(Clone)]
enum CounterMsg {
    Click,
    Reset,
}

// ANCHOR: cookbook_components_counter
fn counter(recv_parent_msg: impl Stream<Item = CounterMsg> + Send + 'static) -> ViewBuilder {
    let clicked = Output::<CounterMsg>::default();
    let mut num_clicks = Input::<u32>::default();
    let click_stream = clicked.get_stream();

    rsx! (
        button(on:click = clicked.sink().contra_map(|_: JsDomEvent| CounterMsg::Click)) {
            {(
                "clicks = 0",
                num_clicks.stream().unwrap().map(|n| format!("clicks = {}", n))
            )}
        }
    )
    .with_task(async move {
        let mut msg = click_stream.boxed().or(recv_parent_msg.boxed());
        let mut clicks: u32 = 0;
        loop {
            match msg.next().await {
                Some(CounterMsg::Click) => {
                    clicks += 1;
                }
                Some(CounterMsg::Reset) => {
                    clicks = 0;
                }
                None => break,
            }

            num_clicks.set(clicks).await.unwrap();
        }
    })
}
// ANCHOR_END: cookbook_components_counter

// ANCHOR: cookbook_components_app
fn app() -> ViewBuilder {
    let reset_clicked = Output::<CounterMsg>::default();

    rsx! {
        div() {
            "Application"
            br(){}
            {counter(reset_clicked.get_stream())}
            button(on:click = reset_clicked.sink().contra_map(|_:JsDomEvent| CounterMsg::Reset)){
                "Click to reset"
            }
        }
    }
}
// ANCHOR_END: cookbook_components_app

#[wasm_bindgen]
pub fn main(parent_id: Option<String>) -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    // ANCHOR: cookbook_components_app_build
    let view = JsDom::try_from(app()).unwrap();
    // ANCHOR_END: cookbook_components_app_build

    if let Some(id) = parent_id {
        let parent = mogwai_dom::utils::document()
            .visit_as::<web_sys::Document, JsDom>(|doc| {
                JsDom::from_jscast(&doc.get_element_by_id(&id).unwrap())
            })
            .unwrap();
        view.run_in_container(parent)
    } else {
        view.run()
    }
    .unwrap();

    Ok(())
}
