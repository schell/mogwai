#![allow(unused_braces)]
use mogwai::prelude::*;
use mogwai_hydrator::Hydrator;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn can_hydrate_view() {
    console_log::init_with_level(log::Level::Trace).unwrap();

    let container = view! {
        <div id="hydrator1"></div>
    };
    let container_el: HtmlElement = container.inner.clone_as::<HtmlElement>().unwrap();
    container.run().unwrap();
    container_el.set_inner_html(r#"<div id="my_div"><p>inner text</p></div>"#);
    assert_eq!(
        container_el.inner_html().as_str(),
        r#"<div id="my_div"><p>inner text</p></div>"#
    );
    log::info!("built");

    let (tx_class, rx_class) = mpmc::bounded::<String>(1);
    let (tx_text, rx_text) = mpmc::bounded::<String>(1);
    let builder = builder! {
        <div id="my_div">
            <p class=rx_class>{("", rx_text)}</p>
        </div>
    };
    let hydrator = Hydrator::try_from(builder).map_err(|e| panic!("{:#?}", e)).unwrap();
    let _hydrated_view: View<Dom> = View::from(hydrator);
    log::info!("hydrated");

    tx_class.send("new_class".to_string()).await.unwrap();
    mpmc::until_empty(&tx_class).await;
    assert_eq!(
        container_el.inner_html().as_str(),
        r#"<div id="my_div"><p class="new_class">inner text</p></div>"#
    );
    log::info!("updated class");

    tx_text
        .send("different inner text".to_string())
        .await
        .unwrap();
    mpmc::until_empty(&tx_text).await;
    assert_eq!(tx_text.receiver_count(), 1);
    assert_eq!(
        container_el.inner_html().as_str(),
        r#"<div id="my_div"><p class="new_class">different inner text</p></div>"#
    );
    log::info!("updated text");
}
