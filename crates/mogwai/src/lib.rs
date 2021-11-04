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
pub mod component;
pub mod event;
pub mod model;
pub mod patch;
pub mod prelude;
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

pub mod futures {
    //! A re-export of the `futures` crate.
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    pub use futures::*;

    use crate::target::Sendable;

    /// A simple wrapper around an async `Sender` to help implement `Sink`.
    #[derive(Clone)]
    pub struct SenderSink<S, T> {
        sender: S,
        sending_msgs: Arc<Mutex<VecDeque<T>>>,
    }

    /// Errors returned when using [`Sink`] operations.
    #[derive(Debug)]
    pub enum SinkError {
        /// Receiver is closed.
        Closed,
        /// The channel is full
        Full,
    }

    impl<T: 'static> SenderSink<async_channel::Sender<T>, T> {
        fn flush_sink(&mut self) -> Result<(), SinkError> {
            if self.sender.is_closed() {
                return Err(SinkError::Closed);
            }

            let mut msgs = self.sending_msgs.lock().unwrap();
            while let Some(item) = msgs.pop_front() {
                match self.sender.try_send(item) {
                    Ok(()) => {}
                    Err(err) => match err {
                        async_channel::TrySendError::Full(t) => {
                            msgs.push_front(t);
                            return Err(SinkError::Full);
                        }
                        async_channel::TrySendError::Closed(t) => {
                            msgs.push_front(t);
                            return Err(SinkError::Closed);
                        }
                    },
                }
            }

            assert!(msgs.is_empty());
            Ok(())
        }
    }

    impl<T: Clone> SenderSink<async_broadcast::Sender<T>, T> {
        fn flush_sink(&mut self) -> std::task::Poll<Result<(), SinkError>> {
            let closed = if let Some(item) = self.sending_msgs.lock().unwrap().pop_front() {
                match self.sender.try_broadcast(item) {
                    Ok(_) => false,
                    Err(err) => {
                        let closed = err.is_closed();
                        let item = err.into_inner();
                        self.sending_msgs.lock().unwrap().push_front(item);
                        closed
                    }
                }
            } else {
                false
            };

            self.sender.set_capacity(1 + self.sender.len());

            std::task::Poll::Ready(if closed {
                Err(SinkError::Closed)
            } else {
                Ok(())
            })
        }
    }

    impl<T: Unpin + 'static> Sink<T> for SenderSink<async_channel::Sender<T>, T> {
        type Error = SinkError;

        fn poll_ready(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            if self.sender.is_closed() {
                return std::task::Poll::Ready(Err(SinkError::Closed));
            }

            let cap = self.sender.capacity();

            let msgs = self.sending_msgs.lock().unwrap();
            if cap.is_none() || cap.unwrap() > msgs.len() {
                std::task::Poll::Ready(Ok(()))
            } else {
                // There are already messages in the queue
                std::task::Poll::Pending
            }
        }

        fn start_send(self: std::pin::Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
            if self.sender.is_closed() {
                return Err(SinkError::Closed);
            }

            let mut msgs = self.sending_msgs.lock().unwrap();
            let item = {
                msgs.push_back(item);
                msgs.pop_front().unwrap()
            };

            match self.sender.try_send(item) {
                Ok(()) => Ok(()),
                Err(async_channel::TrySendError::Full(t)) => {
                    msgs.push_front(t);
                    Ok(())
                }
                Err(async_channel::TrySendError::Closed(t)) => {
                    msgs.push_front(t);
                    Err(SinkError::Closed)
                }
            }
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            let data = self.get_mut();
            match data.flush_sink() {
                Ok(()) => std::task::Poll::Ready(Ok(())),
                Err(err) => match err {
                    SinkError::Closed => std::task::Poll::Ready(Err(SinkError::Closed)),
                    SinkError::Full => std::task::Poll::Pending,
                },
            }
        }

        fn poll_close(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            let data = self.get_mut();
            let poll = match data.flush_sink() {
                Ok(()) => std::task::Poll::Ready(Ok(())),
                Err(err) => match err {
                    SinkError::Closed => std::task::Poll::Ready(Err(SinkError::Closed)),
                    SinkError::Full => std::task::Poll::Pending,
                },
            };
            data.sender.close();
            poll
        }
    }

    impl<T: Clone + Unpin + 'static> Sink<T> for SenderSink<async_broadcast::Sender<T>, T> {
        type Error = SinkError;

        fn poll_ready(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            if self.sender.len() < self.sender.capacity() {
                std::task::Poll::Ready(Ok(()))
            } else {
                std::task::Poll::Pending
            }
        }

        fn start_send(self: std::pin::Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
            let data = self.get_mut();
            match data.sender.try_broadcast(item) {
                Ok(_) => Ok(()),
                Err(err) => match err {
                    async_broadcast::TrySendError::Full(item) => {
                        let len = data.sender.len();
                        data.sender.set_capacity(1 + len);
                        data.sending_msgs.lock().unwrap().push_back(item);
                        Ok(())
                    }
                    async_broadcast::TrySendError::Closed(_) => Err(SinkError::Closed),
                    async_broadcast::TrySendError::Inactive(_) => Ok(()),
                },
            }
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            let data = self.get_mut();
            data.flush_sink()
        }

        fn poll_close(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            let data = self.get_mut();
            let poll = data.flush_sink();
            data.sender.close();
            poll
        }
    }

    /// An extension trait that adds the ability for [`async_channel::Sender`] and
    /// [`async_broadcast::Sender`] to ergonomically create [`Sink`]s.
    pub trait IntoSenderSink<T>
    where
        Self: Sized,
    {
        /// Create a [`Sink`].
        fn sink(&self) -> SenderSink<Self, T>;
    }

    impl<T> IntoSenderSink<T> for async_channel::Sender<T> {
        fn sink(&self) -> SenderSink<Self, T> {
            SenderSink {
                sender: self.clone(),
                sending_msgs: Default::default(),
            }
        }
    }

    impl<T> IntoSenderSink<T> for async_broadcast::Sender<T> {
        fn sink(&self) -> SenderSink<Self, T> {
            SenderSink {
                sender: self.clone(),
                sending_msgs: Default::default(),
            }
        }
    }

    /// Type for supporting contravariant mapped sinks.
    pub struct ContraMap<S, X, Y> {
        sink: S,
        #[cfg(target_arch = "wasm32")]
        map: Box<dyn Fn(X) -> Y + 'static>,

        #[cfg(not(target_arch = "wasm32"))]
        map: Box<dyn Fn(X) -> Y + Send + 'static>,
    }

    impl<S: Sink<Y> + Unpin, X, Y> Sink<X> for ContraMap<S, X, Y> {
        type Error = <S as Sink<Y>>::Error;

        fn poll_ready(
            self: std::pin::Pin<&mut Self>,
            cx: &mut task::Context<'_>,
        ) -> task::Poll<Result<(), Self::Error>> {
            futures::ready!(self.get_mut().sink.poll_ready_unpin(cx))?;
            std::task::Poll::Ready(Ok(()))
        }

        fn start_send(self: std::pin::Pin<&mut Self>, item: X) -> Result<(), Self::Error> {
            let data = self.get_mut();
            let item = (data.map)(item);
            data.sink.start_send_unpin(item)?;
            Ok(())
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            cx: &mut task::Context<'_>,
        ) -> task::Poll<Result<(), Self::Error>> {
            futures::ready!(self.get_mut().sink.poll_flush_unpin(cx))?;
            std::task::Poll::Ready(Ok(()))
        }

        fn poll_close(
            self: std::pin::Pin<&mut Self>,
            cx: &mut task::Context<'_>,
        ) -> task::Poll<Result<(), Self::Error>> {
            self.get_mut().sink.poll_close_unpin(cx)
        }
    }

    /// Type for supporting contravariant filter-mapped sinks.
    pub struct ContraFilterMap<S, X, Y> {
        sink: S,
        #[cfg(target_arch = "wasm32")]
        map: Box<dyn Fn(X) -> Option<Y> + 'static>,

        #[cfg(not(target_arch = "wasm32"))]
        map: Box<dyn Fn(X) -> Option<Y> + Send + 'static>,
    }

    impl<S: Sink<Y> + Unpin, X, Y> Sink<X> for ContraFilterMap<S, X, Y> {
        type Error = <S as Sink<Y>>::Error;

        fn poll_ready(
            self: std::pin::Pin<&mut Self>,
            cx: &mut task::Context<'_>,
        ) -> task::Poll<Result<(), Self::Error>> {
            futures::ready!(self.get_mut().sink.poll_ready_unpin(cx))?;
            std::task::Poll::Ready(Ok(()))
        }

        fn start_send(self: std::pin::Pin<&mut Self>, item: X) -> Result<(), Self::Error> {
            let data = self.get_mut();
            if let Some(item) = (data.map)(item) {
                data.sink.start_send_unpin(item)?;
            }
            Ok(())
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            cx: &mut task::Context<'_>,
        ) -> task::Poll<Result<(), Self::Error>> {
            futures::ready!(self.get_mut().sink.poll_flush_unpin(cx))?;
            std::task::Poll::Ready(Ok(()))
        }

        fn poll_close(
            self: std::pin::Pin<&mut Self>,
            cx: &mut task::Context<'_>,
        ) -> task::Poll<Result<(), Self::Error>> {
            self.get_mut().sink.poll_close_unpin(cx)
        }
    }

    /// Contravariant functor extensions for types that implement [`Sink`].
    pub trait Contravariant<T>: Sink<T> + Sized {
        /// Extend this sink using a map function.
        ///
        /// This composes the map function _in front of the sink_, much like [`SinkExt::with`]
        /// but without async and without the option of failure.
        fn contra_map<S>(self, f: impl Fn(S) -> T + Sendable) -> ContraMap<Self, S, T> {
            ContraMap {
                map: Box::new(f),
                sink: self,
            }
        }

        /// Extend this sink using a filtering map function.
        ///
        /// This composes the map function _in front of the sink_, much like [`SinkExt::with_flat_map`]
        /// but without async and without the option of failure.
        fn contra_filter_map<S>(self, f: impl Fn(S) -> Option<T> + Sendable) -> ContraFilterMap<Self, S, T> {
            ContraFilterMap {
                map: Box::new(f),
                sink: self,
            }
        }
    }

    impl<S: Sized, T> Contravariant<T> for S where S: Sink<T> {}

    #[cfg(all(not(target_arch = "wasm32"), test))]
    mod test {
        use super::{ContraMap, Contravariant, IntoSenderSink, SinkExt};

        #[test]
        fn can_contra_map() {
            smol::block_on(async {
                let (tx, mut rx) = crate::channel::broadcast::bounded::<String>(1);

                // sanity
                tx.broadcast("blah".to_string()).await.unwrap();
                let _ = rx.recv().await.unwrap();

                let mut tx: ContraMap<_, u32, String> =
                    tx.sink().contra_map(|n: u32| format!("{}", n));
                tx.send(42).await.unwrap();
                let s = rx.recv().await.unwrap();
                assert_eq!(s.as_str(), "42");
            });
        }
    }
}

pub mod macros {
    //! RSX style macros for building DOM views.
    pub use mogwai_html_macro::{builder, view};
}

//#[cfg(doctest)]
//doc_comment::doctest!("../../../README.md");

#[cfg(all(test, not(target_arch = "wasm32")))]
mod test {
    use std::convert::TryFrom;

    use crate::{self as mogwai, channel::broadcast, ssr::SsrElement};
    use mogwai::{
        builder::ViewBuilder,
        channel::broadcast::*,
        macros::*,
        view::{Dom, View},
    };
    use web_sys::Event;

    #[test]
    fn cast_type_in_builder() {
        let _div = builder! {
            <div cast:type=mogwai::view::Dom id="hello">"Inner Text"</div>
        };
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
