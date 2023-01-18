//! Domain agnostic view doclaration.
use std::{
    any::Any,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{RawWaker, Wake, Waker},
};

use crate::{
    patch::{HashPatch, ListPatch},
    sink::{Sink, SinkExt},
    stream::{Stream, StreamExt},
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

/// Downcasts various generic view types into specific view types.
pub trait Downcast<V> {
    fn downcast(self) -> anyhow::Result<V>;
}

impl<T> Downcast<T> for T {
    fn downcast(self) -> anyhow::Result<T> {
        Ok(self)
    }
}

/// A type erased view.
///
/// Used to write view builders in a domain-agnostic way.
pub struct AnyView {
    pub inner: Box<dyn Any + Send + Sync>,
    clone_fn: fn(&AnyView) -> AnyView,
    #[cfg(debug_assertions)]
    pub inner_type_name: &'static str,
}

impl std::fmt::Debug for AnyView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnyView")
            .field("inner_type", &format!("{}", self.inner_type_name()))
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

fn any_view_clone<V: View>(any_view: &AnyView) -> AnyView
where
    AnyView: Downcast<V>,
{
    let v: &V = any_view.downcast_ref().unwrap();
    AnyView {
        inner: Box::new(v.clone()) as Box<dyn Any + Send + Sync>,
        clone_fn: any_view_clone::<V>,
        #[cfg(debug_assertions)]
        inner_type_name: std::any::type_name::<V>(),
    }
}

impl AnyView {
    pub fn new<V>(inner: V) -> Self
    where
        V: View,
        AnyView: Downcast<V>,
    {
        AnyView {
            inner: Box::new(inner),
            clone_fn: any_view_clone::<V>,
            #[cfg(debug_assertions)]
            inner_type_name: std::any::type_name::<V>(),
        }
    }

    pub fn inner_type_name(&self) -> &'static str {
        #[cfg(not(debug_assertions))]
        let type_name = "unknown w/o debug_assertions";
        #[cfg(debug_assertions)]
        let type_name = self.inner_type_name;

        type_name
    }

    pub fn downcast_ref<T: View>(&self) -> Option<&T> {
        self.inner.downcast_ref::<T>()
    }

    pub fn downcast_mut<T: View>(&mut self) -> Option<&mut T> {
        self.inner.downcast_mut::<T>()
    }
}

fn any_event_clone<T: Any + Clone + Send + Sync>(any_event: &AnyEvent) -> AnyEvent
where
    AnyEvent: Downcast<T>,
{
    let ev: &T = any_event.downcast_ref().unwrap();
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
    pub inner: Box<dyn Any + Send + Sync>,
    clone_fn: fn(&AnyEvent) -> AnyEvent,
    #[cfg(debug_assertions)]
    pub inner_type_name: &'static str,
}

impl Clone for AnyEvent {
    fn clone(&self) -> Self {
        (self.clone_fn)(self)
    }
}

impl AnyEvent {
    pub fn new<T>(inner: T) -> Self
    where
        T: Any + Send + Sync + Clone,
        AnyEvent: Downcast<T>,
    {
        AnyEvent {
            inner: Box::new(inner),
            clone_fn: any_event_clone::<T>,
            #[cfg(debug_assertions)]
            inner_type_name: std::any::type_name::<T>(),
        }
    }

    pub fn downcast_ref<T: Any + Send + Sync + Clone>(&self) -> Option<&T> {
        self.inner.downcast_ref::<T>()
    }

    pub fn downcast_mut<T: Any + Send + Sync + Clone>(&mut self) -> Option<&mut T> {
        self.inner.downcast_mut::<T>()
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

    use crate::{
        stream::{Stream, StreamExt},
        view::exhaust,
    };

    #[test]
    fn exhaust_items() {
        let stream: Pin<Box<dyn Stream<Item = usize> + Send + Sync>> = Box::pin(
            futures_lite::stream::iter(vec![0, 1, 2])
                .chain(futures_lite::stream::once(3))
                .chain(futures_lite::stream::once(4))
                .chain(futures_lite::stream::unfold(
                    Some(()),
                    |mut seed| async move {
                        seed.take()?;
                        let _ = crate::time::wait_millis(2).await;
                        Some((5, None))
                    },
                ))
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

impl<T, St: Stream + Send + 'static> MogwaiValue<T, St> {
    pub fn pinned(self) -> MogwaiValue<T, PinBoxStream<St::Item>> {
        match self {
            MogwaiValue::Owned(s) => MogwaiValue::Owned(s),
            MogwaiValue::Stream(st) => MogwaiValue::Stream(Box::pin(st)),
            MogwaiValue::OwnedAndStream(s, st) => MogwaiValue::OwnedAndStream(s, Box::pin(st)),
        }
    }

    /// Split into a possible current value and future values.
    ///
    /// If there is no current value the first element will be `None`.
    ///
    /// If there is _only_ a current value the second element will be `None`.
    pub fn split(self) -> (Option<T>, Option<St>) {
        match self {
            MogwaiValue::Owned(s) => (Some(s), None),
            MogwaiValue::Stream(st) => (None, Some(st)),
            MogwaiValue::OwnedAndStream(s, st) => (Some(s), Some(st)),
        }
    }
}

impl<T, St: Stream<Item = T> + Send + 'static> MogwaiValue<T, St> {
    pub fn map<S>(self, f: impl Fn(T) -> S + Send + 'static) -> MogwaiValue<S, PinBoxStream<S>> {
        match self {
            MogwaiValue::Owned(s) => MogwaiValue::Owned(f(s)),
            MogwaiValue::Stream(st) => MogwaiValue::Stream(Box::pin(st.map(f))),
            MogwaiValue::OwnedAndStream(s, st) => {
                MogwaiValue::OwnedAndStream(f(s), Box::pin(st.map(f)))
            }
        }
    }
}

pub type PinBoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send + 'static>>;

impl From<bool> for MogwaiValue<bool, PinBoxStream<bool>> {
    fn from(b: bool) -> Self {
        MogwaiValue::Owned(b)
    }
}

impl From<&'static str> for MogwaiValue<&'static str, PinBoxStream<String>> {
    fn from(s: &'static str) -> Self {
        MogwaiValue::Owned(s)
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

impl<St> From<(&'static str, St)> for MogwaiValue<&'static str, St>
where
    St: Stream<Item = String>,
{
    fn from(s: (&'static str, St)) -> Self {
        MogwaiValue::OwnedAndStream(s.0, s.1)
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

/// The starting identity of a view.
#[derive(Debug)]
pub enum ViewIdentity {
    Branch(&'static str),
    NamespacedBranch(&'static str, &'static str),
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

/// A listener (sink) of certain events.
///
/// In some domains like the web, events have string names that can be used
/// to subscribe to them. In other domains (like those in languages with sum
/// types) the name doesn't matter, and you may simply filter based on the
/// enum's variant.
pub struct Listener {
    pub event_name: &'static str,
    pub event_target: &'static str,
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
/// It is the the blueprint of a view - everything needed to create or hydrate
/// the view.
pub struct ViewBuilder {
    /// The identity of the view.
    ///
    /// Either a name or a tuple of a name and a namespace.
    pub identity: ViewIdentity,
    /// All initial values this view has at build time.
    pub initial_values: Vec<Update>,
    /// All declarative updates this view will undergo.
    pub updates: Vec<MogwaiStream<Update>>,
    /// Post build operations/computations that run and mutate the view after
    /// initialization.
    pub post_build_ops: Vec<PostBuild>,
    /// Sinks that want a clone of the view once it is initialized.
    pub view_sinks: Vec<MogwaiSink<AnyView>>,
    /// All event listeners (event sinks)
    pub listeners: Vec<Listener>,
    /// Asynchronous tasks that run after the view has been initialized.
    pub tasks: Vec<MogwaiFuture<()>>,
    /// A pre-built view node to use as the root.
    ///
    /// This is good for hydrating pre-rendered nodes and for optimizations.
    pub hydration_root: Option<AnyView>,
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
    /// Returns whether this builder is a leaf element, ie _not_ a container
    /// element.
    pub fn is_leaf(&self) -> bool {
        matches!(self.identity, ViewIdentity::Leaf(_))
    }

    /// Create a new container element builder.
    pub fn element(tag: &'static str) -> Self {
        ViewBuilder {
            identity: ViewIdentity::Branch(tag),
            initial_values: Default::default(),
            updates: Default::default(),
            post_build_ops: vec![],
            view_sinks: vec![],
            listeners: vec![],
            tasks: vec![],
            hydration_root: None,
        }
    }

    /// Create a new namespaced container element builder.
    pub fn element_ns(tag: &'static str, ns: &'static str) -> Self {
        ViewBuilder {
            identity: ViewIdentity::NamespacedBranch(tag, ns),
            initial_values: Default::default(),
            updates: Default::default(),
            post_build_ops: vec![],
            view_sinks: vec![],
            listeners: vec![],
            tasks: vec![],
            hydration_root: None,
        }
    }

    /// Create a new node builder.
    pub fn text<St: Stream<Item = String> + Send + 'static>(
        st: impl Into<MogwaiValue<String, St>>,
    ) -> Self {
        let (may_s, may_st) = st.into().split();
        let identity = may_s.unwrap_or_default();
        let mut updates: Vec<PinBoxStream<Update>> = vec![];
        if let Some(st) = may_st {
            updates.push(Box::pin(st.map(Update::Text)));
        }

        ViewBuilder {
            identity: ViewIdentity::Leaf(identity),
            initial_values: vec![],
            updates,
            post_build_ops: vec![],
            tasks: vec![],
            listeners: vec![],
            view_sinks: vec![],
            hydration_root: None,
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
        let (may_text, may_st) = st.into().split();
        if let Some(text) = may_text {
            self.identity = ViewIdentity::Leaf(text);
        }
        if let Some(st) = may_st {
            self.updates.push(Box::pin(st.map(Update::Text)));
        }
        self
    }

    /// Add a stream to patch the attributes of this builder.
    pub fn with_attrib_stream<St: Stream<Item = HashPatch<String, String>> + Send + 'static>(
        mut self,
        st: impl Into<MogwaiValue<HashPatch<String, String>, St>>,
    ) -> Self {
        let (may_patch, may_st) = st.into().split();
        if let Some(patch) = may_patch {
            self.initial_values.push(Update::Attribute(patch));
        }
        if let Some(st) = may_st {
            self.updates.push(Box::pin(st.map(Update::Attribute)));
        }
        self
    }

    /// Add a stream to patch a single attribute of this builder.
    pub fn with_single_attrib_stream<St: Stream<Item = String> + Send + 'static>(
        mut self,
        k: impl Into<String>,
        st: impl Into<MogwaiValue<String, St>>,
    ) -> Self {
        let key = k.into();
        let (may_val, may_st) = st.into().split();
        if let Some(val) = may_val {
            self.initial_values
                .push(Update::Attribute(HashPatch::Insert(key.clone(), val)));
        }
        if let Some(st) = may_st {
            self.updates.push(Box::pin(
                st.map(move |v| Update::Attribute(HashPatch::Insert(key.clone(), v))),
            ));
        }
        self
    }

    /// Add a stream to patch the boolean attributes of this builder.
    pub fn with_bool_attrib_stream<St: Stream<Item = HashPatch<String, bool>> + Send + 'static>(
        mut self,
        st: impl Into<MogwaiValue<HashPatch<String, bool>, St>>,
    ) -> Self {
        let (may_patch, may_st) = st.into().split();
        if let Some(patch) = may_patch {
            self.initial_values.push(Update::BooleanAttribute(patch));
        }
        if let Some(st) = may_st {
            self.updates
                .push(Box::pin(st.map(Update::BooleanAttribute)));
        }

        self
    }

    /// Add a stream to patch a single boolean attribute of this builder.
    pub fn with_single_bool_attrib_stream<St: Stream<Item = bool> + Send + 'static>(
        mut self,
        k: impl Into<String>,
        st: impl Into<MogwaiValue<bool, St>>,
    ) -> Self {
        let key = k.into();
        let (may_val, may_st) = st.into().split();
        if let Some(val) = may_val {
            self.initial_values
                .push(Update::BooleanAttribute(HashPatch::Insert(
                    key.clone(),
                    val,
                )));
        }
        if let Some(st) = may_st {
            self.updates.push(Box::pin(st.map(move |v| {
                Update::BooleanAttribute(HashPatch::Insert(key.clone(), v))
            })));
        }

        self
    }

    /// Add a stream to patch the style attribute of this builder.
    pub fn with_style_stream<St: Stream<Item = String> + Send + 'static>(
        mut self,
        st: impl Into<MogwaiValue<String, St>>,
    ) -> Self {
        fn split_style(
            s: &String,
        ) -> std::iter::FilterMap<std::str::Split<'_, char>, fn(&str) -> Option<Update>> {
            str::split(s, ';').filter_map(|style| {
                let (k, v) = style.split_once(':')?;
                Some(Update::Style(HashPatch::Insert(
                    k.trim().to_string(),
                    v.trim().to_string(),
                )))
            })
        }

        let (may_style, may_st) = st.into().split();
        if let Some(style) = may_style {
            self.initial_values.extend(split_style(&style));
        }
        if let Some(st) = may_st {
            self.updates.push(Box::pin(st.flat_map(|s| {
                futures_lite::stream::iter(split_style(&s).collect::<Vec<_>>())
            })));
        }
        self
    }

    /// Add a stream to patch a single style of this builder.
    pub fn with_single_style_stream<St: Stream<Item = String> + Send + 'static>(
        mut self,
        k: impl Into<String>,
        st: impl Into<MogwaiValue<String, St>>,
    ) -> Self {
        let key = k.into();
        let (may_style, may_st) = st.into().split();
        if let Some(style) = may_style {
            self.initial_values
                .push(Update::Style(HashPatch::Insert(key.clone(), style)));
        }
        if let Some(st) = may_st {
            self.updates.push(Box::pin(
                st.map(move |v| Update::Style(HashPatch::Insert(key.clone(), v))),
            ));
        }
        self
    }

    /// Add a stream to patch the list of children of this builder.
    pub fn with_child_stream<St: Stream<Item = ListPatch<ViewBuilder>> + Send + 'static>(
        mut self,
        st: impl Into<MogwaiValue<ListPatch<ViewBuilder>, St>>,
    ) -> Self {
        let (may_patch, may_st) = st.into().split();
        if let Some(patch) = may_patch {
            self.initial_values.push(Update::Child(patch));
        }
        if let Some(st) = may_st {
            self.updates.push(Box::pin(st.map(Update::Child)));
        }
        self
    }

    /// Append a child or iterator of children.
    pub fn append(mut self, children: impl Into<AppendArg>) -> Self {
        let arg = children.into();

        match arg {
            AppendArg::Single(bldr) => {
                self.initial_values
                    .push(Update::Child(ListPatch::push(bldr)));
            }
            AppendArg::Iter(bldrs) => {
                self.initial_values
                    .extend(bldrs.into_iter().map(|b| Update::Child(ListPatch::push(b))));
            }
        }

        self
    }

    /// Add an operation to perform after the view has been built.
    pub fn with_post_build<V, F>(mut self, f: F) -> Self
    where
        V: View,
        AnyView: Downcast<V>,
        F: FnOnce(&mut V) -> anyhow::Result<()> + Send + Sync + 'static,
    {
        let g = |any_view: &mut AnyView| {
            let type_name = any_view.inner_type_name();
            let v: &mut V = any_view.downcast_mut().with_context(|| {
                format!(
                    "cannot downcast_mut this AnyView{{{}}} to {}",
                    type_name,
                    std::any::type_name::<V>()
                )
            })?;
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
    ) -> Self
    where
        AnyView: Downcast<V>,
    {
        let sink: MogwaiSink<AnyView> =
            Box::new(sink.contra_map(|any_view: AnyView| any_view.downcast().unwrap()));
        self.view_sinks.push(sink);
        self
    }

    /// Capture the view and update it using the given update function for each
    /// value that comes from the given stream.
    ///
    /// The only parameter is a tuple to support being used from the
    /// [`rsx`](crate::rsx) macro's `capture:for_each` attribute, since the
    /// right hand side of such attributes must be a singular Rust
    /// expression:
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
    ) -> Self
    where
        AnyView: Downcast<V>,
    {
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
        name: &'static str,
        target: &'static str,
        si: impl Sink<Event> + Send + Sync + Unpin + 'static,
    ) -> Self
    where
        AnyEvent: Downcast<Event>,
    {
        let sink = Box::new(si.contra_map(|any: AnyEvent| {
            let event: Event = any.downcast().unwrap();
            event
        }));

        let listener = Listener {
            event_name: name,
            event_target: target.into(),
            sink,
        };

        self.listeners.push(listener);
        self
    }

    /// Use the given view node instead of creating a new node from scratch.
    ///
    /// This is used for hydrating reactivity from a pre-rendered or "ossified"
    /// node.
    pub fn with_hydration_root<V: View>(mut self, view: V) -> Self
    where
        AnyView: Downcast<V>,
    {
        self.hydration_root = Some(AnyView::new(view));
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

impl<St> From<(String, St)> for ViewBuilder
where
    St: Stream<Item = String> + Send + Sync + 'static,
{
    fn from(tuple: (String, St)) -> Self {
        ViewBuilder::text(tuple)
    }
}

impl<'a, St> From<(&'a str, St)> for ViewBuilder
where
    St: Stream<Item = String> + Send + Sync + 'static,
{
    fn from(tuple: (&'a str, St)) -> Self {
        ViewBuilder::text(tuple)
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
