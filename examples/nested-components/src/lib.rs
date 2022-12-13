use log::Level;
use mogwai_dom::core::futures::stream;
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
        let mut msg = stream::select_all(vec![click_stream.boxed(), recv_parent_msg.boxed()]);
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

#[wasm_bindgen]
pub fn main(parent_id: Option<String>) -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    let view = JsDom::try_from(app()).unwrap();

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

#[cfg(test)]
mod test {
    use mogwai_dom::prelude::*;

    #[test]
    fn can_component_from_viewbuilder() {
        let _comp = html! {
            <div id="my_component">
                <p>"Hello!"</p>
            </div>
        };
    }

    #[smol_potat::test]
    async fn can_component_logic() {
        let (tx, rx) = broadcast::bounded::<u32>(1);
        let comp = html! (
            <div id="my_component">
                <p>
                    {("initial value", rx.map(|n| format!("got message: {}", n)))}
                </p>
            </div>
        )
        .with_task(async move {
            tx.broadcast(1).await.unwrap();
            tx.broadcast(42).await.unwrap();
        });
        let view = JsDom::try_from(comp).unwrap();
        view.run().unwrap();
    }

    #[smol_potat::test]
    async fn can_more_component_logic() {
        let (tx_logic, mut rx_logic) = broadcast::bounded::<()>(1);
        let (tx_view, rx_view) = broadcast::bounded::<u32>(1);

        let comp = html! (
            <div id="my_component" on:click=tx_logic.contra_map(|_| ())>
                <p>
                    {("initial value", rx_view.map(|n| format!("got clicks: {}", n)))}
                </p>
            </div>
        )
        .with_task(async move {
            let mut clicks = 0;
            tx_view.broadcast(clicks).await.unwrap();

            loop {
                match rx_logic.next().await {
                    Some(()) => {
                        clicks += 1;
                        tx_view.broadcast(clicks).await.unwrap();
                    }
                    None => break,
                }
            }
        });
        let view = JsDom::try_from(comp).unwrap();
        view.run().unwrap();
    }
}
