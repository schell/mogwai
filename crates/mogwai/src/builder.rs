//! A low cost intermediate structure for creating views.
use crate::{
    patch::{HashPatch, ListPatch},
    spawn::{PostBuild, Sendable, Sinkable, SinkingWith, Streamable, Streaming},
    view::{Dom, View},
};
use futures::{Stream, StreamExt};
use std::{
    convert::TryFrom,
    pin::Pin,
    sync::{Arc, RwLock},
    task::{RawWaker, Wake, Waker},
};

struct DummyWaker;

impl Wake for DummyWaker {
    fn wake(self: std::sync::Arc<Self>) {}
}

/// Exhaust a stream until polling returns pending.
///
/// Returns the stream and the gathered items.
///
/// Useful for getting the starting value of a view.
pub fn exhaust<T, St>(mut stream: St) -> (St, Vec<T>)
where
    St: Stream<Item = T> + Sendable + Unpin,
{
    let raw_waker = RawWaker::from(Arc::new(DummyWaker));
    let waker = unsafe { Waker::from_raw(raw_waker) };
    let mut cx = std::task::Context::from_waker(&waker);
    let mut items = vec![];
    while let std::task::Poll::Ready(Some(t)) = stream.poll_next_unpin(&mut cx) {
        items.push(t);
    }
    (stream, items)
}

#[cfg(test)]
mod exhaust {
    use crate::builder::exhaust;
    use futures::StreamExt;

    #[test]
    fn exhaust_items() {
        let stream = Box::pin(
            futures::stream::iter(vec![0, 1, 2])
                .chain(futures::stream::once(async { 3 }))
                .chain(futures::stream::once(async { 4 }))
                .chain(futures::stream::once(async {
                    let _ = crate::time::wait_approx(2.0).await;
                    5
                })),
        );

        let (stream, items) = exhaust(stream);
        assert_eq!(items, vec![0, 1, 2, 3, 4]);

        futures::executor::block_on(async move {
            let n = stream.ready_chunks(100).next().await.unwrap();
            assert_eq!(n, vec![5]);
        });
    }
}

/// A stream of any static type.
pub type ValueStream<T> = Pin<Box<Streaming<T>>>;

/// A text/string stream.
pub type TextStream = Pin<Box<Streaming<String>>>;

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

impl<'a> From<&'a str> for MogwaiValue<'a, String, TextStream> {
    fn from(s: &'a str) -> Self {
        MogwaiValue::Owned(s.into())
    }
}

impl From<String> for MogwaiValue<'static, String, TextStream> {
    fn from(s: String) -> Self {
        MogwaiValue::Owned(s)
    }
}

impl<S: Sendable, St: Streamable<S>> From<St> for MogwaiValue<'static, S, St> {
    fn from(s: St) -> Self {
        MogwaiValue::Stream(s)
    }
}

impl<S: Sendable, St: Streamable<S>> From<(S, St)> for MogwaiValue<'static, S, St> {
    fn from(s: (S, St)) -> Self {
        MogwaiValue::OwnedAndStream(s.0, s.1)
    }
}

impl<'a, St: Streamable<String>> From<(&'a str, St)> for MogwaiValue<'a, String, St> {
    fn from(s: (&'a str, St)) -> Self {
        MogwaiValue::OwnedAndStream(s.0.to_string(), s.1)
    }
}

impl<'a, S: Clone + Sendable, St: Streamable<S>> From<MogwaiValue<'a, S, St>>
    for Pin<Box<Streaming<S>>>
{
    fn from(v: MogwaiValue<'a, S, St>) -> Self {
        match v {
            MogwaiValue::Ref(s) => {
                let s = s.clone();
                Box::pin(futures::stream::once(async move { s }))
            }
            MogwaiValue::Owned(s) => Box::pin(futures::stream::once(async move { s })),
            MogwaiValue::Stream(s) => Box::pin(s),
            MogwaiValue::OwnedAndStream(s, st) => {
                Box::pin(futures::stream::once(async move { s }).chain(st))
            }
            MogwaiValue::RefAndStream(s, st) => {
                let s = s.clone();
                Box::pin(futures::stream::once(async move { s }).chain(st))
            }
        }
    }
}

/// Boolean stream.
type BoolStream = Pin<Box<Streaming<bool>>>;

/// HashPatch updates for String attributes.
pub type AttribStream = Pin<Box<Streaming<HashPatch<String, String>>>>;

/// HashPatch updates for boolean attributes.
pub type BooleanAttribStream = Pin<Box<Streaming<HashPatch<String, bool>>>>;

/// HashPatch updates for style key value pairs.
pub type StyleStream = Pin<Box<Streaming<HashPatch<String, String>>>>;

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

/// Child patching declaration.
pub type ChildStream<T> = Pin<Box<Streaming<ListPatch<T>>>>;

/// The constituent values and streams of a [`ViewBuilder`].
///
/// The values have been [`exhaust`]ed from the streams to be used
/// for initialization.
///
/// This is an intermediate state between a [`ViewBuilder`] and a [`View`].
pub struct DecomposedViewBuilder<T> {
    /// Construction argument string.
    pub construct_with: String,
    /// Optional namespace.
    pub ns: Option<String>,
    /// The view's initial text declarations.
    pub texts: Vec<String>,
    /// The view's future text stream.
    pub text_stream: TextStream,
    /// This view's initial attribute declarations.
    pub attribs: Vec<HashPatch<String, String>>,
    /// The view's future attribute stream.
    pub attrib_stream: AttribStream,
    /// The view's initial boolean attribute declarations.
    pub bool_attribs: Vec<HashPatch<String, bool>>,
    /// The view's future boolean attribute stream.
    pub bool_attrib_stream: BooleanAttribStream,
    /// This view's style declarations.
    pub styles: Vec<HashPatch<String, String>>,
    /// The view's future style stream.
    pub style_stream: StyleStream,
    /// This view's child patch declarations.
    pub children: Vec<ListPatch<ViewBuilder<T>>>,
    /// This view's future child stream.
    pub child_stream: ChildStream<ViewBuilder<T>>,
    /// This view's post build operations.
    pub ops: Vec<Box<PostBuild<T>>>,
}

/// An un-built mogwai view.
/// A ViewBuilder is the most generic view representation in the mogwai library.
/// It is the the blueprint of a view - everything needed to create, hydrate or serialize the view.
pub struct ViewBuilder<T> {
    /// Construction argument string.
    construct_with: String,
    /// Optional namespace.
    ns: Option<String>,
    /// This view's text declarations.
    texts: Vec<TextStream>,
    /// This view's attribute declarations.
    attribs: Vec<AttribStream>,
    /// This view's boolean attribute declarations.
    bool_attribs: Vec<BooleanAttribStream>,
    /// This view's style declarations.
    styles: Vec<StyleStream>,
    /// This view's child patch declarations.
    patches: Vec<ChildStream<ViewBuilder<T>>>,
    /// This view's post build operations.
    ops: Vec<Box<PostBuild<T>>>,
}

impl<T: Sendable> ViewBuilder<T> {
    /// Create a new element builder.
    pub fn element(tag: &str) -> Self {
        ViewBuilder {
            construct_with: tag.to_string(),
            ns: None,
            texts: vec![],
            attribs: vec![],
            bool_attribs: vec![],
            styles: vec![],
            ops: vec![],
            patches: vec![],
        }
    }

    /// Create a new text builder.
    pub fn text<'a, Mv, St>(mv: Mv) -> Self
    where
        MogwaiValue<'a, String, St>: From<Mv>,
        St: Streamable<String>,
    {
        ViewBuilder::element("").with_text_stream(mv)
    }

    /// Add a namespace to the element.
    pub fn with_namespace(mut self, ns: &str) -> Self {
        self.ns = Some(ns.to_string());
        self
    }

    /// Add a stream to set the text of this builder.
    pub fn with_text_stream<'a, Mv, St>(mut self, mv: Mv) -> Self
    where
        MogwaiValue<'a, String, St>: From<Mv>,
        St: Streamable<String>,
    {
        let s: MogwaiValue<'a, String, St> = mv.into();
        let t: Pin<Box<Streaming<String>>> = s.into();
        self.texts.push(t);
        self
    }

    /// Add a stream to patch the attributes of this builder.
    pub fn with_attrib_stream<St>(mut self, st: St) -> Self
    where
        St: Streamable<HashPatch<String, String>>,
    {
        self.attribs.push(Box::pin(st));
        self
    }

    /// Add a stream to patch a single attribute of this builder.
    pub fn with_single_attrib_stream<'a, S, Mv, St>(mut self, s: S, mv: Mv) -> Self
    where
        S: Into<String>,
        MogwaiValue<'a, String, St>: From<Mv>,
        St: Streamable<String>,
    {
        let k = s.into();
        let s: MogwaiValue<'a, String, St> = mv.into();
        let t: TextStream = s.into();
        let t = t.map(move |v| HashPatch::Insert(k.clone(), v));
        self.attribs.push(Box::pin(t));
        self
    }

    /// Add a stream to patch the boolean attributes of this builder.
    pub fn with_bool_attrib_stream<St>(mut self, st: St) -> Self
    where
        St: Streamable<HashPatch<String, bool>>,
    {
        self.bool_attribs.push(Box::pin(st));
        self
    }

    /// Add a stream to patch a single boolean attribute of this builder.
    pub fn with_single_bool_attrib_stream<'a, S, Mv, St>(mut self, s: S, mv: Mv) -> Self
    where
        S: Into<String>,
        Mv: Into<MogwaiValue<'a, bool, St>>,
        St: Streamable<bool>,
    {
        let k = s.into();
        let s: MogwaiValue<'a, bool, St> = mv.into();
        let t = BoolStream::from(s).map(move |v| HashPatch::Insert(k.clone(), v));
        self.bool_attribs.push(Box::pin(t));
        self
    }

    /// Add a stream to patch the styles of this builder.
    pub fn with_style_stream<'a, St, Mv>(mut self, mv: Mv) -> Self
    where
        Mv: Into<MogwaiValue<'a, String, St>>,
        St: Streamable<String>,
    {
        let s: MogwaiValue<'a, String, St> = mv.into();
        let t = TextStream::from(s).flat_map(|v: String| {
            let kvs = v
                .split(';')
                .filter_map(|style| {
                    let (k, v) = style.split_once(':')?;
                    Some(HashPatch::Insert(
                        k.trim().to_string(),
                        v.trim().to_string(),
                    ))
                })
                .collect::<Vec<_>>();
            futures::stream::iter(kvs)
        });
        self.styles.push(Box::pin(t));
        self
    }

    /// Add a stream to patch a single style of this builder.
    pub fn with_single_style_stream<'a, S, Mv, St>(mut self, s: S, mv: Mv) -> Self
    where
        S: Into<String>,
        Mv: Into<MogwaiValue<'a, String, St>>,
        St: Streamable<String>,
    {
        let k = s.into();
        let s: MogwaiValue<'a, String, St> = mv.into();
        let t = TextStream::from(s).map(move |v| HashPatch::Insert(k.clone(), v));
        self.styles.push(Box::pin(t));
        self
    }

    /// Add a stream to patch the list of children of this builder.
    pub fn with_child_stream(mut self, s: impl Streamable<ListPatch<ViewBuilder<T>>>) -> Self {
        self.patches.push(Box::pin(s));
        self
    }

    /// Add a single child.
    ///
    /// This is a convenient short-hand for calling [`ViewBuilder::with_child_stream`] with
    /// a single child, right now - instead of a stream later.
    pub fn with_child(self, child: ViewBuilder<T>) -> Self {
        self.with_child_stream(futures::stream::once(async move { ListPatch::Push(child) }))
    }

    /// Add an operation to perform after the view has been built.
    pub fn with_post_build<F>(mut self, run: F) -> Self
    where
        F: FnOnce(&mut T) + Sendable,
    {
        self.ops.push(Box::new(run));
        self
    }
}

impl ViewBuilder<Dom> {
    /// Add a sink into which view events of the given name will be sent.
    pub fn with_event(self, name: &str, tx: impl Sinkable<web_sys::Event>) -> Self {
        log::info!("requested set '{}' event", name);
        let name = name.to_string();
        self.with_post_build(move |dom| {
            log::info!("setting '{}' event", &name);
            dom.set_event(EventTargetType::Myself, &name, Box::pin(tx));
        })
    }

    /// Add a sink into which window events of the given name will be sent.
    pub fn with_window_event(self, name: &str, tx: impl Sinkable<web_sys::Event>) -> Self {
        let name = name.to_string();
        self.with_post_build(move |dom| {
            dom.set_event(EventTargetType::Window, &name, Box::pin(tx));
        })
    }

    /// Add a sink into which document events of the given name will be sent.
    pub fn with_document_event(self, name: &str, tx: impl Sinkable<web_sys::Event>) -> Self {
        let name = name.to_string();
        self.with_post_build(move |dom| {
            dom.set_event(EventTargetType::Document, &name, Box::pin(tx));
        })
    }
}

impl<C: Sendable> From<&String> for ViewBuilder<C> {
    fn from(s: &String) -> Self {
        ViewBuilder::text(s.as_str())
    }
}

impl<C: Sendable> From<String> for ViewBuilder<C> {
    fn from(s: String) -> Self {
        ViewBuilder::text(s.as_str())
    }
}

impl<C:Sendable, St: Streamable<String>> From<(&str, St)> for ViewBuilder<C> {
    fn from(sst: (&str, St)) -> Self {
        ViewBuilder::text(sst)
    }
}

impl<C: 'static> From<ViewBuilder<C>> for DecomposedViewBuilder<C> {
    fn from(
        ViewBuilder {
            construct_with,
            ns,
            texts,
            attribs,
            bool_attribs,
            styles,
            patches,
            ops,
        }: ViewBuilder<C>,
    ) -> Self {
        let (text_stream, texts) = exhaust(Box::pin(futures::stream::select_all(texts)));
        let (attrib_stream, attribs) = exhaust(Box::pin(futures::stream::select_all(attribs)));
        let (bool_attrib_stream, bool_attribs) =
            exhaust(Box::pin(futures::stream::select_all(bool_attribs)));
        let (style_stream, styles) = exhaust(Box::pin(futures::stream::select_all(styles)));
        let (child_stream, children) = exhaust(Box::pin(futures::stream::select_all(patches)));
        DecomposedViewBuilder {
            construct_with,
            ns,
            texts,
            text_stream,
            attribs,
            attrib_stream,
            bool_attribs,
            bool_attrib_stream,
            styles,
            style_stream,
            children,
            child_stream,
            ops,
        }
    }
}

/// We can transform a ViewBuilder<T, _, _> into any View<T> when
/// T can be created from a DecomposedViewBuilder.
impl<C> TryFrom<ViewBuilder<C>> for View<C>
where
    C: 'static,
    View<C>: TryFrom<DecomposedViewBuilder<C>>,
{
    type Error = <View<C> as TryFrom<DecomposedViewBuilder<C>>>::Error;

    fn try_from(value: ViewBuilder<C>) -> Result<Self, Self::Error> {
        let decomp: DecomposedViewBuilder<C> = value.into();
        View::try_from(decomp)
    }
}

impl TryFrom<DecomposedViewBuilder<Dom>> for View<Dom> {
    type Error = String;

    fn try_from(
        DecomposedViewBuilder {
            construct_with,
            ns,
            texts,
            text_stream,
            attribs,
            attrib_stream,
            bool_attribs,
            bool_attrib_stream,
            styles,
            style_stream,
            children,
            child_stream,
            ops,
        }: DecomposedViewBuilder<Dom>,
    ) -> Result<Self, Self::Error> {
        let mut el: Dom = if !texts.is_empty() || construct_with.is_empty() {
            let node = Dom::text("")?;
            for text in texts.into_iter() {
                node.set_text(&text)?;
            }
            let text_stream: Pin<Box<Streaming<Result<String, String>>>> =
                Box::pin(text_stream.map(Ok));
            let text_sink: Pin<Box<SinkingWith<String, String>>> =
                Box::pin(futures::sink::unfold::<Dom, _, _, String, _>(
                    node.clone(),
                    |node, text: String| async move {
                        node.set_text(&text)?;
                        Ok(node)
                    },
                ));
            crate::spawn::spawn(async move {
                futures::pin_mut!(text_sink);
                let _ = text_stream.forward(text_sink).await;
            });
            node
        } else {
            Dom::element(&construct_with, ns.as_deref())?
        };

        for patch in attribs.into_iter() {
            el.patch_attribs(patch)?;
        }
        let attrib_stream: Pin<Box<Streaming<Result<HashPatch<String, String>, String>>>> =
            Box::pin(attrib_stream.map(Ok));
        let attrib_sink: Pin<Box<SinkingWith<HashPatch<String, String>, String>>> = Box::pin(
            futures::sink::unfold::<_, _, _, HashPatch<String, String>, _>(
                el.clone(),
                |view, patch| async move {
                    view.patch_attribs(patch)?;
                    Ok(view)
                },
            ),
        );
        crate::spawn::spawn(async move {
            futures::pin_mut!(attrib_sink);
            let _ = attrib_stream.forward(&mut attrib_sink).await;
        });

        for patch in bool_attribs.into_iter() {
            el.patch_bool_attribs(patch)?;
        }

        let bool_attrib_stream: Pin<Box<Streaming<Result<HashPatch<String, bool>, String>>>> =
            Box::pin(bool_attrib_stream.map(Ok));
        let bool_attrib_sink: Pin<Box<SinkingWith<HashPatch<_, _>, String>>> = Box::pin(
            futures::sink::unfold::<_, _, _, HashPatch<String, bool>, _>(
                el.clone(),
                |view, patch| async move {
                    view.patch_bool_attribs(patch)?;
                    Ok(view)
                },
            ),
        );
        crate::spawn::spawn(async move {
            futures::pin_mut!(bool_attrib_sink);
            let _ = bool_attrib_stream.forward(&mut bool_attrib_sink).await;
        });

        for patch in styles.into_iter() {
            el.patch_styles(patch)?;
        }
        let style_stream: Pin<Box<Streaming<Result<HashPatch<String, String>, String>>>> =
            Box::pin(style_stream.map(Ok));
        let style_sink: Pin<Box<SinkingWith<HashPatch<_, _>, String>>> = Box::pin(
            futures::sink::unfold::<_, _, _, HashPatch<String, String>, _>(
                el.clone(),
                |view, patch| async move {
                    view.patch_styles(patch)?;
                    Ok(view)
                },
            ),
        );
        crate::spawn::spawn(async move {
            futures::pin_mut!(style_sink);
            let _ = style_stream.forward(&mut style_sink).await;
        });

        for op in ops.into_iter() {
            (op)(&mut el);
        }

        for patch in children.into_iter() {
            let patch = patch.map(|vb| View::try_from(vb).unwrap().into_inner());
            el.patch_children(patch)?;
        }
        let child_stream: Pin<Box<Streaming<Result<ListPatch<_>, String>>>> =
            Box::pin(child_stream.map(Ok));
        let child_sink: Pin<Box<SinkingWith<ListPatch<_>, String>>> =
            Box::pin(futures::sink::unfold(
                el.clone(),
                |view, patch: ListPatch<ViewBuilder<_>>| async move {
                    let patch = patch.map(|vb| View::try_from(vb).unwrap().into_inner());
                    view.patch_children(patch)?;
                    Ok(view)
                },
            ));
        crate::spawn::spawn(async move {
            futures::pin_mut!(child_sink);
            let _ = child_stream.forward(&mut child_sink).await;
        });

        Ok(View {
            inner: el,
            detach: Arc::new(RwLock::new(Box::new(|t| t.detach()))),
        })
    }
}
