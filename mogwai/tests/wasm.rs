#![allow(unused_braces)]
use mogwai::prelude::*;
use mogwai_html_macro::target_arch_is_wasm32;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::Element;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn this_arch_is_wasm32() {
    assert!(target_arch_is_wasm32! {});
}

#[wasm_bindgen_test]
fn gizmo_ref_as_child() {
    // Since the pre tag is dropped after the scope block the last assert should
    // show that the div tag has no children.
    let div = {
        let pre = view! { <pre>"this has text"</pre> };
        let div = view! { <div id="parent"></div> };
        (div.as_ref() as &Node).append_child(pre.as_ref()).unwrap();
        assert!(
            div.first_child().is_some(),
            "parent does not contain in-scope child"
        );
        //console::log_1(&"dropping pre".into());
        div
    };
    assert!(
        div.first_child().is_none(),
        "parent does not maintain out-of-scope child"
    );
    //console::log_1(&"dropping parent".into());
}

#[wasm_bindgen_test]
fn gizmo_as_child() {
    // Since the pre tag is *not* dropped after the scope block the last assert
    // should show that the div tag has a child.
    let div = {
        let div = view! {
            <div id="parent-div">
                <pre>"some text"</pre>
            </div>
        };
        assert!(div.first_child().is_some(), "could not add child gizmo");
        div
    };
    assert!(
        div.first_child().is_some(),
        "could not keep hold of child gizmo"
    );
    assert_eq!(div.children.len(), 1, "parent is missing static_gizmo");
    //console::log_1(&"dropping div and pre".into());
}

#[wasm_bindgen_test]
fn gizmo_tree() {
    let root = view! {
        <div id="root">
            <div id="branch">
                <div id="leaf">
                    "leaf"
                </div>
            </div>
        </div>
    };
    if let Some(branch) = root.first_child() {
        if let Some(leaf) = branch.first_child() {
            if let Some(leaf) = leaf.dyn_ref::<Element>() {
                assert_eq!(leaf.id(), "leaf");
            } else {
                panic!("leaf is not an Element");
            }
        } else {
            panic!("branch has no leaf");
        }
    } else {
        panic!("root has no branch");
    }
}

#[wasm_bindgen_test]
fn gizmo_texts() {
    let div = view! {
        <div>
            "here is some text "
            // i can use comments, yay!
            {&format!("{}", 66)}
            " <- number"
        </div>
    };
    assert_eq!(
        &div.outer_html(),
        "<div>here is some text 66 &lt;- number</div>"
    );
}

#[wasm_bindgen_test]
fn rx_attribute_jsx() {
    let (tx, rx) = txrx::<String>();
    let div = view! {
        <div class=("now", rx) />
    };
    let div_el: &HtmlElement = div.as_ref();
    assert_eq!(div_el.outer_html(), r#"<div class="now"></div>"#);

    tx.send(&"later".to_string());
    assert_eq!(div_el.outer_html(), r#"<div class="later"></div>"#);
}

#[wasm_bindgen_test]
fn rx_style_plain() {
    let (tx, rx) = txrx::<String>();

    let mut div: View<HtmlElement> = View::element("div");
    div.style("display", ("block", rx));

    let div_el: &HtmlElement = div.as_ref();
    assert_eq!(
        div_el.outer_html(),
        r#"<div style="display: block;"></div>"#
    );

    tx.send(&"none".to_string());
    assert_eq!(div_el.outer_html(), r#"<div style="display: none;"></div>"#);
}

#[wasm_bindgen_test]
fn rx_style_jsx() {
    let (tx, rx) = txrx::<String>();
    let div = view! { <div style:display=("block", rx) /> };
    let div_el: &HtmlElement = div.as_ref();
    assert_eq!(
        div_el.outer_html(),
        r#"<div style="display: block;"></div>"#
    );

    tx.send(&"none".to_string());
    assert_eq!(div_el.outer_html(), r#"<div style="display: none;"></div>"#);
}

#[wasm_bindgen_test]
fn rx_text() {
    let (tx, rx) = txrx();

    let mut div: View<HtmlElement> = View::element("div");
    div.with(("initial", rx).into());

    let el: &HtmlElement = div.as_ref();
    assert_eq!(el.inner_text().as_str(), "initial");
    tx.send(&"after".into());
    assert_eq!(el.inner_text(), "after");
}

#[wasm_bindgen_test]
fn tx_on_click_plain() {
    let (tx, rx) = txrx_fold(0, |n: &mut i32, _: &Event| -> String {
        *n += 1;
        if *n == 1 {
            "Clicked 1 time".to_string()
        } else {
            format!("Clicked {} times", *n)
        }
    });

    let mut button: View<HtmlElement> = View::element("button");
    button.with(("Clicked 0 times", rx).into());
    button.on("click", tx);

    let el: &HtmlElement = button.as_ref();
    assert_eq!(el.inner_html(), "Clicked 0 times");
    el.click();
    assert_eq!(el.inner_html(), "Clicked 1 time");
}

#[wasm_bindgen_test]
fn tx_on_click_jsx() {
    let (tx, rx) = txrx_fold(0, |n: &mut i32, _: &Event| -> String {
        *n += 1;
        if *n == 1 {
            "Clicked 1 time".to_string()
        } else {
            format!("Clicked {} times", *n)
        }
    });

    let button = view! { <button on:click=tx>{("Clicked 0 times", rx)}</button> };
    let el: &HtmlElement = button.as_ref();

    assert_eq!(el.inner_html(), "Clicked 0 times");
    el.click();
    assert_eq!(el.inner_html(), "Clicked 1 time");
}


#[wasm_bindgen_test]
fn tx_window_on_click_jsx() {
    let (tx, rx) = txrx();
    let _button = view! {
        <button window:load=tx>
        {(
            "Waiting...",
            rx.branch_map(|_| "Loaded!".into())
        )}
        </button>
    };
}

//fn nice_compiler_error() {
//    let _div = view! {
//        <div unknown:colon:thing="not ok" />
//    };
//}

#[test]
#[wasm_bindgen_test]
fn can_i_alter_views_on_the_server() {
    let (tx_text, rx_text) = txrx::<String>();
    let (tx_style, rx_style) = txrx::<String>();
    let (tx_class, rx_class) = txrx::<String>();
    let view = view! {
        <div style:float=("left", rx_style)><p class=("p_class", rx_class)>{("here", rx_text)}</p></div>
    };
    assert_eq!(
        &view.clone().into_html_string(),
        r#"<div style="float: left;"><p class="p_class">here</p></div>"#
    );

    tx_text.send(&"there".to_string());
    assert_eq!(
        &view.clone().into_html_string(),
        r#"<div style="float: left;"><p class="p_class">there</p></div>"#
    );

    tx_style.send(&"right".to_string());
    assert_eq!(
        &view.clone().into_html_string(),
        r#"<div style="float: right;"><p class="p_class">there</p></div>"#
    );

    tx_class.send(&"my_p_class".to_string());
    assert_eq!(
        &view.clone().into_html_string(),
        r#"<div style="float: right;"><p class="my_p_class">there</p></div>"#
    );
}


#[wasm_bindgen_test]
fn can_hydrate_view() {
    let original_view = view! {
        <div id="my_div">
            <p class="class">"inner text"</p>
        </div>
    };
    let original_el: HtmlElement = (original_view.as_ref() as &HtmlElement).clone();
    original_view.run().unwrap();

    let (tx_class, rx_class) = txrx::<String>();
    let (tx_text, rx_text) = txrx::<String>();
    let hydrated_view = View::try_from(hydrate! {
        <div id="my_div">
            <p class=("unused_class", rx_class)>{("unused inner text", rx_text)}</p>
        </div>
    })
    .unwrap();

    hydrated_view.forget().unwrap();

    tx_class.send(&"new_class".to_string());
    tx_text.send(&"different inner text".to_string());

    assert_eq!(
        original_el.outer_html().as_str(),
        r#"<div id="my_div"><p class="new_class">different inner text</p></div>"#
    );
}


#[wasm_bindgen_test]
fn can_hydrate_or_view() {
    let (tx_class, rx_class) = txrx::<String>();
    let (tx_text, rx_text) = txrx::<String>();
    let count = txrx::new_shared(0 as u32);
    let (tx_pb, rx_pb) =
        txrx_fold_shared(count.clone(), |count: &mut u32, _: &HtmlElement| -> () {
            *count += 1;
            ()
        });

    rx_pb.respond(|_| println!("post build"));

    let fresh_view = || {
        view! {
            <div id="my_div" post:build=(&tx_pb).clone()>
                <p class=("class", rx_class.branch())>{("inner text", rx_text.branch())}</p>
            </div>
        }
    };
    let hydrate_view = || {
        View::try_from(hydrate! {
            <div id="my_div" post:build=(&tx_pb).clone()>
                <p class=("class", rx_class.branch())>{("inner text", rx_text.branch())}</p>
            </div>
        })
    };

    let view = fresh_view();

    let original_el: HtmlElement = (view.as_ref() as &HtmlElement).clone();
    view.run().unwrap();

    let hydrated_view = hydrate_view().unwrap();
    hydrated_view.forget().unwrap();

    tx_class.send(&"new_class".to_string());
    tx_text.send(&"different inner text".to_string());

    assert_eq!(
        original_el.outer_html().as_str(),
        r#"<div id="my_div"><p class="new_class">different inner text</p></div>"#
    );

    assert_eq!(*count.borrow(), 2);
}
