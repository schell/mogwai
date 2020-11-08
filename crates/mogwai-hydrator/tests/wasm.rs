#![allow(unused_braces)]
use mogwai::prelude::*;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn can_hydrate_view() {
    let container = view! {
        <div id="hydrator1"></div>
    };
    let container_el: HtmlElement = container.dom_ref().clone();
    container.run().unwrap();
    container_el.set_inner_html(r#"<div id="my_div"><p>inner text</p></div>"#);
    assert_eq!(
        container_el.inner_html().as_str(),
        r#"<div id="my_div"><p>inner text</p></div>"#
    );

    let (tx_class, rx_class) = txrx::<String>();
    let (tx_text, rx_text) = txrx::<String>();
    let _hydrated_view: View<HtmlElement> = View::try_from(hydrate! {
        <div id="my_div">
            <p class=rx_class>{rx_text}</p>
        </div>
    })
    .unwrap();

    tx_class.send(&"new_class".to_string());
    assert_eq!(
        container_el.inner_html().as_str(),
        r#"<div id="my_div"><p class="new_class">inner text</p></div>"#
    );

    tx_text.send(&"different inner text".to_string());

    assert_eq!(
        container_el.inner_html().as_str(),
        r#"<div id="my_div"><p class="new_class">different inner text</p></div>"#
    );
}

#[wasm_bindgen_test]
async fn can_hydrate_or_view() {
    let (tx_class, rx_class) = txrx::<String>();
    let (tx_text, rx_text) = txrx::<String>();
    let count = txrx::new_shared(0 as u32);
    let (tx_pb, rx_pb) =
        txrx_fold_shared(count.clone(), |count: &mut u32, _: &HtmlElement| -> u32 {
            *count += 1;
            *count
        });

    let fresh_view = || {
        view! {
            <div id="my_div2" post:build=(&tx_pb).clone()>
                <p class=("class", rx_class.branch())>
                    {("inner text", rx_text.branch())}
                </p>
            </div>
        }
    };
    let hydrate_view = || {
        View::try_from(hydrate! {
            <div id="my_div2" post:build=(&tx_pb).clone()>
                <p class=("class", rx_class.branch())>{("inner text", rx_text.branch())}</p>
                </div>
        })
    };

    let view = fresh_view();
    let pb_count = rx_pb.message().await;
    assert_eq!(pb_count, 1, "no post-build sent after fresh view");

    let original_el: HtmlElement = (view.dom_ref().as_ref() as &HtmlElement).clone();
    view.run().unwrap();

    let _hydrated_view = hydrate_view().unwrap();

    tx_class.send(&"new_class".to_string());
    tx_text.send(&"different inner text".to_string());

    assert_eq!(
        original_el.outer_html().as_str(),
        r#"<div id="my_div2"><p class="new_class">different inner text</p></div>"#
    );

    // post builds are sent out the frame after the view is created, so we can await
    // responses from the post build receiver.
    let pb_count = rx_pb.message().await;
    assert_eq!(pb_count, 2);
}
