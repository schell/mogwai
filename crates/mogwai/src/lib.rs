#![warn(missing_docs)]
//! # Mogwai
//!
//! Mogwai is library for user interface development using Rust-to-Wasm
//! compilation. Its goals are simple:
//! * provide a declarative approach to creating and managing DOM nodes
//! * encapsulate component state and compose components easily
//! * explicate DOM updates
//! * feel snappy
//!
//! ## Learn more
//! If you're new to Mogwai, check out the [introduction](an_introduction) module.
//pub mod an_introduction;
pub mod builder;
pub mod channel;
pub mod event;
pub mod model;
pub mod patch;
//pub mod component;
//pub mod gizmo;
//pub mod prelude;
pub mod ssr;
//pub mod txrx;
pub mod utils;
pub mod view;

#[cfg(doctest)]
doc_comment::doctest!("../../README.md");

#[cfg(test)]
mod test {
    use futures::stream::once;
    use mogwai_html_macro::{builder, target_arch_is_wasm32};
    use std::{cell::Ref, convert::TryInto, ops::Deref};
    use wasm_bindgen::JsCast;
    use wasm_bindgen_test::*;
    use web_sys::Element;

    use crate::{self as mogwai, builder::ViewBuilder, ssr::SsrElement, utils, view::View};

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn this_arch_is_wasm32() {
        assert!(target_arch_is_wasm32! {});
    }

    #[wasm_bindgen_test]
    fn can_create_text_view_node_from_str() {
        let _view: View<web_sys::Text> = ViewBuilder::text("Hello!").try_into().unwrap();
    }

    #[wasm_bindgen_test]
    fn can_create_text_view_node_from_string() {
        let _view: View<web_sys::Text> =
            ViewBuilder::text("Hello!".to_string()).try_into().unwrap();
    }

    #[wasm_bindgen_test]
    fn can_create_text_view_node_from_stream() {
        let s = once(async { "Hello!".to_string() });
        let _view: View<web_sys::Text> = ViewBuilder::text(s).try_into().unwrap();
    }

    #[wasm_bindgen_test]
    fn can_create_text_view_node_from_string_and_stream() {
        let s = "Hello!".to_string();
        let st = once(async { "Goodbye!".to_string() });
        let _view: View<web_sys::Text> = ViewBuilder::text((s, st)).try_into().unwrap();
    }

    #[wasm_bindgen_test]
    fn can_create_text_view_node_from_str_and_stream() {
        let st = once(async { "Goodbye!".to_string() });
        let _view: View<web_sys::Text> = ViewBuilder::text(("Hello!", st)).try_into().unwrap();
    }

    #[wasm_bindgen_test]
    async fn can_nest_created_text_view_node() {
        let view: View<web_sys::HtmlElement> = ViewBuilder::element("div")
            .with_child(ViewBuilder::text("Hello!"))
            .with_single_attrib_stream("id", "view1")
            .with_single_style_stream("color", "red")
            .with_single_style_stream(
                "width",
                futures::stream::once(async { "100px".to_string() }),
            )
            .try_into()
            .unwrap();

        let _ = utils::wait_approximately(2.0).await;

        assert_eq!(
            String::from(&view).as_str(),
            r#"<div id="view1" style="color: red; width: 100px;">Hello!</div>"#
        );
    }

    #[wasm_bindgen_test]
    async fn ssr_can_nest_created_text_view_node() {
        let view: View<SsrElement<()>> = ViewBuilder::element("div")
            .with_child(ViewBuilder::text("Hello!"))
            .with_single_attrib_stream("id", "view1")
            .with_single_style_stream("color", "red")
            .with_single_style_stream(
                "width",
                futures::stream::once(async { "100px".to_string() }),
            )
            .try_into()
            .unwrap();

        let _ = utils::wait_approximately(2.0).await;
        let node = view.inner.node.lock().await;

        assert_eq!(
            String::from(node.deref()).as_str(),
            r#"<div id="view1" style="width: 100px; color: red;">Hello!</div>"#
        );
    }

    #[wasm_bindgen_test]
    fn can_use_rsx_to_make_builder() {
        let (tx, rx) = mogwai::channel::bounded::<web_sys::Event>(1);
        let rsx: ViewBuilder<web_sys::HtmlElement, web_sys::Node, web_sys::Event> = builder! {
            <div id="view_zero" style:background_color="red">
                <pre on:click=tx.clone()>"this has text"</pre>
            </div>
        };

        let manual: ViewBuilder<web_sys::HtmlElement, web_sys::Node, web_sys::Event> =
            mogwai::builder::ViewBuilder::element("div")
            .with_single_attrib_stream("id", "view_zero")
            .with_single_style_stream("background_color", "red")
            .with_child(
                mogwai::builder::ViewBuilder::element("pre")
                    .with_event("click", tx.clone())
                    .with_child(mogwai::builder::ViewBuilder::text("this has text")),
            );
    }

    //#[wasm_bindgen_test]
    //fn gizmo_ref_as_child() {
    //    // Since the pre tag is dropped after the scope block the last assert should
    //    // show that the div tag has no children.
    //    let div = {
    //        let pre = view! { <pre>"this has text"</pre> };
    //        let div = view! { <div id="parent"></div> };
    //        div.dom_ref().append_child(&pre.dom_ref()).unwrap();
    //        assert!(
    //            div.dom_ref().first_child().is_some(),
    //            "parent does not contain in-scope child"
    //        );
    //        //console::log_1(&"dropping pre".into());
    //        div
    //    };
    //    assert!(
    //        div.dom_ref().first_child().is_none(),
    //        "parent does not maintain out-of-scope child"
    //    );
    //    //console::log_1(&"dropping parent".into());
    //}

    //#[wasm_bindgen_test]
    //fn gizmo_as_child() {
    //    // Since the pre tag is *not* dropped after the scope block the last assert
    //    // should show that the div tag has a child.
    //    let div = {
    //        let div = view! {
    //            <div id="parent-div">
    //                <pre>"some text"</pre>
    //                </div>
    //        };
    //        assert!(
    //            div.dom_ref().first_child().is_some(),
    //            "could not add child gizmo"
    //        );
    //        div
    //    };
    //    assert!(
    //        div.dom_ref().first_child().is_some(),
    //        "could not keep hold of child gizmo"
    //    );
    //    assert_eq!(
    //        div.dom_ref().child_nodes().length(),
    //        1,
    //        "parent is missing static_gizmo"
    //    );
    //    //console::log_1(&"dropping div and pre".into());
    //}

    //#[wasm_bindgen_test]
    //fn gizmo_tree() {
    //    let root = view! {
    //        <div id="root">
    //            <div id="branch">
    //                <div id="leaf">
    //                    "leaf"
    //                </div>
    //            </div>
    //        </div>
    //    };
    //    let el = root.dom_ref();
    //    if let Some(branch) = el.first_child() {
    //        if let Some(leaf) = branch.first_child() {
    //            if let Some(leaf) = leaf.dyn_ref::<Element>() {
    //                assert_eq!(leaf.id(), "leaf");
    //            } else {
    //                panic!("leaf is not an Element");
    //            }
    //        } else {
    //            panic!("branch has no leaf");
    //        }
    //    } else {
    //        panic!("root has no branch");
    //    }
    //}

    //#[wasm_bindgen_test]
    //fn gizmo_texts() {
    //    let div = view! {
    //        <div>
    //            "here is some text "
    //        // i can use comments, yay!
    //        {&format!("{}", 66)}
    //        " <- number"
    //            </div>
    //    };
    //    assert_eq!(
    //        &div.dom_ref().outer_html(),
    //        "<div>here is some text 66 &lt;- number</div>"
    //    );
    //}

    //#[wasm_bindgen_test]
    //fn rx_attribute_jsx() {
    //    let (tx, rx) = txrx::<String>();
    //    let div = view! {
    //        <div class=("now", rx) />
    //    };
    //    let div_el: Ref<HtmlElement> = div.dom_ref();
    //    assert_eq!(div_el.outer_html(), r#"<div class="now"></div>"#);

    //    tx.send(&"later".to_string());
    //    assert_eq!(div_el.outer_html(), r#"<div class="later"></div>"#);
    //}

    //#[wasm_bindgen_test]
    //fn rx_style_plain() {
    //    let (tx, rx) = txrx::<String>();

    //    let mut div: View<HtmlElement> = View::element("div");
    //    div.style("display", ("block", rx));

    //    let div_el: Ref<HtmlElement> = div.dom_ref();
    //    assert_eq!(
    //        div_el.outer_html(),
    //        r#"<div style="display: block;"></div>"#
    //    );

    //    tx.send(&"none".to_string());
    //    assert_eq!(div_el.outer_html(), r#"<div style="display: none;"></div>"#);
    //}

    //#[wasm_bindgen_test]
    //fn rx_style_jsx() {
    //    let (tx, rx) = txrx::<String>();
    //    let div = view! { <div style:display=("block", rx) /> };
    //    let div_el: Ref<HtmlElement> = div.dom_ref();
    //    assert_eq!(
    //        div_el.outer_html(),
    //        r#"<div style="display: block;"></div>"#
    //    );

    //    tx.send(&"none".to_string());
    //    assert_eq!(div_el.outer_html(), r#"<div style="display: none;"></div>"#);
    //}

    //#[wasm_bindgen_test]
    //pub fn rx_text() {
    //    let (tx, rx) = txrx::<String>();

    //    let mut div: View<HtmlElement> = View::element("div");
    //    div.with(View::from(("initial", rx)));

    //    let el: Ref<HtmlElement> = div.dom_ref();
    //    assert_eq!(el.inner_text().as_str(), "initial");
    //    tx.send(&"after".into());
    //    assert_eq!(el.inner_text(), "after");
    //}

    //#[wasm_bindgen_test]
    //fn tx_on_click_plain() {
    //    let (tx, rx) = txrx_fold(0, |n: &mut i32, _: &Event| -> String {
    //        *n += 1;
    //        if *n == 1 {
    //            "Clicked 1 time".to_string()
    //        } else {
    //            format!("Clicked {} times", *n)
    //        }
    //    });

    //    let mut button: View<HtmlElement> = View::element("button");
    //    button.with(View::from(("Clicked 0 times", rx)));
    //    button.on("click", tx);

    //    let el: Ref<HtmlElement> = button.dom_ref();
    //    assert_eq!(el.inner_html(), "Clicked 0 times");
    //    el.click();
    //    assert_eq!(el.inner_html(), "Clicked 1 time");
    //}

    //#[wasm_bindgen_test]
    //fn tx_on_click_jsx() {
    //    let (tx, rx) = txrx_fold(0, |n: &mut i32, _: &Event| -> String {
    //        *n += 1;
    //        if *n == 1 {
    //            "Clicked 1 time".to_string()
    //        } else {
    //            format!("Clicked {} times", *n)
    //        }
    //    });

    //    let button = view! { <button on:click=tx>{("Clicked 0 times", rx)}</button> };
    //    let el: Ref<HtmlElement> = button.dom_ref();

    //    assert_eq!(el.inner_html(), "Clicked 0 times");
    //    el.click();
    //    assert_eq!(el.inner_html(), "Clicked 1 time");
    //}

    //#[wasm_bindgen_test]
    //fn tx_window_on_click_jsx() {
    //    let (tx, rx) = txrx();
    //    let _button = view! {
    //        <button window:load=tx>
    //        {(
    //            "Waiting...",
    //            rx.branch_map(|_| "Loaded!".into())
    //        )}
    //        </button>
    //    };
    //}

    ////fn nice_compiler_error() {
    ////    let _div = view! {
    ////        <div unknown:colon:thing="not ok" />
    ////    };
    ////}

    //#[test]
    //#[wasm_bindgen_test]
    //pub fn can_i_alter_views_on_the_server() {
    //    let (tx_text, rx_text) = txrx::<String>();
    //    let (tx_style, rx_style) = txrx::<String>();
    //    let (tx_class, rx_class) = txrx::<String>();
    //    let view = view! {
    //        <div style:float=("left", rx_style)><p class=("p_class", rx_class)>{("here", rx_text)}</p></div>
    //    };
    //    assert_eq!(
    //        &view.clone().html_string(),
    //        r#"<div style="float: left;"><p class="p_class">here</p></div>"#
    //    );

    //    tx_text.send(&"there".to_string());
    //    assert_eq!(
    //        &view.clone().html_string(),
    //        r#"<div style="float: left;"><p class="p_class">there</p></div>"#
    //    );

    //    tx_style.send(&"right".to_string());
    //    assert_eq!(
    //        &view.clone().html_string(),
    //        r#"<div style="float: right;"><p class="p_class">there</p></div>"#
    //    );

    //    tx_class.send(&"my_p_class".to_string());
    //    assert_eq!(
    //        &view.clone().html_string(),
    //        r#"<div style="float: right;"><p class="my_p_class">there</p></div>"#
    //    );
    //}

    //#[wasm_bindgen_test]
    //async fn can_wait_approximately() {
    //    let millis_waited = utils::wait_approximately(22.0).await;
    //    log::trace!("21 !>= {}", millis_waited);
    //    assert!(millis_waited >= 21.0);
    //}

    //#[wasm_bindgen_test]
    //async fn can_patch_children() {
    //    let (tx, rx) = txrx::<Patch<View<HtmlElement>>>();
    //    let view = view! {
    //        <ol id="main" patch:children=rx>
    //            <li>"Zero"</li>
    //            <li>"One"</li>
    //        </ol>
    //    };

    //    let dom: HtmlElement = view.dom_ref().clone();
    //    view.run().unwrap();

    //    assert_eq!(
    //        dom.outer_html().as_str(),
    //        r#"<ol id="main"><li>Zero</li><li>One</li></ol>"#
    //    );

    //    let two = view! {
    //        <li>"Two"</li>
    //    };

    //    tx.send(&Patch::PushBack { value: two });
    //    assert_eq!(
    //        dom.outer_html().as_str(),
    //        r#"<ol id="main"><li>Zero</li><li>One</li><li>Two</li></ol>"#
    //    );

    //    tx.send(&Patch::PopFront);
    //    assert_eq!(
    //        dom.outer_html().as_str(),
    //        r#"<ol id="main"><li>One</li><li>Two</li></ol>"#
    //    );

    //    tx.send(&Patch::Insert {
    //        index: 0,
    //        value: view! {<li>"Zero"</li>},
    //    });
    //    assert_eq!(
    //        dom.outer_html().as_str(),
    //        r#"<ol id="main"><li>Zero</li><li>One</li><li>Two</li></ol>"#
    //    );

    //    tx.send(&Patch::Remove { index: 2 });
    //    assert_eq!(
    //        dom.outer_html().as_str(),
    //        r#"<ol id="main"><li>Zero</li><li>One</li></ol>"#
    //    );

    //    tx.send(&Patch::PushFront {
    //        value: view! {<li>"Negative One"</li>},
    //    });
    //    assert_eq!(
    //        dom.outer_html().as_str(),
    //        r#"<ol id="main"><li>Negative One</li><li>Zero</li><li>One</li></ol>"#
    //    );

    //    tx.send(&Patch::PopBack);
    //    assert_eq!(
    //        dom.outer_html().as_str(),
    //        r#"<ol id="main"><li>Negative One</li><li>Zero</li></ol>"#
    //    );

    //    tx.send(&Patch::Replace {
    //        index: 1,
    //        value: view! {<li>"One"</li>},
    //    });
    //    assert_eq!(
    //        dom.outer_html().as_str(),
    //        r#"<ol id="main"><li>Negative One</li><li>One</li></ol>"#
    //    );

    //    tx.send(&Patch::RemoveAll);
    //    assert_eq!(dom.outer_html().as_str(), r#"<ol id="main"></ol>"#);
    //}

    //#[wasm_bindgen_test]
    //fn can_patch_children_into() {
    //    let (tx, rx) = txrx::<Patch<String>>();
    //    let view = view! {
    //        <p id="main" patch:children=rx>
    //            "Zero ""One"
    //        </p>
    //    };

    //    let dom: HtmlElement = view.dom_ref().clone();
    //    view.run().unwrap();

    //    assert_eq!(dom.outer_html().as_str(), r#"<p id="main">Zero One</p>"#);

    //    tx.send(&Patch::PushFront {
    //        value: "First ".to_string(),
    //    });
    //    assert_eq!(
    //        dom.outer_html().as_str(),
    //        r#"<p id="main">First Zero One</p>"#
    //    );

    //    tx.send(&Patch::RemoveAll);
    //    assert_eq!(dom.outer_html().as_str(), r#"<p id="main"></p>"#);
    //}

    //#[wasm_bindgen_test]
    //fn can_add_children_as_vec() {
    //    // Unfortunately this doesn't mix well with RSX. To remedy this:
    //    // TODO: Add ParentView impls instead of calling
    //    // `ViewBuilder::from` on everything passed to `with`.
    //    let mut view = view! {<ul></ul>};
    //    let children = (0..3)
    //        .map(|i| {
    //            view! { <li>{format!("{}", i)}</li> }
    //        })
    //        .collect::<Vec<_>>();
    //    view.with(children);

    //    assert_eq!(
    //        view.dom_ref().outer_html().as_str(),
    //        "<ul><li>0</li><li>1</li><li>2</li></ul>"
    //    );
    //}
}
