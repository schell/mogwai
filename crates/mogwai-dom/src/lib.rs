//! # Mogwai
//!
//! Mogwai is library for multi-domain user interface development using sinks and streams.
//!
//! Its goals are simple:
//! * provide a declarative approach to creating and managing interface nodes, without
//!   a bias towards a specific UI domain (ie web, games, desktop applications, mobile)
//! * encapsulate component state and compose components easily
//! * explicate mutations and updates
//! * feel snappy
//!
//! ## Javascript/Browser DOM
//! This library is specific to writing mogwai apps to run in the browser via WASM.
//!
//! ## Learn more
//! Please check out the [introduction module](an_introduction).
//!
//! ## Acronyms
//! If you're wondering what the acronym "mogwai" stands for, here is a table of
//! options that work well, depending on the domain. It's fun to mix and match.
//!
//! | M           | O         | G           | W      | A             | I            |
//! |-------------|-----------|-------------|--------|---------------|--------------|
//! | minimal     | obvious   | graphical   | web    | application   | interface    |
//! | modular     | operable  | graphable   | widget |               |              |
//! | mostly      |           | gui         | work   |               |              |
//! Provides trait implementations and helper functions for running mogwai
//! html-based UI graphs in the browser and on a server.
//!
//! ## JavaScript interoperability
//! This library is a thin layer on top of the //! [web-sys](https://crates.io/crates/web-sys)
//! crate which provides raw bindings to _tons_ of browser web APIs.
//! Many of the DOM specific structs, enums and traits come from `web-sys`.
//! It is important to understand the [`JsCast`](../prelude/trait.JsCast.html) trait
//! for writing web apps in Rust. Specifically its `dyn_into` and `dyn_ref` functions
//! are the primary way to cast JavaScript values as specific Javascript types.
pub mod an_introduction;
pub mod event;
pub mod utils;
pub mod view;

pub mod core {
    //! Re-export of the mogwai library.
    pub use mogwai::*;
}

pub mod prelude {
    //! Re-exports for convenience.
    pub use super::event::*;
    pub use super::utils::*;
    pub use super::view::*;
    pub use mogwai::prelude::*;
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod nonwasm {
    use std::sync::Arc;

    use async_executor::Executor;

    use crate as mogwai_dom;
    use crate::{
        core::{
            channel::{broadcast, mpsc},
            time::wait_while_async,
        },
        prelude::*,
    }; // for macro features

    #[test]
    fn component_nest() {
        let click_output = mogwai::relay::Output::default();
        let my_button_component = rsx! {
            button(on:click = click_output.sink().contra_map(|_| ())) {"Click me!"}
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

        let _my_div: Dom = rsx! {
            div() {
                h1 { "Click to print a line" }
                {my_button_component}
            }
        }
        .build()
        .unwrap();
    }

    fn capture_view() {
        let (tx, mut rx) = broadcast::bounded::<Dom>(1);
        let view: Dom = html! {
            <div>
                <pre
                capture:view = tx >
                "Tack :)"
                </pre>
                </div>
        }
        .build()
        .unwrap();

        view.run_while(async move {
            let dom = rx.next().await.unwrap();
            assert_eq!(dom.html_string().await, "<pre>Tack :)</pre>");
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
                    .with_single_attrib_stream("fill", "yellow")
                    as ViewBuilder<JsDom>,
            ) as ViewBuilder<JsDom>;
    }

    #[test]
    fn input() {
        let _ = html! {
            <input boolean:checked=true />
        };
    }

    #[test]
    fn append_works() {
        let (tx, rx) = broadcast::bounded::<()>(1);
        let _ = rsx! {
            div( window:load=tx.contra_map(|_:DomEvent| ()) ) {
                {("", rx.map(|()| "Loaded!".to_string()))}
            }
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

    fn fragments() {
        let vs: Vec<ViewBuilder<Dom>> = html! {
            <div>"hello"</div>
            <div>"hola"</div>
            <div>"kia ora"</div>
        };

        let s = html! {
            <section>{vs}</section>
        };
        let view = s.build().unwrap();
        Executor::default().run(async move {
            assert_eq!(
                view.html_string().await,
                "<section><div>hello</div> <div>hola</div> <div>kia ora</div></section>"
            );
        });
    }

    #[test]
    fn post_build_manual() {
        let (tx, _rx) = broadcast::bounded::<()>(1);

        let _div = ViewBuilder::element("div")
            .with_single_attrib_stream("id", "hello")
            .with_post_build(move |_: &mut JsDom| {
                let _ = tx.inner.try_broadcast(()).unwrap();
            })
            .append(ViewBuilder::text("Hello"));
    }

    async fn post_build_rsx() {
        let executor = Arc::new(Executor::default());
        executor.run(async {
            let (tx, mut rx) = broadcast::bounded::<()>(1);

            let _div: SsrDom = rsx! {
                div(id="hello", post:build=move |_: &mut SsrDom| {
                    let _ = tx.inner.try_broadcast(())?;
                    Ok(())
                }) { "Hello" }
            }
            .build()
            .unwrap();

            rx.recv().await.unwrap();
        });
    }

    async fn can_construct_text_builder_from_tuple() {
        let executor = Arc::new(Executor::default());
        executor.run(async {
            let (_tx, rx) = broadcast::bounded::<String>(1);
            let _div: SsrDom = html! {
                <div>{("initial", rx)}</div>
            }
            .build()
            .unwrap();
        });
    }

    fn ssr_properties_overwrite() {
        let executor = Arc::new(Executor::default());
        executor.run(async {
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

    fn ssr_attrib_overwrite() {
        let executor = Arc::new(Executor::default());
        executor.run(async {
            let el: SsrDom = SsrDom::element(executor.clone(), "div");

            el.set_attrib("class", Some("my_class")).unwrap();
            assert_eq!(el.html_string().await, r#"<div class="my_class"></div>"#);

            el.set_attrib("class", Some("your_class")).unwrap();
            assert_eq!(el.html_string().await, r#"<div class="your_class"></div>"#);
        });
    }

    pub fn can_alter_ssr_views() {
        let executor = Arc::new(Executor::default());
        executor.run(async {
            let (tx_text, rx_text) = broadcast::bounded::<String>(1);
            let (tx_style, rx_style) = broadcast::bounded::<String>(1);
            let (tx_class, rx_class) = broadcast::bounded::<String>(1);
            let view: SsrDom = html! {
                <div style:float=("left", rx_style)><p class=("p_class", rx_class)>{("here", rx_text)}</p></div>
            }.build().unwrap();
            assert_eq!(
                view.html_string().await,
                r#"<div style="float: left;"><p class="p_class">here</p></div>"#
            );

            let _ = tx_text.inner.try_broadcast("there".to_string()).unwrap();
            tx_text.until_empty().await;

            assert_eq!(
                view.html_string().await,
                r#"<div style="float: left;"><p class="p_class">there</p></div>"#
            );

            let _ = tx_style.inner.try_broadcast("right".to_string()).unwrap();
            tx_style.until_empty().await;

            assert_eq!(
                view.html_string().await,
                r#"<div style="float: right;"><p class="p_class">there</p></div>"#
            );

            let _ = tx_class
                .inner
                .try_broadcast("my_p_class".to_string())
                .unwrap();
            tx_class.until_empty().await;

            assert_eq!(
                view.html_string().await,
                r#"<div style="float: right;"><p class="my_p_class">there</p></div>"#
            );
        });
    }

    fn can_use_string_stream_as_child() {
        let executor = Arc::new(Executor::default());
        executor.run(async {
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
            let _: SsrDom = bldr.build().unwrap();
        });
    }

    #[test]
    fn test_use_tx_in_logic_loop() {
        let exe = Executor::default();
        exe.run(async {
            let (tx, mut rx) = broadcast::bounded::<()>(1);
            let (tx_end, mut rx_end) = broadcast::bounded::<()>(1);
            let tx_logic = tx.clone();
            exe.spawn(async move {
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

    fn patch_children_rsx_md() {
        let exe = Executor::default();
        exe.run(async {
            // ANCHOR: patch_children_rsx
            let (mut tx, rx) = mpsc::bounded(1);
            let my_view: Dom = html! {
                <div id="main" patch:children=rx>"Waiting for a patch message..."</div>
            }
            .build()
            .unwrap();

            tx.send(ListPatch::drain()).await.unwrap();
            // just as a sanity check we wait until the view has updated
            wait_while_async(1.0, || async {
                my_view.html_string().await != r#"<div id="main"></div>"#
            })
            .await
            .unwrap();

            let other_viewbuilder = html! {
                <h1>"Hello!"</h1>
            };

            tx.send(ListPatch::push(other_viewbuilder)).await.unwrap();
            wait_while_async(1.0, || async {
                my_view.html_string().await != r#"<div id="main"><h1>Hello!</h1></div>"#
            })
            .await
            .unwrap();
            // ANCHOR_END:patch_children_rsx
        });
    }

    #[test]
    pub fn can_build_readme_button() {}
}

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm {
    use std::ops::Bound;

    use mogwai::prelude::*;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_test::*;
    use web_sys::HtmlElement;

    wasm_bindgen_test_configure!(run_in_browser);

    type JsDomBuilder = ViewBuilder<JsDom>;

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
        let (tx, _) = broadcast::bounded::<DomEvent>(1);

        let rsx: JsDomBuilder = html! {
            <div id="view_zero" style:background_color="red">
                <pre on:click=tx.clone()>"this has text"</pre>
            </div>
        };
        let rsx_view = rsx.try_into().unwrap();

        let manual: JsDomBuilder = ViewBuilder::element("div")
            .with_single_attrib_stream("id", "view_zero")
            .with_single_style_stream("background-color", "red")
            .append(
                ViewBuilder::element("pre")
                    .with_event("click", EventTargetType::Myself, tx)
                    .append(ViewBuilder::text("this has text")),
            );
        let manual_view = manual.try_into().unwrap();

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

        let val = v.inner_read().left().unwrap();
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
    async fn gizmo_as_child() {
        // Since the pre tag is *not* dropped after the scope block the last assert
        // should show that the div tag has a child.
        let div = {
            let div = html! {
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
        let root = html! {
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
        let div = html! {
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
        let div = html! {
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
        let div = html! { <div style:display=("block", rx) /> }
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
        let _div = html! {
            <div id="hello" capture:view=tx.contra_map(|_| ())>
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
            let r = Some(if *n == 1 {
                "Clicked 1 time".to_string()
            } else {
                format!("Clicked {} times", *n)
            });
            futures::future::ready(r)
        });

        let button = html! {
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
        let (mut tx, rx) = mpsc::bounded::<ListPatch<ViewBuilder<JsDom>>>(1);
        let view = html! {
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
        let (mut tx, rx) = mpsc::bounded::<ListPatch<String>>(1);
        let view = html! {
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
        let _ = bldr.try_into().unwrap();
    }

    fn sendable<T: SendConstraints>(_: &T) {}

    #[wasm_bindgen_test]
    pub fn output_sendable() {
        let output: Output<JsDom> = Output::default();
        sendable(&output);

        mogwai::spawn(async move {
            let _ = output;
        })
    }

    #[wasm_bindgen_test]
    async fn can_capture_with_captured() {
        let capture: Captured<JsDom> = Captured::default().clone();
        let b = html! {
            <div id="chappie" capture:view=capture.sink()></div>
        };
        let _ = b.try_into().unwrap();
        let dom = capture.get().await;
        assert_eq!(dom.html_string().await, r#"<div id="chappie"></div>"#);
    }
}

#[cfg(test)]
mod test {
    use crate as mogwai_dom;
    use crate::prelude::*;
    use crate::view::DomBuilder; // for macros
    use async_executor::Executor;

    #[test]
    fn can_relay() {
        #[derive(Default)]
        struct Thing {
            view: Output<AnyView>,
            click: Output<()>,
            text: Input<String>,
        }

        impl Thing {
            fn view(mut self) -> ViewBuilder {
                rsx! (
                    div(
                        capture:view=self.view.sink(),
                        on:click=self.click.sink().contra_map(|_| ())
                    ) {
                        {("Hi", self.text.stream().unwrap())}
                    }
                )
                .with_task(async move {
                    let mut clicks = 0;
                    while let Some(()) = self.click.get().await {
                        clicks += 1;
                        self.text
                            .set(if clicks == 1 {
                                "1 click.".to_string()
                            } else {
                                format!("{} clicks.", clicks)
                            })
                            .await
                            .unwrap_or_else(|_| panic!("could not set text"));
                    }
                })
            }
        }

        let thing = Thing::default();
        let dom: Dom = thing.view().build().unwrap();
        match dom {
            Dom::Js(js) => js.run().unwrap(),
            Dom::Ssr(_) => {}
        }
    }

    fn can_capture_with_captured() {
        let capture: Captured<Dom> = Captured::default();
        let b = rsx! {
            div(id="chappie", capture:view=capture.sink()){}
        };
        let dom: Dom = b.build().unwrap();
        Executor::default().run(async move {
            let dom = capture.get().await;
            assert_eq!(dom.html_string().await, r#"<div id="chappie"></div>"#);
        });
    }
}
