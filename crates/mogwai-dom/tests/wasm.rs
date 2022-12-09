use mogwai_dom::prelude::*;
use mogwai_dom::{view::js::Hydrator, core::{time::repeat_times, channel::mpsc}};
use std::convert::TryFrom;
use wasm_bindgen_test::*;
use web_sys::HtmlElement;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn can_hydrate_view() {
    console_log::init_with_level(log::Level::Trace).unwrap();

    let container = JsDom::try_from(html! {
        <div id="hydrator1"></div>
    }).unwrap();
    let container_el: HtmlElement = container.clone_as::<HtmlElement>().unwrap();
    container.run().unwrap();
    container_el.set_inner_html(r#"<div id="my_div"><p>inner text</p></div>"#);
    assert_eq!(
        container_el.inner_html().as_str(),
        r#"<div id="my_div"><p>inner text</p></div>"#
    );
    log::info!("built");

    let (mut tx_class, rx_class) = mpsc::bounded::<String>(1);
    let (mut tx_text, rx_text) = mpsc::bounded::<String>(1);
    let builder = html! {
        <div id="my_div">
            <p class=rx_class>{("", rx_text)}</p>
        </div>
    };
    let hydrator = Hydrator::try_from(builder)
        .map_err(|e| panic!("{:#?}", e))
        .unwrap();
    let _hydrated_view: JsDom = JsDom::from(hydrator);
    log::info!("hydrated");

    tx_class.send("new_class".to_string()).await.unwrap();
    repeat_times(0.1, 10, || async {
        container_el.inner_html().as_str()
            == r#"<div id="my_div"><p class="new_class">inner text</p></div>"#
    })
    .await
    .unwrap();
    log::info!("updated class");

    tx_text
        .send("different inner text".to_string())
        .await
        .unwrap();
    repeat_times(0.1, 10, || async {
        container_el.inner_html().as_str()
            == r#"<div id="my_div"><p class="new_class">different inner text</p></div>"#
    })
    .await
    .unwrap();
    log::info!("updated text");
}
