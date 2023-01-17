//! # Mogwai
//!
//! Mogwai is library for multi-domain user interface development using sinks
//! and streams.
//!
//! Its goals are simple:
//! * provide a declarative approach to creating and managing interface nodes,
//!   without a bias towards a specific UI domain (ie web, games, desktop
//!   applications, mobile)
//! * encapsulate component state and compose components easily
//! * explicate mutations and updates
//! * feel snappy
//!
//! ## Javascript/Browser DOM
//! This library is specific to writing mogwai apps to run in the browser via
//! WASM.
//!
//! ## Learn more
//! Please check out the [introduction module](an_introduction).
//!
//! ## Acronyms
//! If you're wondering what the acronym "mogwai" stands for, here is a table of
//! options that work well, depending on the domain. It's fun to mix and match.
//!
//! | M           | O         | G           | W      | A             | I
//! |
//! |-------------|-----------|-------------|--------|---------------|--------------|
//! | minimal     | obvious   | graphical   | web    | application   | interface
//! | | modular     | operable  | graphable   | widget |               |
//! | | mostly      |           | gui         | work   |               |
//! |
//!
//! ## JavaScript interoperability
//! This library is a thin layer on top of the [web-sys](https://crates.io/crates/web-sys)
//! crate which provides raw bindings to _tons_ of browser web APIs.
//! Many of the DOM specific structs, enums and traits come from `web-sys`.
//! It is important to understand the [`JsCast`](../prelude/trait.JsCast.html)
//! trait for writing web apps in Rust. Specifically its `dyn_into` and
//! `dyn_ref` functions are the primary way to cast JavaScript values as
//! specific Javascript types.
pub mod an_introduction;
pub mod event;
pub mod utils;
pub mod view;
pub use mogwai_macros::{builder, html, rsx};

pub mod core {
    //! Re-export of the mogwai library.
    pub use mogwai::*;
}

pub mod prelude {
    //! Re-exports for convenience.
    pub use super::{event::*, utils::*, view::*};
    pub use mogwai::prelude::*;
    pub use std::convert::TryFrom;
}

#[cfg(doctest)]
doc_comment::doctest!("../../../README.md", readme);

#[cfg(all(test, not(target_arch = "wasm32")))]
mod nonwasm {
    use std::sync::Arc;

    use async_executor::Executor;

    use crate as mogwai_dom;
    use crate::{
        core::{
            channel::{broadcast, mpsc},
            time::repeat_times,
        },
        prelude::*,
    }; // for macro features

    #[test]
    fn component_nest() {
        let click_output = mogwai::relay::Output::default();
        let my_button_component = rsx! {
            button(on:click = click_output.sink().contra_map(|_: AnyEvent| ())) {"Click me!"}
        }
        .with_task(async move {
            loop {
                if let Some(()) = click_output.get().await {
                    println!("click received");
                } else {
                    println!("click event stream was dropped");
                    break;
                }
            }
        });

        let _my_div = Dom::try_from(rsx! {
            div() {
                h1 { "Click to print a line" }
                {my_button_component}
            }
        })
        .unwrap();
    }

    #[test]
    fn capture_view_channel_md() {
        // ANCHOR: capture_view_channel_md
        use mogwai_dom::{core::channel::broadcast, prelude::*};

        futures::executor::block_on(async {
            println!("using channels");
            let (tx, mut rx) = broadcast::bounded::<Dom>(1.try_into().unwrap());

            let builder = rsx! {
                div(){
                    button(capture:view = tx) { "Click" }
                }
            };

            let div = Dom::try_from(builder).unwrap();

            div.run_while(async move {
                let _button: Dom = rx.next().await.unwrap();
            })
            .await
            .unwrap();
        });
        // ANCHOR_END: capture_view_channel_md
    }

    #[test]
    fn capture_view_captured_md() {
        // ANCHOR_END: capture_view_captured_md
        use mogwai_dom::prelude::*;

        futures::executor::block_on(async {
            println!("using captured");
            let captured: Captured<Dom> = Captured::default();

            let builder = html! {
                <div><button capture:view = captured.sink()>"Click"</button></div>
            };

            let div = Dom::try_from(builder).unwrap();

            div.run_while(async move {
                let _button: Dom = captured.get().await;
            })
            .await
            .unwrap();
        });
        // ANCHOR_END: capture_view_captured_md
    }

    #[test]
    fn capture_view() {
        futures::executor::block_on(async move {
            let (tx, mut rx) = broadcast::bounded::<SsrDom>(1.try_into().unwrap());
            let view = SsrDom::try_from(html! {
                <div>
                    <pre
                    capture:view = tx >
                    "Tack :)"
                    </pre>
                    </div>
            })
            .unwrap();

            view.executor
                .run(async {
                    let dom = rx.next().await.unwrap();
                    assert_eq!(dom.html_string().await, "<pre>Tack :)</pre>");
                })
                .await;
        });
    }

    #[test]
    fn test_append() {
        let ns = "http://www.w3.org/2000/svg";
        let _bldr = ViewBuilder::element_ns("svg", ns)
            .with_single_attrib_stream("width", "100")
            .with_single_attrib_stream("height", "100")
            .append(
                ViewBuilder::element_ns("circle", ns)
                    .with_single_attrib_stream("cx", "50")
                    .with_single_attrib_stream("cy", "50")
                    .with_single_attrib_stream("r", "40")
                    .with_single_attrib_stream("stroke", "green")
                    .with_single_attrib_stream("stroke-width", "4")
                    .with_single_attrib_stream("fill", "yellow"),
            );
    }

    #[test]
    fn input() {
        let _ = html! {
            <input boolean:checked=true />
        };
    }

    #[test]
    fn append_works() {
        let (tx, rx) = broadcast::bounded::<()>(1.try_into().unwrap());
        let _ = rsx! {
            div( window:load=tx.contra_map(|_:DomEvent| ()) ) {
                {("", rx.map(|()| "Loaded!".to_string()))}
            }
        };
    }

    #[test]
    fn can_append_vec() {
        let _div: ViewBuilder = ViewBuilder::element("div").append(vec![ViewBuilder::element("p")]);
    }

    #[test]
    fn can_append_option() {
        let _div: ViewBuilder = ViewBuilder::element("div").append(None as Option<ViewBuilder>);
    }

    #[test]
    fn fragments() {
        let vs: Vec<ViewBuilder> = html! {
            <div>"hello"</div>
            <div>"hola"</div>
            <div>"kia ora"</div>
        };

        let s = html! {
            <section>{vs}</section>
        };
        let view = SsrDom::try_from(s).unwrap();
        futures::executor::block_on(async move {
            assert_eq!(
                view.html_string().await,
                "<section><div>hello</div> <div>hola</div> <div>kia ora</div></section>"
            );
        });
    }

    #[test]
    fn post_build_manual() {
        let (tx, _rx) = broadcast::bounded::<()>(1.try_into().unwrap());

        let _div = ViewBuilder::element("div")
            .with_single_attrib_stream("id", "hello")
            .with_post_build(move |_: &mut JsDom| {
                let _ = tx.inner.try_broadcast(())?;
                Ok(())
            })
            .append(ViewBuilder::text("Hello"));
    }

    #[test]
    fn post_build_rsx() {
        futures::executor::block_on(async {
            let (tx, mut rx) = broadcast::bounded::<()>(1.try_into().unwrap());

            let _div = SsrDom::try_from(rsx! {
                div(id="hello", post:build=move |_: &mut SsrDom| {
                    let _ = tx.inner.try_broadcast(())?;
                    Ok(())
                }) { "Hello" }
            })
            .unwrap();

            rx.recv().await.unwrap();
        });
    }

    #[test]
    fn can_construct_text_builder_from_tuple() {
        futures::executor::block_on(async {
            let (_tx, rx) = broadcast::bounded::<String>(1.try_into().unwrap());
            let _div = SsrDom::try_from(html! {
                <div>{("initial", rx)}</div>
            })
            .unwrap();
        });
    }

    #[test]
    fn ssr_properties_overwrite() {
        let executor = Arc::new(Executor::default());
        futures::executor::block_on(async {
            let el: SsrDom = SsrDom::element(executor.clone(), "div");
            el.set_style("float", "right").unwrap();
            assert_eq!(
                el.html_string().await,
                r#"<div style="float: right;"></div>"#
            );

            el.set_style("float", "left").unwrap();
            assert_eq!(
                el.html_string().await,
                r#"<div style="float: left;"></div>"#
            );

            el.set_style("width", "100px").unwrap();
            assert_eq!(
                el.html_string().await,
                r#"<div style="float: left; width: 100px;"></div>"#
            );
        });
    }

    #[test]
    fn ssr_attrib_overwrite() {
        let executor = Arc::new(Executor::default());
        futures::executor::block_on(async {
            let el: SsrDom = SsrDom::element(executor.clone(), "div");

            el.set_attrib("class", Some("my_class")).unwrap();
            assert_eq!(el.html_string().await, r#"<div class="my_class"></div>"#);

            el.set_attrib("class", Some("your_class")).unwrap();
            assert_eq!(el.html_string().await, r#"<div class="your_class"></div>"#);
        });
    }

    async fn wait_eq(
        t: &str,
        secs: f64,
        view: &SsrDom,
    ) {
        let start = mogwai::time::now() / 1000.0;
        let timeout = secs;
        loop {
            let s = view.html_string().await;
            let now = mogwai::time::now();
            if (now - start) >= timeout {
                panic!("timeout {}s: {:?} != {:?} ", timeout, t, s);
            } else if t.trim() == s.trim() {
                return;
            }
            mogwai_dom::core::time::wait_one_frame().await;
        }
    }

    #[test]
    pub fn ssr_simple_update() {
        futures_lite::future::block_on(async {
            let mut text = Input::<String>::default();

            let view = SsrDom::try_from(ViewBuilder::text(("hello", text.stream().unwrap()))).unwrap();
            let v = view.clone();
            view.run_while(async move {
                wait_eq(r#"hello"#, 1.0, &v).await;
                text.set("goodbye").await.unwrap();
                wait_eq(r#"goodbye"#, 1.0, &v).await;
            }).await.unwrap();
        });
    }

    #[test]
    pub fn ssr_simple_nested_update() {
        futures_lite::future::block_on(async {
            let mut text = Input::<String>::default();

            let view = SsrDom::try_from(rsx!(
                p() {
                    {("hello", text.stream().unwrap())}
                }
            )).unwrap();
            let v = view.clone();
            view.run_while(async move {
                wait_eq(r#"<p>hello</p>"#, 1.0, &v).await;

                text.set("goodbye").await.unwrap();
                wait_eq(r#"<p>goodbye</p>"#, 1.0, &v).await;

                text.set("kia ora").await.unwrap();
                wait_eq(r#"<p>kia ora</p>"#, 1.0, &v).await;
            }).await.unwrap();
        });
    }

    #[test]
    pub fn ssr_simple_nested_with_two_inputs_update() {
        futures_lite::future::block_on(async {
            let mut text = Input::<String>::default();
            let mut class = Input::<String>::default();

            let view = SsrDom::try_from(rsx!(
                p(class=("p_class", class.stream().unwrap())) {
                    {("hello", text.stream().unwrap())}
                }
            )).unwrap();
            let v = view.clone();
            view.run_while(async move {
                wait_eq(r#"<p class="p_class">hello</p>"#, 1.0, &v).await;

                text.set("goodbye").await.unwrap();
                wait_eq(r#"<p class="p_class">goodbye</p>"#, 1.0, &v).await;

                class.set("my_p_class").await.unwrap();
                wait_eq(r#"<p class="my_p_class">goodbye</p>"#, 1.0, &v).await;
            }).await.unwrap();
        });
    }

    #[test]
    pub fn can_alter_ssr_views() {
        use mogwai::relay::*;
        futures_lite::future::block_on(async {
            let mut text = Input::<String>::default();
            let mut style = Input::<String>::default();
            let mut class = Input::<String>::default();

            let view = SsrDom::try_from(rsx! {
                div(style:float=("left", style.stream().unwrap())) {
                    p(class=("p_class", class.stream().unwrap())) {
                        {("here", text.stream().unwrap())}
                    }
                }
            })
            .unwrap();

            let v = view.clone();
            view.run_while(async move {
                    wait_eq(
                        r#"<div style="float: left;"><p class="p_class">here</p></div>"#,
                        1.0,
                        &v,
                    )
                    .await;

                    let _ = text.try_send("there".to_string()).unwrap();
                    wait_eq(
                        r#"<div style="float: left;"><p class="p_class">there</p></div>"#,
                        1.0,
                        &v
                    ).await;

                    let _ = style.try_send("right".to_string()).unwrap();
                    wait_eq(
                        r#"<div style="float: right;"><p class="p_class">there</p></div>"#,
                        1.0,
                        &v
                    ).await;

                    let _ = class.try_send("my_p_class".to_string()).unwrap();
                    wait_eq(
                        r#"<div style="float: right;"><p class="my_p_class">there</p></div>"#,
                        1.0,
                        &v
                    ).await;
                })
                .await.unwrap();
        });
    }

    #[test]
    fn can_use_string_stream_as_child() {
        futures::executor::block_on(async {
            let clicks = futures::stream::iter(vec![0, 1, 2]);
            let bldr = html! {
                <span>
                {
                    ViewBuilder::text(clicks.map(|clicks| match clicks {
                        1 => "1 click".to_string(),
                        n => format!("{} clicks", n),
                    }))
                }
                </span>
            };
            let _ = SsrDom::try_from(bldr).unwrap();
        });
    }

    #[test]
    fn test_use_tx_in_logic_loop() {
        futures::executor::block_on(async {
            let executor = Arc::new(Executor::default());
            let (tx, mut rx) = broadcast::bounded::<()>(1.try_into().unwrap());
            let (tx_end, mut rx_end) = broadcast::bounded::<()>(1.try_into().unwrap());
            let tx_logic = tx.clone();
            executor
                .spawn(async move {
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
                })
                .detach();
            executor
                .run(async {
                    tx_logic.broadcast(()).await.unwrap();
                    rx_end.next().await.unwrap();
                })
                .await;
        });
    }

    #[test]
    fn patch_children_rsx_md() {
        futures::executor::block_on(async {
            // ANCHOR: patch_children_rsx
            let (tx, rx) = mpsc::bounded(1);
            let my_view = SsrDom::try_from(html! {
                <div id="main" patch:children=rx>"Waiting for a patch message..."</div>
            })
            .unwrap();

            my_view
                .executor
                .run(async {
                    tx.send(ListPatch::drain()).await.unwrap();
                    // just as a sanity check we wait until the view has removed all child
                    // nodes
                    repeat_times(0.1, 10, || async {
                        my_view.html_string().await == r#"<div id="main"></div>"#
                    })
                    .await
                    .unwrap();

                    let other_viewbuilder = html! {
                        <h1>"Hello!"</h1>
                    };

                    tx.send(ListPatch::push(other_viewbuilder)).await.unwrap();
                    // now wait until the view has been patched with the new child
                    repeat_times(0.1, 10, || async {
                        let html_string = my_view.html_string().await;
                        html_string == r#"<div id="main"><h1>Hello!</h1></div>"#
                    })
                    .await
                    .unwrap();
                })
                .await;
            // ANCHOR_END:patch_children_rsx
        });
    }

    #[test]
    pub fn can_build_readme_button() {}
}

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm {
    use std::ops::Bound;

    use crate as mogwai_dom;
    use crate::{
        core::{
            channel::{broadcast, mpsc},
            time::*,
        },
        prelude::*,
        view::js::Hydrator,
    };
    use futures::stream;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_test::*;
    use web_sys::HtmlElement;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn can_create_text_view_node_from_str() {
        let _view: JsDom = ViewBuilder::text("Hello!").try_into().unwrap();
    }

    #[wasm_bindgen_test]
    async fn can_create_text_view_node_from_string() {
        let _view: JsDom = ViewBuilder::text("Hello!".to_string()).try_into().unwrap();
    }

    #[wasm_bindgen_test]
    async fn can_create_text_view_node_from_stream() {
        let s = stream::once(async { "Hello!".to_string() });
        let _view: JsDom = ViewBuilder::text(s).try_into().unwrap();
    }

    #[wasm_bindgen_test]
    async fn can_create_text_view_node_from_string_and_stream() {
        let s = "Hello!".to_string();
        let st = stream::once(async { "Goodbye!".to_string() });
        let _view: JsDom = ViewBuilder::text((s, st)).try_into().unwrap();
    }

    #[wasm_bindgen_test]
    async fn can_create_text_view_node_from_str_and_stream() {
        let st = stream::once(async { "Goodbye!".to_string() });
        let _view: JsDom = ViewBuilder::text(("Hello!", st)).try_into().unwrap();
    }

    #[wasm_bindgen_test]
    async fn can_nest_created_text_view_node() {
        let view: JsDom = ViewBuilder::element("div")
            .append(ViewBuilder::text("Hello!"))
            .with_single_attrib_stream("id", "view1")
            .with_single_style_stream("color", "red")
            .with_single_style_stream(
                "width",
                futures::stream::once(async { "100px".to_string() }),
            )
            .try_into()
            .unwrap();

        assert_eq!(
            view.html_string().await,
            r#"<div id="view1" style="color: red; width: 100px;">Hello!</div>"#
        );
    }

    #[wasm_bindgen_test]
    async fn ssr_can_nest_created_text_view_node() {
        let view: JsDom = ViewBuilder::element("div")
            .append(ViewBuilder::text("Hello!"))
            .with_single_attrib_stream("id", "view1")
            .with_single_style_stream("color", "red")
            .with_single_style_stream(
                "width",
                futures::stream::once(async { "100px".to_string() }),
            )
            .try_into()
            .unwrap();

        assert_eq!(
            view.html_string().await,
            r#"<div id="view1" style="color: red; width: 100px;">Hello!</div>"#
        );
    }

    #[wasm_bindgen_test]
    async fn can_use_rsx_to_make_builder() {
        let (tx, _) = broadcast::bounded::<AnyEvent>(1);

        let rsx = html! {
            <div id="view_zero" style:background_color="red">
                <pre on:click=tx.clone()>"this has text"</pre>
            </div>
        };
        let rsx_view: JsDom = rsx.try_into().unwrap();

        let manual = ViewBuilder::element("div")
            .with_single_attrib_stream("id", "view_zero")
            .with_single_style_stream("background-color", "red")
            .append(
                ViewBuilder::element("pre")
                    .with_event("click", "myself", tx)
                    .append(ViewBuilder::text("this has text")),
            );
        let manual_view: JsDom = manual.try_into().unwrap();

        assert_eq!(
            rsx_view.html_string().await,
            manual_view.html_string().await
        );
    }

    #[wasm_bindgen_test]
    async fn viewbuilder_child_order() {
        let v: JsDom = html! {
            <div>
                <p id="one">"i am 1"</p>
                <p id="two">"i am 2"</p>
                <p id="three">"i am 3"</p>
            </div>
        }
        .try_into()
        .unwrap();

        let nodes = v.dyn_ref::<web_sys::Node>().unwrap().child_nodes();
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
    async fn gizmo_as_child() {
        // Since the pre tag is *not* dropped after the scope block the last assert
        // should show that the div tag has a child.
        let div = {
            let div: JsDom = html! {
                <div id="parent-div">
                    <pre>"some text"</pre>
                    </div>
            }
            .try_into()
            .unwrap();
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
    async fn gizmo_tree() {
        let root: JsDom = html! {
            <div id="root">
                <div id="branch">
                    <div id="leaf">
                        "leaf"
                    </div>
                </div>
            </div>
        }
        .try_into()
        .unwrap();
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
    async fn gizmo_texts() {
        let div: JsDom = html! {
            <div>
                "here is some text "
            // i can use comments, yay!
            {&format!("{}", 66)}
            " <- number"
                </div>
        }
        .try_into()
        .unwrap();
        assert_eq!(
            &div.clone_as::<web_sys::Element>().unwrap().outer_html(),
            "<div>here is some text 66 &lt;- number</div>"
        );
    }

    #[wasm_bindgen_test]
    async fn rx_attribute_jsx() {
        let (tx, rx) = broadcast::bounded::<String>(1);
        let div: JsDom = html! {
            <div class=("now", rx) />
        }
        .try_into()
        .unwrap();
        let div_el: web_sys::HtmlElement = div.clone_as::<web_sys::HtmlElement>().unwrap();
        assert_eq!(div_el.outer_html(), r#"<div class="now"></div>"#);

        tx.broadcast("later".to_string()).await.unwrap();
        tx.until_empty().await;

        assert_eq!(div_el.outer_html(), r#"<div class="later"></div>"#);
    }

    #[wasm_bindgen_test]
    async fn rx_style_jsx() {
        let (tx, rx) = broadcast::bounded::<String>(1);
        let div: JsDom = html! { <div style:display=("block", rx) /> }
            .try_into()
            .unwrap();
        let div_el = div.clone_as::<web_sys::HtmlElement>().unwrap();
        assert_eq!(
            div_el.outer_html(),
            r#"<div style="display: block;"></div>"#
        );

        tx.broadcast("none".to_string()).await.unwrap();
        tx.until_empty().await;

        assert_eq!(div_el.outer_html(), r#"<div style="display: none;"></div>"#);
    }

    #[wasm_bindgen_test]
    async fn capture_view_and_contra_map() {
        let (tx, mut rx) = broadcast::bounded::<()>(1);
        let _div: JsDom = html! {
            <div id="hello" capture:view=tx.contra_map(|_: JsDom| ())>
                "Hello there"
            </div>
        }
        .try_into()
        .unwrap();

        let () = rx.recv().await.unwrap();
    }

    #[wasm_bindgen_test]
    pub async fn rx_text() {
        let (tx, rx) = broadcast::bounded::<String>(1);

        let div: JsDom = html! {
            <div>{("initial", rx)}</div>
        }
        .try_into()
        .unwrap();

        let el = div.clone_as::<web_sys::HtmlElement>().unwrap();
        assert_eq!(el.inner_text().as_str(), "initial");

        tx.broadcast("after".into()).await.unwrap();
        tx.until_empty().await;

        assert_eq!(el.inner_text(), "after");
    }

    #[wasm_bindgen_test]
    async fn tx_on_click() {
        let (tx, rx) = broadcast::bounded(1);

        log::info!("test!");
        let rx = rx.scan(0, |n: &mut i32, _: JsDomEvent| {
            log::info!("event!");
            *n += 1;
            Some(
                if *n == 1 {
                    "Clicked 1 time".to_string()
                } else {
                    format!("Clicked {} times", *n)
                },
            )
        });

        let button: JsDom = html! {
            <button on:click=tx.clone()>{("Clicked 0 times", rx)}</button>
        }
        .try_into()
        .unwrap();

        let el = button.clone_as::<web_sys::HtmlElement>().unwrap();
        assert_eq!(el.inner_html(), "Clicked 0 times");

        el.click();
        tx.until_empty().await;
        let _ = wait_millis(1000).await;

        assert_eq!(el.inner_html(), "Clicked 1 time");
    }

    //fn nice_compiler_error() {
    //    let _div = html! {
    //        <div unknown:colon:thing="not ok" />
    //    };
    //}

    #[wasm_bindgen_test]
    async fn can_patch_children() {
        let (tx, rx) = mpsc::bounded::<ListPatch<ViewBuilder>>(1);
        let view: JsDom = html! {
            <ol id="main" patch:children=rx>
                <li>"Zero"</li>
                <li>"One"</li>
            </ol>
        }
        .try_into()
        .unwrap();

        let dom: HtmlElement = view.clone_as::<HtmlElement>().unwrap();
        view.run().unwrap();

        wait_while(1.0, || {
            dom.outer_html().as_str() != r#"<ol id="main"><li>Zero</li><li>One</li></ol>"#
        })
        .await
        .unwrap();

        let html = r#"<ol id="main"><li>Zero</li><li>One</li><li>Two</li></ol>"#;
        tx.send(ListPatch::push(html! {<li>"Two"</li>}))
            .await
            .unwrap();
        let _ = wait_while(5.0, || dom.outer_html().as_str() != html).await;
        assert_eq!(html, dom.outer_html());

        tx.send(ListPatch::splice(0..1, None.into_iter()))
            .await
            .unwrap();
        wait_while(1.0, || {
            dom.outer_html().as_str() != r#"<ol id="main"><li>One</li><li>Two</li></ol>"#
        })
        .await
        .unwrap();

        tx.send(ListPatch::splice(
            0..0,
            Some(html! {<li>"Zero"</li>}).into_iter(),
        ))
        .await
        .unwrap();
        wait_while(1.0, || {
            dom.outer_html().as_str()
                != r#"<ol id="main"><li>Zero</li><li>One</li><li>Two</li></ol>"#
        })
        .await
        .unwrap();

        tx.send(ListPatch::splice(2..3, None.into_iter()))
            .await
            .unwrap();
        wait_while(1.0, || {
            dom.outer_html().as_str() != r#"<ol id="main"><li>Zero</li><li>One</li></ol>"#
        })
        .await
        .unwrap();

        tx.send(ListPatch::splice(
            0..0,
            Some(html! {<li>"Negative One"</li>}).into_iter(),
        ))
        .await
        .unwrap();
        wait_while(1.0, || {
            dom.outer_html().as_str()
                != r#"<ol id="main"><li>Negative One</li><li>Zero</li><li>One</li></ol>"#
        })
        .await
        .unwrap();

        tx.send(ListPatch::Pop).await.unwrap();
        wait_while(1.0, || {
            dom.outer_html().as_str() != r#"<ol id="main"><li>Negative One</li><li>Zero</li></ol>"#
        })
        .await
        .unwrap();

        tx.send(ListPatch::splice(
            1..2,
            Some(html! {<li>"One"</li>}).into_iter(),
        ))
        .await
        .unwrap();
        wait_while(1.0, || {
            dom.outer_html().as_str() != r#"<ol id="main"><li>Negative One</li><li>One</li></ol>"#
        })
        .await
        .unwrap();

        use std::ops::RangeBounds;
        let range = 0..;
        let (start, end) = (range.start_bound(), range.end_bound());
        assert_eq!(start, Bound::Included(&0));
        assert_eq!(end, Bound::Unbounded);
        assert!((start, end).contains(&1));

        tx.send(ListPatch::splice(0.., None.into_iter()))
            .await
            .unwrap();
        wait_while(1.0, || {
            dom.outer_html().as_str() != r#"<ol id="main"></ol>"#
        })
        .await
        .unwrap();
    }

    #[wasm_bindgen_test]
    async fn can_patch_children_into() {
        let (tx, rx) = mpsc::bounded::<ListPatch<String>>(1);
        let view: JsDom = html! {
            <p id="main" patch:children=rx.map(|p| p.map(|s| ViewBuilder::text(s)))>
                "Zero ""One"
            </p>
        }
        .try_into()
        .unwrap();

        let dom: HtmlElement = view.clone_as().unwrap();
        view.run().unwrap();

        assert_eq!(dom.outer_html().as_str(), r#"<p id="main">Zero One</p>"#);

        tx.send(ListPatch::splice(
            0..0,
            std::iter::once("First ".to_string()),
        ))
        .await
        .unwrap();
        wait_while(1.0, || {
            dom.outer_html().as_str() != r#"<p id="main">First Zero One</p>"#
        })
        .await
        .unwrap();

        tx.send(ListPatch::splice(.., std::iter::empty()))
            .await
            .unwrap();
        wait_while(1.0, || dom.outer_html().as_str() != r#"<p id="main"></p>"#)
            .await
            .unwrap();
    }

    #[wasm_bindgen_test]
    pub async fn can_use_string_stream_as_child() {
        let clicks = futures::stream::iter(vec![0, 1, 2]);
        let bldr = html! {
            <span>
            {
                ViewBuilder::text(clicks.map(|clicks| match clicks {
                    1 => "1 click".to_string(),
                    n => format!("{} clicks", n),
                }))
            }
            </span>
        };
        let _: JsDom = bldr.try_into().unwrap();
    }

    fn sendable<T: Send + Sync + 'static>(_: &T) {}

    #[wasm_bindgen_test]
    pub fn output_sendable() {
        let output: Output<JsDom> = Output::default();
        sendable(&output);

        wasm_bindgen_futures::spawn_local(async move {
            let _ = output;
        })
    }

    #[wasm_bindgen_test]
    async fn can_capture_with_captured() {
        let capture: Captured<JsDom> = Captured::default().clone();
        let b = html! {
            <div id="chappie" capture:view=capture.sink()></div>
        };
        let _: JsDom = b.try_into().unwrap();
        let dom = capture.get().await;
        assert_eq!(dom.html_string().await, r#"<div id="chappie"></div>"#);
    }

    #[wasm_bindgen_test]
    async fn can_hydrate_view() {
        console_log::init_with_level(log::Level::Trace).unwrap();

        let container = JsDom::try_from(html! {
            <div id="hydrator1"></div>
        })
        .unwrap();
        let container_el: HtmlElement = container.clone_as::<HtmlElement>().unwrap();
        container.run().unwrap();
        container_el.set_inner_html(r#"<div id="my_div"><p>inner text</p></div>"#);
        assert_eq!(
            container_el.inner_html().as_str(),
            r#"<div id="my_div"><p>inner text</p></div>"#
        );
        log::info!("built");

        let (tx_class, rx_class) = mpsc::bounded::<String>(1);
        let (tx_text, rx_text) = mpsc::bounded::<String>(1);
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

    #[wasm_bindgen_test]
    async fn can_capture_for_each() {
        let (tx, rx) = mpsc::bounded(1);
        let (tx_done, mut rx_done) = mpsc::bounded(1);
        let dom = JsDom::try_from(rsx! {
            input(
                type = "text",
                capture:for_each = (
                    rx.map(|n:usize| format!("{}", n)),
                    JsDom::try_to(web_sys::HtmlInputElement::set_value)
                )
            ) {}
        })
        .expect("could not build dom");

        wasm_bindgen_futures::spawn_local(async move {
            let mut n = 0;
            while n < 3 {
                tx.send(n).await.unwrap();
                n += 1;
            }
            tx_done.send(()).await.unwrap();
        });

        dom.run_while(async move {
            let _ = rx_done.next().await;
        })
        .await
        .unwrap();

        let value = dom
            .visit_as(|input: &web_sys::HtmlInputElement| input.value())
            .unwrap();
        assert_eq!("2", value.as_str());
    }
}

#[cfg(test)]
mod test {

    use crate as mogwai_dom;
    use crate::prelude::*;

    #[test]
    fn can_relay() {
        struct Thing {
            view: Output<Dom>,
            click: Output<()>,
            text: Input<String>,
        }

        impl Default for Thing {
            fn default() -> Self {
                Self {
                    view: Default::default(),
                    click: Default::default(),
                    text: Default::default(),
                }
            }
        }

        impl Thing {
            fn view(mut self) -> ViewBuilder {
                rsx! (
                    div(
                        capture:view=self.view.sink(),
                        on:click=self.click.sink().contra_map(|_: AnyEvent| ())
                    ) {
                        {("Hi", self.text.stream().unwrap())}
                    }
                )
                .with_task(async move {
                    let mut clicks = 0;
                    while let Some(()) = self.click.get().await {
                        clicks += 1;
                        self.text
                            .set(
                                if clicks == 1 {
                                    "1 click.".to_string()
                                } else {
                                    format!("{} clicks.", clicks)
                                },
                            )
                            .await
                            .unwrap_or_else(|_| panic!("could not set text"));
                    }
                })
            }
        }

        let thing: Dom = Dom::try_from(Thing::default().view()).unwrap();
        futures::executor::block_on(async move {
            thing
                .run_while(async {
                    let _ = crate::core::time::wait_millis(10).await;
                })
                .await
                .unwrap();
        });
    }

    #[test]
    fn can_capture_with_captured() {
        futures::executor::block_on(async move {
            let capture: Captured<SsrDom> = Captured::default();
            let b = rsx! {
                div(id="chappie", capture:view=capture.sink()){}
            };
            let dom = SsrDom::try_from(b).unwrap();
            dom.executor
                .run(async {
                    let dom = capture.get().await;
                    assert_eq!(dom.html_string().await, r#"<div id="chappie"></div>"#);
                })
                .await;
        });
    }

    #[test]
    fn how_to_set_properties() {
        let mut stream_input_value = Input::<String>::default();
        let _builder = rsx! {
            input(
                type = "text",
                id = "my_text",
                capture:for_each = (
                    stream_input_value.stream().unwrap(),
                    JsDom::try_to(web_sys::HtmlInputElement::set_value)
                )
            ){}
        };
    }
}
