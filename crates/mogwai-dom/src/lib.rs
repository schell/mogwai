//! Provides trait implementations and helper functions for running mogwai
//! html-based UI graphs in the browser and on a server.
pub mod builder;
pub mod event;
pub mod ssr;
pub mod utils;
pub mod view;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod nonwasm {
    use mogwai::{
        core::{
            builder::ViewBuilder,
            channel::broadcast::{self, *},
            futures::{sink::Contravariant, StreamExt},
            target::spawn,
            view::View,
        },
        dom::{event::DomEvent, ssr::SsrElement, view::Dom},
        macros::{builder, view},
    };
    use std::convert::{TryFrom, TryInto};

    #[test]
    fn capture_view() {
        let (tx, mut rx) = broadcast::bounded::<Dom>(1);
        let _view = view! {
            <div>
                <pre
                 capture:view = tx >
                    "Tack :)"
                </pre>
            </div>
        };

        futures::executor::block_on(async move {
            let dom = rx.next().await.unwrap();
            assert_eq!(dom.html_string().await, "<pre>Tack :)</pre>");
        });
    }

    #[test]
    fn test_append() {
        let ns = "http://www.w3.org/2000/svg";
        let _bldr = ViewBuilder::element("svg")
            .with_namespace(ns)
            .with_single_attrib_stream("width", "100")
            .with_single_attrib_stream("height", "100")
            .append(
                ViewBuilder::element("circle")
                    .with_namespace(ns)
                    .with_single_attrib_stream("cx", "50")
                    .with_single_attrib_stream("cy", "50")
                    .with_single_attrib_stream("r", "40")
                    .with_single_attrib_stream("stroke", "green")
                    .with_single_attrib_stream("stroke-width", "4")
                    .with_single_attrib_stream("fill", "yellow")
                    as ViewBuilder<Dom>,
            ) as ViewBuilder<Dom>;
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
            <div
             window:load=tx.contra_map(|_:DomEvent| ())>{("", rx.map(|()| "Loaded!".to_string()))}
            </div>
        };
    }

    #[test]
    fn cast_type_in_builder() {
        let _div = builder! {
            <div cast:type=Dom id="hello">"Inner Text"</div>
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

    #[smol_potat::test]
    async fn fragments() {
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
            view.html_string().await,
            "<section><div>hello</div> <div>hola</div> <div>kia ora</div></section>"
        );
    }

    #[test]
    fn post_build_manual() {
        let (tx, _rx) = bounded::<()>(1);

        let _div = ViewBuilder::element("div")
            .with_single_attrib_stream("id", "hello")
            .with_post_build(move |_: &mut Dom| {
                let _ = tx.inner.try_broadcast(()).unwrap();
            })
            .with_child(ViewBuilder::text("Hello"));
    }

    #[test]
    fn post_build_rsx() {
        let (tx, mut rx) = bounded::<()>(1);

        let _div = view! {
            <div id="hello" post:build=move |_| {
                let _ = tx.inner.try_broadcast(()).unwrap();
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
        let (_tx, rx) = bounded::<String>(1);
        let _div: View<Dom> = view! {
            <div>{("initial", rx)}</div>
        };
    }

    #[smol_potat::test]
    async fn ssr_properties_overwrite() {
        let el: SsrElement = SsrElement::element("div");
        el.set_style("float", "right").unwrap();
        assert_eq!(el.html_string().await, r#"<div style="float: right;"></div>"#);

        el.set_style("float", "left").unwrap();
        assert_eq!(el.html_string().await, r#"<div style="float: left;"></div>"#);

        el.set_style("width", "100px").unwrap();
        assert_eq!(
            el.html_string().await,
            r#"<div style="float: left; width: 100px;"></div>"#
        );
    }

    #[smol_potat::test]
    async fn ssr_attrib_overwrite() {
        let el: SsrElement = SsrElement::element("div");

        el.set_attrib("class", Some("my_class")).unwrap();
        assert_eq!(el.html_string().await, r#"<div class="my_class"></div>"#);

        el.set_attrib("class", Some("your_class")).unwrap();
        assert_eq!(el.html_string().await, r#"<div class="your_class"></div>"#);
    }

    #[smol_potat::test]
    pub async fn can_alter_ssr_views() {
        let (tx_text, rx_text) = bounded::<String>(1);
        let (tx_style, rx_style) = bounded::<String>(1);
        let (tx_class, rx_class) = bounded::<String>(1);
        let view = view! {
            <div style:float=("left", rx_style)><p class=("p_class", rx_class)>{("here", rx_text)}</p></div>
        };
        assert_eq!(
            view.inner.html_string().await,
            r#"<div style="float: left;"><p class="p_class">here</p></div>"#
        );

        let _ = tx_text.inner.try_broadcast("there".to_string()).unwrap();
        smol::block_on(async { tx_text.until_empty().await });

        assert_eq!(
            view.html_string().await,
            r#"<div style="float: left;"><p class="p_class">there</p></div>"#
        );

        let _ = tx_style.inner.try_broadcast("right".to_string()).unwrap();
        smol::block_on(async { tx_style.until_empty().await });

        assert_eq!(
            view.html_string().await,
            r#"<div style="float: right;"><p class="p_class">there</p></div>"#
        );

        let _ = tx_class
            .inner
            .try_broadcast("my_p_class".to_string())
            .unwrap();
        smol::block_on(async { tx_class.until_empty().await });

        assert_eq!(
            view.html_string().await,
            r#"<div style="float: right;"><p class="my_p_class">there</p></div>"#
        );
    }

    #[test]
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

    #[test]
    fn test_use_tx_in_logic_loop() {
        smol::block_on(async {
            let (tx, mut rx) = broadcast::bounded::<()>(1);
            let (tx_end, mut rx_end) = broadcast::bounded::<()>(1);
            let tx_logic = tx.clone();
            spawn(async move {
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

    #[smol_potat::test]
    async fn patch_children_rsx_md() {
        use mogwai::prelude::*;
        // ANCHOR: patch_children_rsx
        let (mut tx, rx) = mpsc::bounded(1);
        let my_view = view! {
            <div id="main" patch:children=rx>"Waiting for a patch message..."</div>
        };

        tx.send(ListPatch::drain()).await.unwrap();
        // just as a sanity check we wait until the view has updated
        wait_while_async(1.0, || async {
            my_view.html_string().await != r#"<div id="main"></div>"#
        }).await.unwrap();

        let other_viewbuilder = builder! {
            <h1>"Hello!"</h1>
        };

        tx.send(ListPatch::push(other_viewbuilder)).await.unwrap();
        wait_while_async(1.0, || async {
            my_view.html_string().await != r#"<div id="main"><h1>Hello!</h1></div>"#
        }).await.unwrap();
        // ANCHOR_END:patch_children_rsx
    }

    #[test]
    pub fn can_build_readme_button() {
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm {
    use std::{
        convert::{TryFrom, TryInto},
        ops::Bound,
    };
    use wasm_bindgen::JsCast;
    use wasm_bindgen_test::*;
    use web_sys::HtmlElement;

    use mogwai::prelude::*;

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
        let s = stream::once(async { "Hello!".to_string() });
        let _view: View<Dom> = ViewBuilder::text(s).try_into().unwrap();
    }

    #[wasm_bindgen_test]
    fn can_create_text_view_node_from_string_and_stream() {
        let s = "Hello!".to_string();
        let st = stream::once(async { "Goodbye!".to_string() });
        let _view: View<Dom> = ViewBuilder::text((s, st)).try_into().unwrap();
    }

    #[wasm_bindgen_test]
    fn can_create_text_view_node_from_str_and_stream() {
        let st = stream::once(async { "Goodbye!".to_string() });
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
            view.html_string().await,
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
            view.html_string().await,
            r#"<div id="view1" style="color: red; width: 100px;">Hello!</div>"#
        );
    }

    #[wasm_bindgen_test]
    async fn can_use_rsx_to_make_builder() {
        let (tx, _) = broadcast::bounded::<DomEvent>(1);

        let rsx: DomBuilder = builder! {
            <div id="view_zero" style:background_color="red">
                <pre on:click=tx.clone()>"this has text"</pre>
            </div>
        };
        let rsx_view = View::try_from(rsx).unwrap();

        let manual: DomBuilder = ViewBuilder::element("div")
            .with_single_attrib_stream("id", "view_zero")
            .with_single_style_stream("background-color", "red")
            .with_child(
                ViewBuilder::element("pre")
                    .with_event("click", EventTargetType::Myself, tx)
                    .with_child(ViewBuilder::text("this has text")),
            );
        let manual_view = View::try_from(manual).unwrap();

        assert_eq!(rsx_view.html_string().await, manual_view.html_string().await);
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
        let (tx, rx) = broadcast::bounded::<String>(1);
        let div = view! {
            <div class=("now", rx) />
        };
        let div_el: web_sys::HtmlElement = div.clone_as::<web_sys::HtmlElement>().unwrap();
        assert_eq!(div_el.outer_html(), r#"<div class="now"></div>"#);

        tx.broadcast("later".to_string()).await.unwrap();
        tx.until_empty().await;

        assert_eq!(div_el.outer_html(), r#"<div class="later"></div>"#);
    }

    #[wasm_bindgen_test]
    async fn rx_style_jsx() {
        let (tx, rx) = broadcast::bounded::<String>(1);
        let div = view! { <div style:display=("block", rx) /> };
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
        let _div = view! {
            <div id="hello" capture:view=tx.contra_map(|_| ())>
                "Hello there"
            </div>
        };

        let () = rx.recv().await.unwrap();
    }

    #[wasm_bindgen_test]
    pub async fn rx_text() {
        let (tx, rx) = broadcast::bounded::<String>(1);

        let div: View<Dom> = view! {
            <div>{("initial", rx)}</div>
        };

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
        let rx = rx.scan(0, |n: &mut i32, _: DomEvent| {
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
            <button on:click=tx.clone()>{("Clicked 0 times", rx)}</button>
        };

        let el = button.clone_as::<web_sys::HtmlElement>().unwrap();
        assert_eq!(el.inner_html(), "Clicked 0 times");

        el.click();
        tx.until_empty().await;
        let _ = wait_millis(1000).await;

        assert_eq!(el.inner_html(), "Clicked 1 time");
    }

    //fn nice_compiler_error() {
    //    let _div = view! {
    //        <div unknown:colon:thing="not ok" />
    //    };
    //}

    #[wasm_bindgen_test]
    async fn can_patch_children() {
        let (mut tx, rx) = mpsc::bounded::<ListPatch<ViewBuilder<Dom>>>(1);
        let view = view! {
            <ol id="main" patch:children=rx>
                <li>"Zero"</li>
                <li>"One"</li>
            </ol>
        };

        let dom: HtmlElement = view.clone_as::<HtmlElement>().unwrap();
        view.into_inner().run().unwrap();

        wait_while(1.0, || {
            dom.outer_html().as_str() != r#"<ol id="main"><li>Zero</li><li>One</li></ol>"#
        })
        .await
        .unwrap();

        tx.send(ListPatch::push(builder! {<li>"Two"</li>}))
            .await
            .unwrap();
        wait_while(1.0, || {
            dom.outer_html().as_str()
                != r#"<ol id="main"><li>Zero</li><li>One</li><li>Two</li></ol>"#
        })
        .await
        .unwrap();

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
            Some(builder! {<li>"Zero"</li>}).into_iter(),
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
            Some(builder! {<li>"Negative One"</li>}).into_iter(),
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
            Some(builder! {<li>"One"</li>}).into_iter(),
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
        let view = view! {
            <p id="main" patch:children=rx.map(|p| p.map(|s| ViewBuilder::text(s)))>
                "Zero ""One"
            </p>
        };

        let dom: HtmlElement = view.clone_as().unwrap();
        view.into_inner().run().unwrap();

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

    fn sendable<T: Sendable>(_:&T) {}

    #[wasm_bindgen_test]
    pub fn output_sendable() {
        let output: Output<Dom> = Output::default();
        sendable(&output);

        mogwai::spawn(async move {
            let _ = output;
        })
    }

    #[wasm_bindgen_test]
    async fn can_capture_with_captured() {
        let capture: Captured<Dom> = Captured::default().clone();
        let b = builder! {
            <div id="chappie" capture:view=capture.sink()></div>
        };
        let View{..} = Component::from(b).build().unwrap();
        let dom = capture.get().await;
        assert_eq!(dom.html_string().await, r#"<div id="chappie"></div>"#);
    }
}

#[cfg(test)]
mod test {
    use mogwai::prelude::*;

    #[test]
    fn can_relay() {
        #[derive(Default)]
        struct Thing {
            view: Output<Dom>,
            click: Output<()>,
            text: Input<String>,
        }

        impl Relay<Dom> for Thing {
            type Error = String;

            fn view(&mut self) -> ViewBuilder<Dom> {
                builder! {
                    <div capture:view=self.view.sink() on:click=self.click.sink().contra_map(|_| ())>
                        {("Hi", self.text.stream().unwrap())}
                    </div>
                }
            }

            fn logic(self) -> std::pin::Pin<Box<dyn Spawnable<Result<(), Self::Error>>>> {
                Box::pin(async move {
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
                            .map_err(|_| "could not set text".to_string())?;
                    }

                    Ok(())
                })
            }
        }

        mogwai::spawn(async {
            let thing = Thing::default();
            let View{ inner: dom } = thing.into_view().unwrap();
            dom.run().unwrap();
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn can_capture_with_captured() {
        let capture: Captured<Dom> = Captured::default();
        let b = builder! {
            <div id="chappie" capture:view=capture.sink()></div>
        };
        let View{..} = Component::from(b).build().unwrap();
        smol::block_on(async move {
            let dom = capture.get().await;
            assert_eq!(dom.html_string().await, r#"<div id="chappie"></div>"#);
        });
    }
}
