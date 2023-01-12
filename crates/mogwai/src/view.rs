//! Domain agnostic view doclaration.
use std::{
    future::Future,
    any::Any,
    pin::Pin,
    sync::Arc,
    task::{RawWaker, Wake, Waker},
};

use crate::{
    sink::{Sink, SinkExt},
    stream::{Stream, StreamExt},
    patch::{HashPatch, ListPatch},
};
use anyhow::Context;
pub use anyhow::Error;

/// A struct with a no-op implementation of Waker
pub struct DummyWaker;

impl Wake for DummyWaker {
    fn wake(self: std::sync::Arc<Self>) {}
}

/// A trait for domain-specific views.
///
/// A view is a smart pointer that can be cheaply cloned, where clones all refer
/// to the same underlying user interface node.
pub trait View: Any + Sized + Clone + Unpin + Send + Sync {}
impl<T: Any + Sized + Clone + Unpin + Send + Sync> View for T {}

/// A type erased view.
///
/// Used to write view builders in a domain-agnostic way.
pub struct AnyView {
    inner: Box<dyn Any + Send + Sync>,
    clone_fn: fn(&AnyView) -> AnyView,
    #[cfg(debug_assertions)]
    inner_type_name: &'static str,
}

impl std::fmt::Debug for AnyView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[cfg(not(debug_assertions))]
        let type_name = "unknown w/o debug_assertions";
        #[cfg(debug_assertions)]
        let type_name = self.inner_type_name;

        f.debug_struct("AnyView")
            .field("inner_type", &format!("{}", type_name))
            .finish()
    }
}

impl Clone for AnyView {
    fn clone(&self) -> Self {
        let cloned_view = (self.clone_fn)(self);
        Self {
            inner: cloned_view.inner,
            clone_fn: self.clone_fn.clone(),
            #[cfg(debug_assertions)]
            inner_type_name: self.inner_type_name,
        }
    }
}

fn any_view_clone<V: View>(any_view: &AnyView) -> AnyView {
    let v: &V = any_view.downcast_ref().unwrap();
    AnyView {
        inner: Box::new(v.clone()) as Box<dyn Any + Send + Sync>,
        clone_fn: any_view_clone::<V>,
        #[cfg(debug_assertions)]
        inner_type_name: std::any::type_name::<V>(),
    }
}

impl AnyView {
    pub fn new<V: View>(inner: V) -> Self {
        AnyView {
            inner: Box::new(inner),
            clone_fn: any_view_clone::<V>,
            #[cfg(debug_assertions)]
            inner_type_name: std::any::type_name::<V>(),
        }
    }

    pub fn downcast_ref<V: View>(&self) -> Option<&V> {
        self.inner.downcast_ref()
    }

    pub fn downcast_mut<V: View>(&mut self) -> Option<&mut V> {
        self.inner.downcast_mut()
    }

    pub fn downcast<V: View>(self) -> anyhow::Result<V> {
        let v: Box<V> = self.inner.downcast().ok().with_context(|| {
            #[cfg(not(debug_assertions))]
            let type_name = "unknown";
            #[cfg(debug_assertions)]
            let type_name = self.inner_type_name;
            format!(
                "could not downcast AnyView {{{type_name}}} to '{}'",
                std::any::type_name::<V>()
            )
        })?;
        Ok(*v)
    }
}

fn any_event_clone<T: Any + Send + Sync + Clone>(any_event: &AnyEvent) -> AnyEvent {
    let ev: &T = any_event.inner.downcast_ref::<T>().unwrap();
    AnyEvent {
        inner: Box::new(ev.clone()),
        clone_fn: any_event_clone::<T>,
        #[cfg(debug_assertions)]
        inner_type_name: any_event.inner_type_name,
    }
}

/// A type erased view event.
///
/// Used to write view builders in a domain-agnostic way.
pub struct AnyEvent {
    inner: Box<dyn Any + Send + Sync>,
    clone_fn: fn(&AnyEvent) -> AnyEvent,
    #[cfg(debug_assertions)]
    inner_type_name: &'static str,
}

impl Clone for AnyEvent {
    fn clone(&self) -> Self {
        (self.clone_fn)(self)
    }
}

impl AnyEvent {
    pub fn new<T: Any + Send + Sync + Clone>(inner: T) -> Self {
        AnyEvent {
            inner: Box::new(inner),
            clone_fn: any_event_clone::<T>,
            #[cfg(debug_assertions)]
            inner_type_name: std::any::type_name::<T>(),
        }
    }

    pub fn downcast<Ev: Any + Send + Sync + Clone>(self) -> anyhow::Result<Ev> {
        #[cfg(debug_assertions)]
        let type_name = self.inner_type_name;
        #[cfg(not(debug_assertions))]
        let type_name = "unknown";

        let v: Box<Ev> = self.inner.downcast().ok().with_context(|| {
            format!(
                "could not downcast AnyEvent{{{type_name}}} to '{}'",
                std::any::type_name::<Ev>()
            )
        })?;
        Ok(*v)
    }

    pub fn downcast_ref<Ev: Any + Send + Sync + Clone>(&self) -> Option<&Ev> {
        self.inner.downcast_ref::<Ev>()
    }
}

lazy_static::lazy_static! {
    static ref WAKER: Waker = unsafe { Waker::from_raw(RawWaker::from(Arc::new(DummyWaker)))};
}

/// Exhaust a stream until polling returns pending or ends.
///
/// Returns the stream and the gathered items.
///
/// Useful for getting the starting values of a view.
pub fn exhaust<T, St>(mut stream: St) -> (St, Vec<T>)
where
    St: Stream<Item = T> + Send + Unpin + 'static,
{
    let mut items = vec![];
    let mut cx = std::task::Context::from_waker(&WAKER);
    while let std::task::Poll::Ready(Some(t)) = stream.poll_next(&mut cx) {
        items.push(t);
    }
    (stream, items)
}

/// Try to get an available `T` from the given stream by polling it.
///
/// This proxies to [`futures_lite::stream::StreamExt::poll_next`].
pub fn try_next<T, V: View, St: Stream<Item = T> + Unpin>(
    stream: &mut St,
) -> std::task::Poll<Option<T>> {
    let raw_waker = RawWaker::from(Arc::new(DummyWaker));
    let waker = unsafe { Waker::from_raw(raw_waker) };
    let mut cx = std::task::Context::from_waker(&waker);

    stream.poll_next(&mut cx)
}

#[cfg(test)]
mod exhaust {
    use std::pin::Pin;

    use crate::{stream::{Stream, StreamExt}, view::exhaust};

    #[test]
    fn exhaust_items() {
        let stream: Pin<Box<dyn Stream<Item = usize> + Send + Sync>> = Box::pin(
            futures_lite::stream::iter(vec![0, 1, 2])
                .chain(futures_lite::stream::once(3))
                .chain(futures_lite::stream::once(4))
                .chain(futures_lite::stream::unfold(Some(()), |mut seed| async move {
                    seed.take()?;
                    let _ = crate::time::wait_millis(2).await;
                    Some((5, None))
                }))
                .chain(futures_lite::stream::once(6))
                .chain(futures_lite::stream::once(7))
                .chain(futures_lite::stream::once(8)),
        );

        let (mut stream, items): (_, Vec<usize>) = exhaust(stream);
        assert_eq!(items, vec![0, 1, 2, 3, 4]);

        futures_lite::future::block_on(async {
            let n = stream.next().await.unwrap();
            assert_eq!(5, n);
        });

        let (_stream, items): (_, Vec<usize>) = exhaust(stream);
        assert_eq!(items, vec![6, 7, 8]);
    }
}

/// An enumeration of values that ViewBuilders accept.
pub enum MogwaiValue<S, St> {
    /// An owned string.
    Owned(S),
    /// A stream of values.
    Stream(St),
    /// An owned value and a stream of values.
    OwnedAndStream(S, St),
}

pub type PinBoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send + 'static>>;

impl From<bool> for MogwaiValue<bool, PinBoxStream<bool>> {
    fn from(b: bool) -> Self {
        MogwaiValue::Owned(b)
    }
}

impl<'a> From<&'a str> for MogwaiValue<String, PinBoxStream<String>> {
    fn from(s: &'a str) -> Self {
        MogwaiValue::Owned(s.into())
    }
}

impl From<&String> for MogwaiValue<String, PinBoxStream<String>> {
    fn from(s: &String) -> Self {
        MogwaiValue::Owned(s.into())
    }
}

impl From<String> for MogwaiValue<String, PinBoxStream<String>> {
    fn from(s: String) -> Self {
        MogwaiValue::Owned(s)
    }
}

impl<S, St> From<St> for MogwaiValue<S, St>
where
    S: Send + 'static,
    St: Stream<Item = S>,
{
    fn from(s: St) -> Self {
        MogwaiValue::Stream(s)
    }
}

impl<'a, St> From<(&'a str, St)> for MogwaiValue<String, St>
where
    St: Stream<Item = String>,
{
    fn from(s: (&'a str, St)) -> Self {
        MogwaiValue::OwnedAndStream(s.0.to_owned(), s.1)
    }
}

impl<'a, St> From<(String, St)> for MogwaiValue<String, St>
where
    St: Stream<Item = String>,
{
    fn from(s: (String, St)) -> Self {
        MogwaiValue::OwnedAndStream(s.0, s.1)
    }
}

impl<S: 'static, St: Stream<Item = S> + 'static> From<MogwaiValue<S, St>>
    for Pin<Box<dyn Stream<Item = S>>>
{
    fn from(v: MogwaiValue<S, St>) -> Self {
        match v {
            MogwaiValue::Owned(s) => Box::pin(futures_lite::stream::iter(std::iter::once(s))),
            MogwaiValue::Stream(s) => Box::pin(s),
            MogwaiValue::OwnedAndStream(s, st) => {
                Box::pin(futures_lite::stream::iter(std::iter::once(s)).chain(st))
            }
        }
    }
}

impl<S: Send + 'static, St: Stream<Item = S> + Send + 'static> From<MogwaiValue<S, St>>
    for Pin<Box<dyn Stream<Item = S> + Send + 'static>>
{
    fn from(v: MogwaiValue<S, St>) -> Self {
        match v {
            MogwaiValue::Owned(s) => Box::pin(futures_lite::stream::iter(std::iter::once(s))),
            MogwaiValue::Stream(s) => Box::pin(s),
            MogwaiValue::OwnedAndStream(s, st) => {
                Box::pin(futures_lite::stream::iter(std::iter::once(s)).chain(st))
            }
        }
    }
}

/// The starting identity of a view.
#[derive(Debug)]
pub enum ViewIdentity {
    Branch(String),
    NamespacedBranch(String, String),
    Leaf(String),
}

pub type MogwaiFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
pub type MogwaiStream<T> = Pin<Box<dyn Stream<Item = T> + Send + 'static>>;
pub type MogwaiSink<T> = Box<dyn Sink<T> + Send + Sync + Unpin + 'static>;
pub type PostBuild = Box<dyn FnOnce(&mut AnyView) -> anyhow::Result<()> + Send + Sync + 'static>;

/// All the updates that a view can undergo.
#[derive(Debug)]
pub enum Update {
    Text(String),
    Attribute(HashPatch<String, String>),
    BooleanAttribute(HashPatch<String, bool>),
    Style(HashPatch<String, String>),
    Child(ListPatch<ViewBuilder>),
}

//impl std::fmt::Debug for Update {
//    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//        match self {
//            Self::Text(arg0) => f.debug_tuple("Text").field(arg0).finish(),
//            Self::Attribute(arg0) => f.debug_tuple("Attribute").field(arg0).finish(),
//            Self::BooleanAttribute(arg0) => f.debug_tuple("BooleanAttribute").field(arg0).finish(),
//            Self::Style(arg0) => f.debug_tuple("Style").field(arg0).finish(),
//            Self::Child(arg0) => f.debug_tuple("Child").field(arg0).finish(),
//        }
//    }
//}

/// A listener (sink) of certain events.
///
/// In some domains like the web, events have string names that can be used
/// to subscribe to them. In other domains (like those in languages with sum types)
/// the name doesn't matter, and you may simply filter based on the enum's variant.
pub struct Listener {
    pub event_name: String,
    pub event_target: String,
    pub sink: MogwaiSink<AnyEvent>,
}

impl std::fmt::Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Listener {
            event_name,
            event_target,
            sink: _,
        } = self;
        f.debug_struct("Listener")
            .field("event_name", event_name)
            .field("event_target", event_target)
            .field("sink", &())
            .finish()
    }
}

/// An un-built mogwai view.
/// A ViewBuilder is a generic view representation.
/// It is the the blueprint of a view - everything needed to create or hydrate the view.
pub struct ViewBuilder {
    /// The identity of the view.
    ///
    /// Either a name or a tuple of a name and a namespace.
    pub identity: ViewIdentity,
    /// All declarative updates this view will undergo.
    pub updates: Vec<MogwaiStream<Update>>,
    /// Post build operations/computations that run and mutate the view after initialization.
    pub post_build_ops: Vec<PostBuild>,
    /// Sinks that want a clone of the view once it is initialized.
    pub view_sinks: Vec<MogwaiSink<AnyView>>,
    /// All event listeners (event sinks)
    pub listeners: Vec<Listener>,
    /// Asynchronous tasks that run after the view has been initialized.
    pub tasks: Vec<MogwaiFuture<()>>,
}

impl std::fmt::Debug for ViewBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewBuilder")
            .field("identity", &self.identity)
            .field("updates", &format!("vec len={}", self.updates.len()))
            .field(
                "post_build_ops",
                &format!("vec len={}", self.post_build_ops.len()),
            )
            .field("view_sinks", &format!("vec len={}", self.view_sinks.len()))
            .field("tasks", &format!("vec len={}", self.tasks.len()))
            .finish()
    }
}

impl ViewBuilder {
    /// Returns whether this builder is a leaf element, ie _not_ a container element.
    pub fn is_leaf(&self) -> bool {
        matches!(self.identity, ViewIdentity::Leaf(_))
    }

    /// Create a new container element builder.
    pub fn element(tag: impl Into<String>) -> Self {
        ViewBuilder {
            identity: ViewIdentity::Branch(tag.into()),
            updates: Default::default(),
            post_build_ops: vec![],
            view_sinks: vec![],
            listeners: vec![],
            tasks: vec![],
        }
    }

    /// Create a new namespaced container element builder.
    pub fn element_ns(tag: impl Into<String>, ns: impl Into<String>) -> Self {
        ViewBuilder {
            identity: ViewIdentity::NamespacedBranch(tag.into(), ns.into()),
            updates: Default::default(),
            post_build_ops: vec![],
            view_sinks: vec![],
            listeners: vec![],
            tasks: vec![],
        }
    }

    /// Create a new node builder.
    pub fn text<St: Stream<Item = String> + Send + 'static>(
        st: impl Into<MogwaiValue<String, St>>,
    ) -> Self {
        let (st, texts) = exhaust(PinBoxStream::from(st.into()));
        let identity = texts
            .into_iter()
            .fold(None, |_, text| Some(text))
            .unwrap_or_else(|| String::new());
        let updates = vec![Box::pin(st.map(Update::Text)) as MogwaiStream<_>];

        ViewBuilder {
            identity: ViewIdentity::Leaf(identity),
            updates,
            post_build_ops: vec![],
            tasks: vec![],
            listeners: vec![],
            view_sinks: vec![],
        }
    }

    /// Adds an asynchronous task.
    pub fn with_task(mut self, f: impl Future<Output = ()> + Send + 'static) -> Self {
        self.tasks.push(Box::pin(f));
        self
    }

    /// Add a stream to set the text of this builder.
    pub fn with_text_stream<St: Stream<Item = String> + Send + 'static>(
        mut self,
        st: impl Into<MogwaiValue<String, St>>,
    ) -> Self {
        let mv = st.into();
        let st = PinBoxStream::<String>::from(mv);
        self.updates.push(Box::pin(st.map(Update::Text)));
        self
    }

    /// Add a stream to patch the attributes of this builder.
    pub fn with_attrib_stream<St: Stream<Item = HashPatch<String, String>> + Send + 'static>(
        mut self,
        st: impl Into<MogwaiValue<HashPatch<String, String>, St>>,
    ) -> Self {
        self.updates.push(Box::pin(
            PinBoxStream::from(st.into()).map(Update::Attribute),
        ));
        self
    }

    /// Add a stream to patch a single attribute of this builder.
    pub fn with_single_attrib_stream<St: Stream<Item = String> + Send + 'static>(
        mut self,
        k: impl Into<String>,
        st: impl Into<MogwaiValue<String, St>>,
    ) -> Self {
        let key = k.into();
        let st = PinBoxStream::from(st.into()).map(move |v| HashPatch::Insert(key.clone(), v));
        self.updates.push(Box::pin(st.map(Update::Attribute)));
        self
    }

    /// Add a stream to patch the boolean attributes of this builder.
    pub fn with_bool_attrib_stream<St: Stream<Item = HashPatch<String, bool>> + Send + 'static>(
        mut self,
        st: impl Into<MogwaiValue<HashPatch<String, bool>, St>>,
    ) -> Self {
        self.updates.push(Box::pin(
            PinBoxStream::from(st.into()).map(Update::BooleanAttribute),
        ));
        self
    }

    /// Add a stream to patch a single boolean attribute of this builder.
    pub fn with_single_bool_attrib_stream<St: Stream<Item = bool> + Send + 'static>(
        mut self,
        k: impl Into<String>,
        st: impl Into<MogwaiValue<bool, St>>,
    ) -> Self {
        let key = k.into();
        let st = PinBoxStream::from(st.into())
            .map(move |b| Update::BooleanAttribute(HashPatch::Insert(key.clone(), b)));
        self.updates.push(Box::pin(st));
        self
    }

    /// Add a stream to patch the style attribute of this builder.
    pub fn with_style_stream<St: Stream<Item = String> + Send + 'static>(
        mut self,
        st: impl Into<MogwaiValue<String, St>>,
    ) -> Self {
        let st = PinBoxStream::from(st.into()).flat_map(|v: String| {
            let kvs = str::split(&v, ';')
                .filter_map(|style| {
                    let (k, v) = style.split_once(':')?;
                    Some(Update::Style(HashPatch::Insert(
                        k.trim().to_string(),
                        v.trim().to_string(),
                    )))
                })
                .collect::<Vec<_>>();
            futures_lite::stream::iter(kvs)
        });
        self.updates.push(Box::pin(st));
        self
    }

    /// Add a stream to patch a single style of this builder.
    pub fn with_single_style_stream<St: Stream<Item = String> + Send + 'static>(
        mut self,
        k: impl Into<String>,
        st: impl Into<MogwaiValue<String, St>>,
    ) -> Self {
        let key = k.into();
        let st = PinBoxStream::from(st.into());
        let st = st.map(move |v| Update::Style(HashPatch::Insert(key.clone(), v)));
        self.updates.push(Box::pin(st));
        self
    }

    /// Add a stream to patch the list of children of this builder.
    pub fn with_child_stream<St: Stream<Item = ListPatch<ViewBuilder>> + Send + 'static>(
        mut self,
        st: impl Into<MogwaiValue<ListPatch<ViewBuilder>, St>>,
    ) -> Self {
        self.updates
            .push(Box::pin(PinBoxStream::from(st.into()).map(Update::Child)));
        self
    }

    /// Append a child or iterator of children.
    pub fn append(self, children: impl Into<AppendArg>) -> Self {
        let arg = children.into();

        let bldrs = match arg {
            AppendArg::Single(bldr) => vec![bldr],
            AppendArg::Iter(bldrs) => bldrs,
        };
        let stream = Box::pin(futures_lite::stream::iter(
            bldrs.into_iter().map(|b| ListPatch::push(b)),
        ));
        self.with_child_stream(stream)
    }

    /// Add an operation to perform after the view has been built.
    pub fn with_post_build<V, F>(mut self, f: F) -> Self
    where
        V: View,
        F: FnOnce(&mut V) -> anyhow::Result<()> + Send + Sync + 'static,
    {
        let g = |any_view: &mut AnyView| {
            let v: &mut V = any_view
                .downcast_mut::<V>()
                .context("cannot downcast_mut this AnyView")?;
            f(v)
        };
        self.post_build_ops.push(Box::new(g) as PostBuild);
        self
    }

    /// Send a clone of the inner view once it is built.
    ///
    /// Wraps `V` in `AnyView` to erase its type until it is built.
    ///
    /// ## Panics
    /// Panics if the `AnyView` cannot be downcast back into `V`.
    pub fn with_capture_view<V: View>(
        mut self,
        sink: impl Sink<V> + Unpin + Send + Sync + 'static,
    ) -> Self {
        let sink: MogwaiSink<AnyView> =
            Box::new(sink.contra_map(|any_view: AnyView| any_view.downcast::<V>().unwrap()));
        self.view_sinks.push(sink);
        self
    }

    /// Capture the view and update it using the given update function for each value
    /// that comes from the given stream.
    ///
    /// The only parameter is a tuple to support being used from the [`rsx`](crate::rsx) macro's
    /// `capture:for_each` attribute, since the right hand side of such attributes must be
    /// a singular Rust expression:
    /// ```rust, ignore
    /// use mogwai_dom::prelude::*;
    /// let (_tx, rx) = mogwai_dom::core::channel::mpsc::bounded::<usize>(1);
    /// let builder = rsx! {
    ///     input(
    ///         type = "text",
    ///         capture:for_each = (
    ///             rx.map(|n:usize| format!("{}", n)),
    ///             JsDom::try_to(web_sys::HtmlInputElement::set_value)
    ///         )
    ///     ) {}
    /// };
    /// ```
    ///
    /// And the above RSX is equivalent to the following:
    /// ```rust, ignore, no_run
    /// let st = rx.map(|n:usize| format!("{}", n));
    /// let f = JsDom::try_to(web_sys::HtmlInputElement::set_value);
    /// let captured = crate::futures_lite::Captured::default();
    /// let builder = ViewBuilder::default()
    ///     .with_capture_view(captured.sink())
    ///     .with_task(async move {
    ///         let view = captured.get().await;
    ///         while let Some(value) = st.next().await {
    ///             f(&view, value);
    ///         }
    ///     })
    /// ```
    pub fn with_capture_for_each<T, V: View>(
        self,
        (mut st, f): (
            impl Stream<Item = T> + Send + Unpin + 'static,
            impl Fn(&V, T) + Send + 'static,
        ),
    ) -> Self {
        let captured = crate::future::Captured::<V>::default();
        self.with_capture_view(captured.sink())
            .with_task(async move {
                let view = captured.get().await;
                while let Some(value) = st.next().await {
                    f(&view, value);
                }
            })
    }

    /// Add a sink into which view events of the given name will be sent.
    ///
    /// ## Panics
    /// If the domain specific view cannot be downcast a panic will happen when
    /// the boxed view is sent into the sink.
    pub fn with_event<Event: Any + Send + Sync + Unpin + Clone>(
        mut self,
        name: impl Into<String>,
        target: impl Into<String>,
        si: impl Sink<Event> + Send + Sync + Unpin + 'static,
    ) -> Self {
        let sink = Box::new(si.contra_map(|any: AnyEvent| {
            let event: Event = any.downcast::<Event>().unwrap();
            event
        }));

        let listener = Listener {
            event_name: name.into(),
            event_target: target.into(),
            sink,
        };

        self.listeners.push(listener);
        self
    }
}

/// An enumeration of types that can be appended as children to [`ViewBuilder`].
pub enum AppendArg {
    /// A single static child.
    Single(ViewBuilder),
    /// A collection of static children.
    Iter(Vec<ViewBuilder>),
}

impl<T> From<Vec<T>> for AppendArg
where
    ViewBuilder: From<T>,
{
    fn from(bldrs: Vec<T>) -> Self {
        AppendArg::Iter(bldrs.into_iter().map(ViewBuilder::from).collect())
    }
}

impl From<&String> for ViewBuilder {
    fn from(s: &String) -> Self {
        ViewBuilder::text(futures_lite::stream::iter(std::iter::once(s.clone())))
    }
}

impl From<String> for ViewBuilder {
    fn from(s: String) -> Self {
        ViewBuilder::text(futures_lite::stream::iter(std::iter::once(s)))
    }
}

impl From<&str> for ViewBuilder {
    fn from(s: &str) -> Self {
        ViewBuilder::text(futures_lite::stream::iter(std::iter::once(s.to_string())))
    }
}

impl<S, St> From<(S, St)> for ViewBuilder
where
    S: AsRef<str>,
    St: Stream<Item = String> + Send + Sync + 'static,
{
    fn from((s, st): (S, St)) -> Self {
        let iter = futures_lite::stream::iter(std::iter::once(s.as_ref().to_string())).chain(st);
        ViewBuilder::text(iter)
    }
}

impl<T: Into<ViewBuilder>> From<T> for AppendArg {
    fn from(t: T) -> Self {
        AppendArg::Single(t.into())
    }
}

impl<T> From<Option<T>> for AppendArg
where
    ViewBuilder: From<T>,
{
    fn from(may_vb: Option<T>) -> Self {
        AppendArg::Iter(
            may_vb
                .into_iter()
                .map(ViewBuilder::from)
                .collect::<Vec<_>>(),
        )
    }
}
