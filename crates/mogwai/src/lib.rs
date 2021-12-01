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
pub mod an_introduction;
pub mod builder;
pub mod channel;
pub mod component;
pub mod event;
pub mod futures;
pub mod model;
pub mod patch;
pub mod prelude;
pub mod relay;
pub mod ssr;
pub mod target;
pub mod time;
pub mod utils;
pub mod view;

pub use target::spawn;

pub mod lock {
    //! Asynchronous locking mechanisms (re-exports).
    pub use async_lock::*;
    pub use futures::lock::*;
}

pub mod macros {
    //! RSX style macros for building DOM views.
    pub use mogwai_html_macro::{builder, view};
}

#[cfg(doctest)]
doc_comment::doctest!("../../../README.md");

#[cfg(all(test, not(target_arch = "wasm32")))]
mod test {
    use std::convert::{TryFrom, TryInto};

    use crate::{self as mogwai, channel::broadcast, prelude::Component, ssr::SsrElement};
    use mogwai::{
        builder::ViewBuilder,
        channel::broadcast::*,
        event::DomEvent,
        futures::{Contravariant, IntoSenderSink, StreamExt},
        macros::*,
        view::{Dom, View},
    };
    use web_sys::Event;

    #[test]
    fn capture_view() {
        let (tx, mut rx) = broadcast::bounded::<Dom>(1);
        let _view = view! {
            <div>
                <pre
                 capture:view = tx.sink() >
                    "Tak :)"
                </pre>
            </div>
        };

        futures::executor::block_on(async move {
            let dom = rx.next().await.unwrap();
            assert_eq!(String::from(&dom), "<pre>Tak :)</pre>");
        });
    }

    #[cfg(feature = "never")]
    #[test]
    fn capture_struct_view() {
        struct_view! {
            <ViewFacade>
                <div>
                    <pre
                     capture:view = get_pre >
                        "Tak :)"
                    </pre>
                </div>
            </ViewFacade>
        }

        let (facade, builder):(ViewFacade<Dom>, _) = ViewFacade::new();
        let _ = Component::from(builder).build().unwrap();

        futures::executor::block_on(async move {
            let dom:Dom = facade.get_pre().await.unwrap();
            assert_eq!(String::from(&dom), "<pre>Tak :)</pre>");
        });
    }

    #[test]
    fn test_append() {
        let ns = "http://www.w3.org/2000/svg";
        let _bldr = mogwai::builder::ViewBuilder::element("svg")
            .with_namespace(ns)
            .with_single_attrib_stream("width", "100")
            .with_single_attrib_stream("height", "100")
            .append(
                mogwai::builder::ViewBuilder::element("circle")
                    .with_namespace(ns)
                    .with_single_attrib_stream("cx", "50")
                    .with_single_attrib_stream("cy", "50")
                    .with_single_attrib_stream("r", "40")
                    .with_single_attrib_stream("stroke", "green")
                    .with_single_attrib_stream("stroke-width", "4")
                    .with_single_attrib_stream("fill", "yellow")
                    as mogwai::builder::ViewBuilder<mogwai::view::Dom>,
            ) as mogwai::builder::ViewBuilder<mogwai::view::Dom>;
    }

    #[test]
    fn input() {
        let _ = builder! {
            <input boolean:checked=true />
        };
    }

    #[test]
    fn append_works() {
        let (tx, rx) = broadcast::bounded::<()>(1);
        let _ = builder! {
            <div window:load=tx.sink().contra_map(|_:DomEvent| ())>{("", rx.map(|()| "Loaded!".to_string()))}</div>
        };
    }

    #[test]
    fn cast_type_in_builder() {
        let _div = builder! {
            <div cast:type=mogwai::view::Dom id="hello">"Inner Text"</div>
        };
    }

    #[test]
    fn can_append_vec() {
        let _div: ViewBuilder<Dom> =
            ViewBuilder::element("div").append(vec![ViewBuilder::element("p")]);
    }

    #[test]
    fn can_append_option() {
        let _div: ViewBuilder<Dom> =
            ViewBuilder::element("div").append(None as Option<ViewBuilder<Dom>>);
    }

    #[test]
    fn fragments() {
        let vs: Vec<ViewBuilder<Dom>> = builder! {
            <div>"hello"</div>
            <div>"hola"</div>
            <div>"kia ora"</div>
        };

        let s: ViewBuilder<Dom> = builder! {
            <section>{vs}</section>
        };
        let view: View<Dom> = s.try_into().unwrap();
        assert_eq!(
            String::from(view).as_str(),
            "<section><div>hello</div> <div>hola</div> <div>kia ora</div></section>"
        );
    }

    #[test]
    fn post_build_manual() {
        let (tx, _rx) = broadcast::<()>(1);

        let _div = mogwai::builder::ViewBuilder::element("div")
            .with_single_attrib_stream("id", "hello")
            .with_post_build(move |_: &mut Dom| {
                let _ = tx.try_broadcast(()).unwrap();
            })
            .with_child(mogwai::builder::ViewBuilder::text("Hello"));
    }

    #[test]
    fn post_build_rsx() {
        let (tx, mut rx) = broadcast::<()>(1);

        let _div = view! {
            <div id="hello" post:build=move |_| {
                let _ = tx.try_broadcast(()).unwrap();
            }>
                "Hello"
            </div>
        };

        smol::block_on(async move {
            rx.recv().await.unwrap();
        });
    }

    #[test]
    fn can_construct_text_builder_from_tuple() {
        let (_tx, rx) = broadcast::<String>(1);
        let _div: View<Dom> = view! {
            <div>{("initial", rx)}</div>
        };
    }

    #[test]
    fn ssr_properties_overwrite() {
        let el: SsrElement<Event> = mogwai::ssr::SsrElement::element("div");
        el.set_style("float", "right").unwrap();
        assert_eq!(el.html_string(), r#"<div style="float: right;"></div>"#);

        el.set_style("float", "left").unwrap();
        assert_eq!(el.html_string(), r#"<div style="float: left;"></div>"#);

        el.set_style("width", "100px").unwrap();
        assert_eq!(
            el.html_string(),
            r#"<div style="float: left; width: 100px;"></div>"#
        );
    }

    #[test]
    fn ssr_attrib_overwrite() {
        let el: SsrElement<Event> = mogwai::ssr::SsrElement::element("div");

        el.set_attrib("class", Some("my_class")).unwrap();
        assert_eq!(el.html_string(), r#"<div class="my_class"></div>"#);

        el.set_attrib("class", Some("your_class")).unwrap();
        assert_eq!(el.html_string(), r#"<div class="your_class"></div>"#);
    }

    #[test]
    pub fn can_alter_ssr_views() {
        let (tx_text, rx_text) = broadcast::<String>(1);
        let (tx_style, rx_style) = broadcast::<String>(1);
        let (tx_class, rx_class) = broadcast::<String>(1);
        let view = view! {
            <div style:float=("left", rx_style)><p class=("p_class", rx_class)>{("here", rx_text)}</p></div>
        };
        assert_eq!(
            String::from(&view),
            r#"<div style="float: left;"><p class="p_class">here</p></div>"#
        );

        let _ = tx_text.try_broadcast("there".to_string()).unwrap();
        smol::block_on(async { broadcast::until_empty(&tx_text).await });

        assert_eq!(
            String::from(&view),
            r#"<div style="float: left;"><p class="p_class">there</p></div>"#
        );

        let _ = tx_style.try_broadcast("right".to_string()).unwrap();
        smol::block_on(async { broadcast::until_empty(&tx_style).await });

        assert_eq!(
            String::from(&view),
            r#"<div style="float: right;"><p class="p_class">there</p></div>"#
        );

        let _ = tx_class.try_broadcast("my_p_class".to_string()).unwrap();
        smol::block_on(async { broadcast::until_empty(&tx_class).await });

        assert_eq!(
            String::from(&view),
            r#"<div style="float: right;"><p class="my_p_class">there</p></div>"#
        );
    }

    #[test]
    pub fn can_use_string_stream_as_child() {
        use mogwai::futures::StreamExt;
        let clicks = futures::stream::iter(vec![0, 1, 2]);
        let bldr = builder! {
            <span>
            {
                ViewBuilder::text(clicks.map(|clicks| match clicks {
                    1 => "1 click".to_string(),
                    n => format!("{} clicks", n),
                }))
            }
            </span>
        };
        let _ = View::try_from(bldr).unwrap();
    }

    #[test]
    fn test_use_tx_in_logic_loop() {
        smol::block_on(async {
            let (tx, mut rx) = broadcast::bounded::<()>(1);
            let (tx_end, mut rx_end) = broadcast::bounded::<()>(1);
            let tx_logic = tx.clone();
            mogwai::spawn(async move {
                let mut ticks = 0u32;
                loop {
                    match rx.next().await {
                        Some(()) => {
                            ticks += 1;
                            match ticks {
                                1 => {
                                    // while in the loop, queue another
                                    tx.broadcast(()).await.unwrap();
                                }
                                _ => break,
                            }
                        }

                        None => break,
                    }
                }
                assert_eq!(ticks, 2);
                tx_end.broadcast(()).await.unwrap();
            });
            tx_logic.broadcast(()).await.unwrap();
            rx_end.next().await.unwrap();
        });
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod test {
    use async_broadcast::broadcast;
    use futures::stream::once;
    use std::{
        convert::{TryFrom, TryInto},
        ops::Bound,
    };
    use wasm_bindgen::JsCast;
    use wasm_bindgen_test::*;
    use web_sys::HtmlElement;

    use crate::{
        self as mogwai,
        builder::ViewBuilder,
        channel::{self, mpmc::bounded},
        futures::{IntoSenderSink, StreamExt},
        macros::*,
        patch::ListPatch,
        view::{Dom, EitherExt, View},
    };

    wasm_bindgen_test_configure!(run_in_browser);

    type DomBuilder = ViewBuilder<Dom>;

    #[wasm_bindgen_test]
    fn can_create_text_view_node_from_str() {
        let _view: View<Dom> = ViewBuilder::text("Hello!").try_into().unwrap();
    }

    #[wasm_bindgen_test]
    fn can_create_text_view_node_from_string() {
        let _view: View<Dom> = ViewBuilder::text("Hello!".to_string()).try_into().unwrap();
    }

    #[wasm_bindgen_test]
    fn can_create_text_view_node_from_stream() {
        let s = once(async { "Hello!".to_string() });
        let _view: View<Dom> = ViewBuilder::text(s).try_into().unwrap();
    }

    #[wasm_bindgen_test]
    fn can_create_text_view_node_from_string_and_stream() {
        let s = "Hello!".to_string();
        let st = once(async { "Goodbye!".to_string() });
        let _view: View<Dom> = ViewBuilder::text((s, st)).try_into().unwrap();
    }

    #[wasm_bindgen_test]
    fn can_create_text_view_node_from_str_and_stream() {
        let st = once(async { "Goodbye!".to_string() });
        let _view: View<Dom> = ViewBuilder::text(("Hello!", st)).try_into().unwrap();
    }

    #[wasm_bindgen_test]
    async fn can_nest_created_text_view_node() {
        let view: View<Dom> = ViewBuilder::element("div")
            .with_child(ViewBuilder::text("Hello!"))
            .with_single_attrib_stream("id", "view1")
            .with_single_style_stream("color", "red")
            .with_single_style_stream(
                "width",
                futures::stream::once(async { "100px".to_string() }),
            )
            .try_into()
            .unwrap();

        assert_eq!(
            String::from(&view).as_str(),
            r#"<div id="view1" style="color: red; width: 100px;">Hello!</div>"#
        );
    }

    #[wasm_bindgen_test]
    async fn ssr_can_nest_created_text_view_node() {
        let view: View<Dom> = ViewBuilder::element("div")
            .with_child(ViewBuilder::text("Hello!"))
            .with_single_attrib_stream("id", "view1")
            .with_single_style_stream("color", "red")
            .with_single_style_stream(
                "width",
                futures::stream::once(async { "100px".to_string() }),
            )
            .try_into()
            .unwrap();

        assert_eq!(
            String::from(&view).as_str(),
            r#"<div id="view1" style="color: red; width: 100px;">Hello!</div>"#
        );
    }

    #[wasm_bindgen_test]
    async fn can_use_rsx_to_make_builder() {
        let (tx, _) = mogwai::channel::mpmc::bounded::<web_sys::Event>(1);

        let rsx: DomBuilder = builder! {
            <div id="view_zero" style:background_color="red">
                <pre on:click=tx.sink()>"this has text"</pre>
            </div>
        };
        let rsx_view = View::try_from(rsx).unwrap();

        let manual: DomBuilder = mogwai::builder::ViewBuilder::element("div")
            .with_single_attrib_stream("id", "view_zero")
            .with_single_style_stream("background-color", "red")
            .with_child(
                mogwai::builder::ViewBuilder::element("pre")
                    .with_event("click", tx.sink())
                    .with_child(mogwai::builder::ViewBuilder::text("this has text")),
            );
        let manual_view = View::try_from(manual).unwrap();

        assert_eq!(String::from(&rsx_view), String::from(&manual_view));
    }

    #[wasm_bindgen_test]
    async fn viewbuilder_child_order() {
        let v: View<Dom> = view! {
            <div>
                <p id="one">"i am 1"</p>
                <p id="two">"i am 2"</p>
                <p id="three">"i am 3"</p>
            </div>
        };

        let val = v.inner.inner_read().left().unwrap();
        let nodes = val.dyn_ref::<web_sys::Node>().unwrap().child_nodes();
        let len = nodes.length();
        assert_eq!(len, 3);
        let mut ids = vec![];
        for i in 0..len {
            let el = nodes
                .get(i)
                .unwrap()
                .dyn_into::<web_sys::Element>()
                .unwrap();
            ids.push(el.id());
        }

        assert_eq!(ids.as_slice(), ["one", "two", "three"]);
    }

    #[wasm_bindgen_test]
    fn gizmo_ref_as_child() {
        // Since the pre tag is dropped after the scope block the last assert should
        // show that the div tag has no children.
        let div = {
            let pre: View<Dom> = view! { <pre>"this has text"</pre> };
            let pre_inner = pre.inner.inner_read().left().unwrap();
            let pre_node = pre_inner.dyn_ref::<web_sys::Node>().unwrap();
            let div: View<Dom> = view! { <div id="parent"></div> };
            let div_inner = div.inner.inner_read().left().unwrap();
            let div_node = div_inner.dyn_ref::<web_sys::Node>().unwrap();
            div_node.append_child(pre_node).unwrap();
            assert!(
                div_node.first_child().is_some(),
                "parent does not contain in-scope child"
            );
            drop(div_inner);
            div
        };

        let div_inner = div.inner.inner_read().left().unwrap();
        assert!(
            div_inner
                .dyn_ref::<web_sys::Node>()
                .unwrap()
                .first_child()
                .is_none(),
            "parent contains out-of-scope child"
        );
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
            assert!(
                div.clone_as::<web_sys::HtmlElement>()
                    .unwrap()
                    .first_child()
                    .is_some(),
                "could not add child gizmo"
            );
            div
        };
        assert!(
            div.clone_as::<web_sys::HtmlElement>()
                .unwrap()
                .first_child()
                .is_some(),
            "could not keep hold of child gizmo"
        );
        assert_eq!(
            div.clone_as::<web_sys::HtmlElement>()
                .unwrap()
                .child_nodes()
                .length(),
            1,
            "parent is missing static_gizmo"
        );
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
        let el = root.clone_as::<web_sys::HtmlElement>().unwrap();
        if let Some(branch) = el.first_child() {
            if let Some(leaf) = branch.first_child() {
                if let Some(leaf) = leaf.dyn_ref::<web_sys::Element>() {
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
            &div.clone_as::<web_sys::Element>().unwrap().outer_html(),
            "<div>here is some text 66 &lt;- number</div>"
        );
    }

    #[wasm_bindgen_test]
    async fn rx_attribute_jsx() {
        let (tx, rx) = bounded::<String>(1);
        let div = view! {
            <div class=("now", rx) />
        };
        let div_el: web_sys::HtmlElement = div.clone_as::<web_sys::HtmlElement>().unwrap();
        assert_eq!(div_el.outer_html(), r#"<div class="now"></div>"#);

        tx.send("later".to_string()).await.unwrap();
        mogwai::channel::mpmc::until_empty(&tx).await;

        assert_eq!(div_el.outer_html(), r#"<div class="later"></div>"#);
    }

    #[wasm_bindgen_test]
    async fn rx_style_jsx() {
        let (tx, rx) = broadcast::<String>(1);
        let div = view! { <div style:display=("block", rx) /> };
        let div_el = div.clone_as::<web_sys::HtmlElement>().unwrap();
        assert_eq!(
            div_el.outer_html(),
            r#"<div style="display: block;"></div>"#
        );

        tx.broadcast("none".to_string()).await.unwrap();
        mogwai::channel::broadcast::until_empty(&tx).await;

        assert_eq!(div_el.outer_html(), r#"<div style="display: none;"></div>"#);
    }

    #[wasm_bindgen_test]
    async fn contra_map_events() {
        let (tx, mut rx) = broadcast::<()>(1);
        let _div = view! {
            <div id="hello" post:build=move |_| {
                let _ = tx.try_broadcast(()).unwrap();
            }>
                "Hello there"
            </div>
        };

        let () = rx.recv().await.unwrap();
    }

    #[wasm_bindgen_test]
    pub async fn rx_text() {
        let (tx, rx) = broadcast::<String>(1);

        let div: View<Dom> = view! {
            <div>{("initial", rx)}</div>
        };

        let el = div.clone_as::<web_sys::HtmlElement>().unwrap();
        assert_eq!(el.inner_text().as_str(), "initial");

        tx.broadcast("after".into()).await.unwrap();
        mogwai::channel::broadcast::until_empty(&tx).await;

        assert_eq!(el.inner_text(), "after");
    }

    #[wasm_bindgen_test]
    async fn tx_on_click() {
        use mogwai::futures::StreamExt;
        let (tx, rx) = mogwai::channel::mpmc::bounded(1);

        log::info!("test!");
        let rx = rx.scan(0, |n: &mut i32, _: web_sys::Event| {
            log::info!("event!");
            *n += 1;
            let r = Some(if *n == 1 {
                "Clicked 1 time".to_string()
            } else {
                format!("Clicked {} times", *n)
            });
            futures::future::ready(r)
        });

        let button = view! {
            <button on:click=tx.sink()>{("Clicked 0 times", rx)}</button>
        };

        let el = button.clone_as::<web_sys::HtmlElement>().unwrap();
        assert_eq!(el.inner_html(), "Clicked 0 times");

        el.click();
        mogwai::channel::mpmc::until_empty(&tx).await;
        let _ = mogwai::time::wait_approx(1000.0).await;

        assert_eq!(el.inner_html(), "Clicked 1 time");
    }

    //fn nice_compiler_error() {
    //    let _div = view! {
    //        <div unknown:colon:thing="not ok" />
    //    };
    //}

    #[wasm_bindgen_test]
    async fn can_wait_approximately() {
        let millis_waited = mogwai::time::wait_approx(22.0).await;
        assert!(millis_waited >= 21.0);
    }

    #[wasm_bindgen_test]
    async fn can_patch_children() {
        let (tx, rx) = bounded::<ListPatch<ViewBuilder<Dom>>>(1);
        let view = view! {
            <ol id="main" patch:children=rx>
                <li>"Zero"</li>
                <li>"One"</li>
            </ol>
        };

        let dom: HtmlElement = view.clone_as::<HtmlElement>().unwrap();
        view.run().unwrap();

        assert_eq!(
            dom.outer_html().as_str(),
            r#"<ol id="main"><li>Zero</li><li>One</li></ol>"#
        );

        tx.try_send(ListPatch::push(builder! {<li>"Two"</li>}))
            .unwrap();
        channel::mpmc::until_empty(&tx).await;
        assert_eq!(
            dom.outer_html().as_str(),
            r#"<ol id="main"><li>Zero</li><li>One</li><li>Two</li></ol>"#
        );

        tx.try_send(ListPatch::splice(0..1, None.into_iter()))
            .unwrap();
        channel::mpmc::until_empty(&tx).await;
        assert_eq!(
            dom.outer_html().as_str(),
            r#"<ol id="main"><li>One</li><li>Two</li></ol>"#
        );

        tx.try_send(ListPatch::splice(
            0..0,
            Some(builder! {<li>"Zero"</li>}).into_iter(),
        ))
        .unwrap();
        channel::mpmc::until_empty(&tx).await;
        assert_eq!(
            dom.outer_html().as_str(),
            r#"<ol id="main"><li>Zero</li><li>One</li><li>Two</li></ol>"#
        );

        tx.try_send(ListPatch::splice(2..3, None.into_iter()))
            .unwrap();
        channel::mpmc::until_empty(&tx).await;
        assert_eq!(
            dom.outer_html().as_str(),
            r#"<ol id="main"><li>Zero</li><li>One</li></ol>"#
        );

        tx.try_send(ListPatch::splice(
            0..0,
            Some(builder! {<li>"Negative One"</li>}).into_iter(),
        ))
        .unwrap();
        channel::mpmc::until_empty(&tx).await;
        assert_eq!(
            dom.outer_html().as_str(),
            r#"<ol id="main"><li>Negative One</li><li>Zero</li><li>One</li></ol>"#
        );

        tx.try_send(ListPatch::Pop).unwrap();
        channel::mpmc::until_empty(&tx).await;
        assert_eq!(
            dom.outer_html().as_str(),
            r#"<ol id="main"><li>Negative One</li><li>Zero</li></ol>"#
        );

        tx.try_send(ListPatch::splice(
            1..2,
            Some(builder! {<li>"One"</li>}).into_iter(),
        ))
        .unwrap();
        channel::mpmc::until_empty(&tx).await;
        assert_eq!(
            dom.outer_html().as_str(),
            r#"<ol id="main"><li>Negative One</li><li>One</li></ol>"#
        );

        use std::ops::RangeBounds;
        let range = 0..;
        let (start, end) = (range.start_bound(), range.end_bound());
        assert_eq!(start, Bound::Included(&0));
        assert_eq!(end, Bound::Unbounded);
        assert!((start, end).contains(&1));

        tx.try_send(ListPatch::splice(0.., None.into_iter()))
            .unwrap();
        channel::mpmc::until_empty(&tx).await;
        assert_eq!(dom.outer_html().as_str(), r#"<ol id="main"></ol>"#);
    }

    #[wasm_bindgen_test]
    async fn can_patch_children_into() {
        let (tx, rx) = bounded::<ListPatch<String>>(1);
        let view = view! {
            <p id="main" patch:children=rx.map(|p| p.map(|s| ViewBuilder::text(s)))>
                "Zero ""One"
            </p>
        };

        let dom: HtmlElement = view.clone_as().unwrap();
        view.run().unwrap();

        assert_eq!(dom.outer_html().as_str(), r#"<p id="main">Zero One</p>"#);

        tx.send(ListPatch::splice(
            0..0,
            std::iter::once("First ".to_string()),
        ))
        .await
        .unwrap();
        mogwai::channel::mpmc::until_empty(&tx).await;
        assert_eq!(
            dom.outer_html().as_str(),
            r#"<p id="main">First Zero One</p>"#
        );

        tx.send(ListPatch::splice(.., std::iter::empty()))
            .await
            .unwrap();
        mogwai::channel::mpmc::until_empty(&tx).await;
        assert_eq!(dom.outer_html().as_str(), r#"<p id="main"></p>"#);
    }

    #[wasm_bindgen_test]
    pub fn can_use_string_stream_as_child() {
        let clicks = futures::stream::iter(vec![0, 1, 2]);
        let bldr = builder! {
            <span>
            {
                ViewBuilder::text(clicks.map(|clicks| match clicks {
                    1 => "1 click".to_string(),
                    n => format!("{} clicks", n),
                }))
            }
            </span>
        };
        let _ = View::try_from(bldr).unwrap();
    }
}
