//! A low cost intermediate structure for creating views.
use crate::{
    channel::SinkError,
    patch::{HashPatch, ListPatch, ListPatchApply},
    ssr::SsrElement,
    view::View,
};
use futures::{Sink, SinkExt, Stream, StreamExt};
use std::{convert::TryFrom, marker::PhantomData, pin::Pin};
use wasm_bindgen::JsCast;

/// A text/string stream.
pub type TextStream = Pin<Box<dyn Stream<Item = String> + 'static>>;

/// An enumeration of string-like values that [`ViewBuilder`]s accept.
pub enum MogwaiValue<'a, S, St> {
    /// A reference to a string.
    Ref(&'a S),
    /// An owned string.
    Owned(S),
    /// A stream of values.
    Stream(St),
    /// An owned value and a stream of values.
    OwnedAndStream(S, St),
    /// A reference to a value and a stream of values.
    RefAndStream(&'a S, St),
}

impl<'a> From<&'a str> for MogwaiValue<'a, String, Pin<Box<dyn Stream<Item = String>>>> {
    fn from(s: &'a str) -> Self {
        MogwaiValue::Owned(s.into())
    }
}

impl From<String> for MogwaiValue<'static, String, Pin<Box<dyn Stream<Item = String>>>> {
    fn from(s: String) -> Self {
        MogwaiValue::Owned(s)
    }
}

impl<S: 'static, St: Stream<Item = S>> From<St> for MogwaiValue<'static, S, St> {
    fn from(s: St) -> Self {
        MogwaiValue::Stream(s)
    }
}

impl<S: 'static, St: Stream<Item = String>> From<(S, St)> for MogwaiValue<'static, S, St> {
    fn from(s: (S, St)) -> Self {
        MogwaiValue::OwnedAndStream(s.0, s.1)
    }
}

impl<'a, St: Stream<Item = String>> From<(&'a str, St)> for MogwaiValue<'a, String, St> {
    fn from(s: (&'a str, St)) -> Self {
        MogwaiValue::OwnedAndStream(s.0.to_string(), s.1)
    }
}

impl<'a, S: Clone + 'static, St: Stream<Item = S> + 'static> From<MogwaiValue<'a, S, St>>
    for Pin<Box<dyn Stream<Item = S>>>
{
    fn from(v: MogwaiValue<'a, S, St>) -> Self {
        match v {
            MogwaiValue::Ref(s) => {
                let s = s.clone();
                futures::stream::once(async move { s }).boxed_local()
            }
            MogwaiValue::Owned(s) => futures::stream::once(async move { s }).boxed_local(),
            MogwaiValue::Stream(s) => s.boxed_local(),
            MogwaiValue::OwnedAndStream(s, st) => futures::stream::once(async move { s })
                .chain(st)
                .boxed_local(),

            MogwaiValue::RefAndStream(s, st) => {
                let s = s.clone();
                futures::stream::once(async move { s })
                    .chain(st)
                    .boxed_local()
            }
        }
    }
}

type BoolStream = Pin<Box<dyn Stream<Item = bool> + 'static>>;

/// HashPatch updates for String attributes.
pub type AttribStream = Pin<Box<dyn Stream<Item = HashPatch<String, String>> + 'static>>;

/// HashPatch updates for boolean attributes.
pub type BooleanAttribStream = Pin<Box<dyn Stream<Item = HashPatch<String, bool>> + 'static>>;

/// HashPatch updates for style key value pairs.
pub type StyleStream = Pin<Box<dyn Stream<Item = HashPatch<String, String>> + 'static>>;

/// An event target declaration.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum EventTargetType {
    /// This target is the view it is declared on.
    Myself,
    /// This target is the window.
    Window,
    /// This target is the document.
    Document,
}

/// An output event declaration.
pub struct EventCmd<Event> {
    /// The target of the event.
    /// In other words this is the target that a listener will be placed on.
    pub type_is: EventTargetType,
    /// The event name.
    pub name: String,
    /// The [`Sender`] that the event should be sent on.
    pub transmitter: Pin<Box<dyn Sink<Event, Error = SinkError>>>,
}

/// Child patching declaration.
pub type ChildStream<T> = Pin<Box<dyn Stream<Item = ListPatch<T>>>>;

/// An un-built mogwai view.
/// A ViewBuilder is the most generic view representation in the mogwai library.
/// It is the the blueprint of a view - everything needed to create, hydrate or serialize the view.
pub struct ViewBuilder<T, Child, Event> {
    _type_is: PhantomData<T>,
    /// Construction argument string.
    construct_with: String,
    /// Optional namespace.
    ns: Option<String>,
    /// Ready sink.
    ///
    /// Sends the update count.
    ready: Option<Pin<Box<dyn Sink<u8, Error = SinkError>>>>,
    /// This view's text declarations.
    texts: Vec<TextStream>,
    /// This view's attribute declarations.
    attribs: Vec<AttribStream>,
    /// This view's boolean attribute declarations.
    bool_attribs: Vec<BooleanAttribStream>,
    /// This view's style declarations.
    styles: Vec<StyleStream>,
    /// This view's child patch declarations.
    patches: Vec<ChildStream<ViewBuilder<Child, Child, Event>>>,
    /// This view's output events.
    events: Vec<EventCmd<Event>>,
}

impl<T, Child: 'static, Event: 'static> ViewBuilder<T, Child, Event> {
    /// Create a new element builder.
    pub fn element(tag: &str) -> Self {
        ViewBuilder {
            _type_is: PhantomData,
            construct_with: tag.to_string(),
            ns: None,
            ready: None,
            texts: vec![],
            attribs: vec![],
            bool_attribs: vec![],
            styles: vec![],
            events: vec![],
            patches: vec![],
        }
    }

    /// Create a new text builder.
    pub fn text<'a, Mv, St>(mv: Mv) -> Self
    where
        Mv: Into<MogwaiValue<'a, String, St>>,
        St: Stream<Item = String> + 'static,
    {
        ViewBuilder::element("").with_text_stream(mv)
    }

    /// Add a namespace to the element.
    pub fn with_namespace(mut self, ns: &str) -> Self {
        self.ns = Some(ns.to_string());
        self
    }

    /// Add a sink to send ready notifications.
    ///
    /// A ready notification is sent when all "ready" updates have
    /// been performed in the current cycle.
    ///
    /// Calling this more than once **replaces** the sink that is
    /// currently set.
    pub fn with_ready_sink(mut self, sink: impl Sink<u8, Error = SinkError> + 'static) -> Self {
        self.ready = Some(Box::pin(sink));
        self
    }

    /// Add a stream to set the text of this builder.
    pub fn with_text_stream<'a, Mv, St>(mut self, mv: Mv) -> Self
    where
        Mv: Into<MogwaiValue<'a, String, St>>,
        St: Stream<Item = String> + 'static,
    {
        let s: MogwaiValue<'a, String, St> = mv.into();
        let mut t = TextStream::from(s);
        self.texts.push(t);
        self
    }

    /// Add a stream to patch the attributes of this builder.
    pub fn with_attrib_stream<St>(mut self, st: St) -> Self
    where
        St: Stream<Item = HashPatch<String, String>> + 'static,
    {
        self.attribs.push(st.boxed_local());
        self
    }

    /// Add a stream to patch a single attribute of this builder.
    pub fn with_single_attrib_stream<'a, S, Mv, St>(mut self, s: S, mv: Mv) -> Self
    where
        S: Into<String>,
        Mv: Into<MogwaiValue<'a, String, St>>,
        St: Stream<Item = String> + 'static,
    {
        let k = s.into();
        let s: MogwaiValue<'a, String, St> = mv.into();
        let t = TextStream::from(s).map(move |v| HashPatch::Insert(k.clone(), v));
        self.attribs.push(t.boxed_local());
        self
    }

    /// Add a stream to patch the boolean attributes of this builder.
    pub fn with_bool_attrib_stream<St>(mut self, st: St) -> Self
    where
        St: Stream<Item = HashPatch<String, bool>> + 'static,
    {
        self.bool_attribs.push(st.boxed_local());
        self
    }

    /// Add a stream to patch a single boolean attribute of this builder.
    pub fn with_single_bool_attrib_stream<'a, S, Mv, St>(mut self, s: S, mv: Mv) -> Self
    where
        S: Into<String>,
        Mv: Into<MogwaiValue<'a, bool, St>>,
        St: Stream<Item = bool> + 'static,
    {
        let k = s.into();
        let s: MogwaiValue<'a, bool, St> = mv.into();
        let t = BoolStream::from(s).map(move |v| HashPatch::Insert(k.clone(), v));
        self.bool_attribs.push(t.boxed_local());
        self
    }

    /// Add a stream to patch the styles of this builder.
    pub fn with_style_stream<St>(mut self, st: St) -> Self
    where
        St: Stream<Item = HashPatch<String, String>> + 'static,
    {
        self.styles.push(st.boxed_local());
        self
    }

    /// Add a stream to patch a single style of this builder.
    pub fn with_single_style_stream<'a, S, Mv, St>(mut self, s: S, mv: Mv) -> Self
    where
        S: Into<String>,
        Mv: Into<MogwaiValue<'a, String, St>>,
        St: Stream<Item = String> + 'static,
    {
        let k = s.into();
        let s: MogwaiValue<'a, String, St> = mv.into();
        let t = TextStream::from(s).map(move |v| HashPatch::Insert(k.clone(), v));
        self.styles.push(t.boxed_local());
        self
    }

    /// Add a stream to patch the list of children of this builder.
    pub fn with_child_stream(
        mut self,
        s: impl Stream<Item = ListPatch<ViewBuilder<Child, Child, Event>>> + 'static,
    ) -> Self {
        self.patches.push(s.boxed_local());
        self
    }

    /// Add a single child.
    ///
    /// This is a convenient short-hand for calling [`ViewBuilder::with_child_stream`] with
    pub fn with_child(mut self, child: ViewBuilder<Child, Child, Event>) -> Self {
        self.with_child_stream(futures::stream::once(async move { ListPatch::Push(child) }))
    }

    /// Add a sink into which view events of the given name will be sent.
    pub fn with_event(
        mut self,
        name: &str,
        tx: impl Sink<Event, Error = SinkError> + 'static,
    ) -> Self {
        self.events.push(EventCmd {
            type_is: EventTargetType::Myself,
            name: name.into(),
            transmitter: Box::pin(tx),
        });
        self
    }

    /// Add a sink into which window events of the given name will be sent.
    pub fn with_window_event(
        mut self,
        name: &str,
        tx: impl Sink<Event, Error = SinkError> + 'static,
    ) -> Self {
        self.events.push(EventCmd {
            type_is: EventTargetType::Window,
            name: name.into(),
            transmitter: Box::pin(tx),
        });
        self
    }

    /// Add a sink into which document events of the given name will be sent.
    pub fn with_document_event(
        mut self,
        name: &str,
        tx: impl Sink<Event, Error = SinkError> + 'static,
    ) -> Self {
        self.events.push(EventCmd {
            type_is: EventTargetType::Document,
            name: name.into(),
            transmitter: Box::pin(tx),
        });
        self
    }

    /// Cast the underlying view type of this builder.
    pub fn with_type<V>(self) -> ViewBuilder<V, Child, Event> {
        let ViewBuilder {
            _type_is: _,
            construct_with,
            ns,
            ready,
            texts,
            attribs,
            bool_attribs,
            styles,
            patches,
            events,
        } = self;
        ViewBuilder {
            _type_is: PhantomData,
            construct_with,
            ns,
            ready,
            texts,
            attribs,
            bool_attribs,
            styles,
            patches,
            events,
        }
    }
}

/// Used for sending update ticks.
#[derive(Clone)]
pub struct ReadyTx {
    /// The underlying sender.
    pub tx: Option<async_channel::Sender<()>>,
}

impl ReadyTx {
    /// Send an update notification if anyone is listening.
    pub async fn tick(&mut self) {
        if let Some(tx) = self.tx.take() {
            match tx.send(()).await {
                Ok(()) => {
                    self.tx = Some(tx);
                }
                Err(async_channel::SendError(())) => {}
            }
        }
    }
}

impl<T> TryFrom<ViewBuilder<T, web_sys::Node, web_sys::Event>> for View<T>
where
    T: JsCast,
{
    type Error = String;

    fn try_from(
        builder: ViewBuilder<T, web_sys::Node, web_sys::Event>,
    ) -> Result<Self, Self::Error> {
        let ViewBuilder {
            _type_is: _,
            construct_with,
            ns,
            ready,
            attribs,
            bool_attribs,
            styles,
            events,
            patches,
            texts,
        } = builder;

        let ready_sink = ready;
        let (ready_tx, ready_rx) = if ready_sink.is_some() {
            let (tx, rx) = async_channel::unbounded::<()>();
            (ReadyTx { tx: Some(tx) }, Some(rx))
        } else {
            (ReadyTx { tx: None }, None)
        };

        let el: T = if !texts.is_empty() {
            let node = web_sys::Text::new().unwrap();
            let stream = futures::stream::select_all(texts)
                .map(|t| Ok(t))
                .boxed_local();
            let sink = futures::sink::unfold::<(web_sys::Text, ReadyTx), _, _, String, ()>(
                (node.clone(), ready_tx.clone()),
                |(node, mut ready), text: String| async move {
                    node.set_data(&text);
                    ready.tick().await;
                    Ok((node, ready))
                },
            );
            wasm_bindgen_futures::spawn_local(async move {
                stream.forward(sink).await.unwrap();
            });
            node.dyn_into::<T>()
                .map_err(|e| format!("could not cast to {}: {:?}", std::any::type_name::<T>(), e))?
        } else if let Some(ns) = ns {
            let window: web_sys::Window = web_sys::window().unwrap();
            let document: web_sys::Document = window.document().unwrap();
            let node = document
                .create_element_ns(Some(&ns), &construct_with)
                .unwrap();
            node.dyn_into::<T>()
                .map_err(|e| format!("could not cast to {}: {:?}", std::any::type_name::<T>(), e))?
        } else {
            let window: web_sys::Window = web_sys::window().unwrap();
            let document: web_sys::Document = window.document().unwrap();
            let node = document.create_element(&construct_with).unwrap();
            node.dyn_into::<T>()
                .map_err(|e| format!("could not cast to {}: {:?}", std::any::type_name::<T>(), e))?
        };

        if !attribs.is_empty() {
            let stream: Pin<Box<dyn Stream<Item = Result<HashPatch<String, String>, ()>>>> =
                futures::stream::select_all(attribs).map(Ok).boxed_local();
            let sink = futures::sink::unfold::<
                (web_sys::HtmlElement, ReadyTx),
                _,
                _,
                HashPatch<String, String>,
                _,
            >(
                (
                    el.dyn_ref::<web_sys::HtmlElement>()
                        .ok_or_else(|| "could not cast to HtmlElement".to_string())?
                        .clone(),
                    ready_tx.clone(),
                ),
                |(view, mut ready), patch| async move {
                    match patch {
                        crate::patch::HashPatch::Insert(k, v) => {
                            view.set_attribute(&k, &v).map_err(|_| ())?;
                        }
                        crate::patch::HashPatch::Remove(k) => {
                            view.remove_attribute(&k).map_err(|_| ())?;
                        }
                    }
                    ready.tick().await;
                    Ok((view, ready))
                },
            );
            wasm_bindgen_futures::spawn_local(async move {
                futures::pin_mut!(sink);
                let _ = stream.forward(&mut sink).await;
            });
        }

        if !bool_attribs.is_empty() {
            let stream: Pin<Box<dyn Stream<Item = Result<HashPatch<String, bool>, ()>>>> =
                futures::stream::select_all(bool_attribs)
                    .map(Ok)
                    .boxed_local();
            let sink = futures::sink::unfold::<
                (web_sys::HtmlElement, ReadyTx),
                _,
                _,
                HashPatch<String, bool>,
                _,
            >(
                (
                    el.dyn_ref::<web_sys::HtmlElement>()
                        .ok_or_else(|| "could not cast to HtmlElement".to_string())?
                        .clone(),
                    ready_tx.clone(),
                ),
                |(view, mut ready), patch| async move {
                    match patch {
                        crate::patch::HashPatch::Insert(k, v) => {
                            if v {
                                view.set_attribute(&k, "").map_err(|_| ())?;
                            } else {
                                view.remove_attribute(&k).map_err(|_| ())?;
                            }
                        }
                        crate::patch::HashPatch::Remove(k) => {
                            view.remove_attribute(&k).map_err(|_| ())?;
                        }
                    }
                    ready.tick().await;
                    Ok((view, ready))
                },
            );
            wasm_bindgen_futures::spawn_local(async move {
                futures::pin_mut!(sink);
                let _ = stream.forward(&mut sink).await;
            })
        }

        if !styles.is_empty() {
            let stream: Pin<Box<dyn Stream<Item = Result<HashPatch<String, String>, ()>>>> =
                futures::stream::select_all(styles).map(Ok).boxed_local();
            let sink = futures::sink::unfold::<
                (web_sys::CssStyleDeclaration, ReadyTx),
                _,
                _,
                HashPatch<String, String>,
                _,
            >(
                (
                    el.dyn_ref::<web_sys::HtmlElement>()
                        .ok_or_else(|| "could not cast to HtmlElement".to_string())?
                        .style(),
                    ready_tx.clone(),
                ),
                |(style, mut ready), patch| async move {
                    match patch {
                        crate::patch::HashPatch::Insert(k, v) => {
                            style.set_property(&k, &v).map_err(|_| ())?;
                        }
                        crate::patch::HashPatch::Remove(k) => {
                            style.remove_property(&k).map_err(|_| ())?;
                        }
                    }
                    ready.tick().await;
                    Ok((style, ready))
                },
            );
            wasm_bindgen_futures::spawn_local(async move {
                futures::pin_mut!(sink);
                let _ = stream.forward(&mut sink).await;
            });
        }

        if !events.is_empty() {
            let target = el
                .dyn_ref::<web_sys::EventTarget>()
                .ok_or_else(|| "could not cast to EventTarget".to_string())?;
            for EventCmd {
                type_is,
                name,
                transmitter,
            } in events.into_iter()
            {
                match type_is {
                    EventTargetType::Myself => {
                        crate::event::add_event(&name, target, transmitter);
                    }
                    EventTargetType::Window => {
                        crate::event::add_event(&name, &web_sys::window().unwrap(), transmitter);
                    }
                    EventTargetType::Document => {
                        crate::event::add_event(
                            &name,
                            &web_sys::window().unwrap().document().unwrap(),
                            transmitter,
                        );
                    }
                }
            }
        }

        if !patches.is_empty() {
            let stream: Pin<Box<dyn Stream<Item = Result<ListPatch<_>, ()>>>> =
                futures::stream::select_all(patches).map(Ok).boxed_local();
            let sink = futures::sink::unfold::<(web_sys::Node, ReadyTx), _, _, _, _>(
                (
                    el.dyn_ref::<web_sys::Node>()
                        .ok_or_else(|| "could not cast to Node".to_string())?
                        .clone(),
                    ready_tx.clone(),
                ),
                |(mut view, mut ready), patch: ListPatch<ViewBuilder<_, _, _>>| async move {
                    let patch = patch.map(|vb| {
                        let view: View<web_sys::Node> = View::try_from(vb).unwrap();
                        view.inner
                    });
                    let _ = view.list_patch_apply(patch);
                    ready.tick().await;
                    Ok((view, ready))
                },
            );
            wasm_bindgen_futures::spawn_local(async move {
                futures::pin_mut!(sink);
                let _ = stream.forward(&mut sink).await;
            });
        }

        // This allows us to tell downstream listeners when our view has processed all
        // "ready" streams (streams that have values waiting in them)
        if let Some(mut sink) = ready_sink {
            if let Some(rx) = ready_rx {
                wasm_bindgen_futures::spawn_local(async move {
                    // start out our ready signal to allow some time for processing any immediate changes.
                    let rx = futures::stream::once(async {
                        let _ = crate::utils::wait_approximately(1.0).await;
                    })
                    .chain(rx)
                    .boxed_local();
                    let mut rx = rx.ready_chunks(u8::MAX as usize);
                    loop {
                        match rx.next().await {
                            Some(updates) => match sink.send(updates.len() as u8).await {
                                Ok(()) => {}
                                Err(err) => match err {
                                    SinkError::Closed => break,
                                    SinkError::Full => panic!("ready sink is full"),
                                },
                            },
                            None => break,
                        }
                    }
                });
            }
        }

        Ok(View { inner: el })
    }
}

impl<Event: 'static> TryFrom<ViewBuilder<SsrElement<Event>, SsrElement<Event>, Event>>
    for View<SsrElement<Event>>
{
    type Error = String;

    fn try_from(
        builder: ViewBuilder<SsrElement<Event>, SsrElement<Event>, Event>,
    ) -> Result<Self, Self::Error> {
        let ViewBuilder {
            _type_is: _,
            construct_with,
            ns,
            ready,
            attribs,
            bool_attribs,
            styles,
            events,
            patches,
            texts,
        } = builder;

        let mut el: SsrElement<Event> = if !texts.is_empty() {
            let node = SsrElement::text("");
            let stream = futures::stream::select_all(texts).map(Ok).boxed_local();
            let sink = futures::sink::unfold::<SsrElement<_>, _, _, String, ()>(
                node.clone(),
                |node, text: String| async move { node.with_text(text).await },
            );
            wasm_bindgen_futures::spawn_local(async move {
                futures::pin_mut!(sink);
                let _ = stream.forward(&mut sink).await;
            });
            node
        } else {
            SsrElement::element(&construct_with)
        };
        if let Some(ns) = ns {
            el = futures::executor::block_on(async move {
                el.with_attrib("xmlns".to_string(), Some(ns)).await.unwrap()
            });
        }
        if !attribs.is_empty() {
            let stream = futures::stream::select_all(attribs).map(|v| Ok(v));
            let sink = futures::sink::unfold(
                el.clone(),
                |el, patch: HashPatch<String, String>| async move {
                    match patch {
                        crate::patch::HashPatch::Insert(k, v) => el.with_attrib(k, Some(v)).await,
                        crate::patch::HashPatch::Remove(k) => el.without_attrib(k).await,
                    }
                },
            );
            wasm_bindgen_futures::spawn_local(async move {
                futures::pin_mut!(sink);
                let _ = stream.forward(&mut sink).await;
            });
        }

        if !bool_attribs.is_empty() {
            let stream: Pin<Box<dyn Stream<Item = Result<HashPatch<String, bool>, ()>>>> =
                futures::stream::select_all(bool_attribs)
                    .map(Ok)
                    .boxed_local();
            let sink = futures::sink::unfold(el.clone(), |el, patch| async move {
                match patch {
                    crate::patch::HashPatch::Insert(k, v) => {
                        if v {
                            el.with_attrib(k, None).await
                        } else {
                            el.without_attrib(k).await
                        }
                    }
                    crate::patch::HashPatch::Remove(k) => el.without_attrib(k).await,
                }
            });
            wasm_bindgen_futures::spawn_local(async move {
                futures::pin_mut!(sink);
                let _ = stream.forward(&mut sink).await;
            });
        }

        if !styles.is_empty() {
            let stream = futures::stream::select_all(styles).map(Ok);
            let sink = futures::sink::unfold(el.clone(), |el, patch| async move {
                match patch {
                    crate::patch::HashPatch::Insert(k, v) => el.with_style(k, v).await,
                    crate::patch::HashPatch::Remove(k) => el.without_style(k).await,
                }
            });
            wasm_bindgen_futures::spawn_local(async move {
                futures::pin_mut!(sink);
                let _ = stream.forward(&mut sink).await;
            });
        }

        if !events.is_empty() {
            for EventCmd {
                type_is,
                name,
                transmitter,
            } in events.into_iter()
            {
                el = futures::executor::block_on(async {
                    el.with_event(type_is, name, transmitter)
                        .await
                        .map_err(|()| "could not add event".to_string())
                })?;
            }
        }

        if !patches.is_empty() {
            let stream: Pin<Box<dyn Stream<Item = Result<ListPatch<SsrElement<Event>>, ()>>>> =
                futures::stream::select_all(patches)
                    .map(|patch| Ok(patch.map(|vb| View::try_from(vb).unwrap().inner)))
                    .boxed_local();
            let sink = futures::sink::unfold(el.clone(), |el, patch| async move {
                el.with_patch_children(patch).await
            });
            wasm_bindgen_futures::spawn_local(async move {
                futures::pin_mut!(sink);
                let _ = stream.forward(&mut sink).await;
            });
        }

        Ok(View { inner: el })
    }
}
