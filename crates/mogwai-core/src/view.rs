//! Domain agnostic view doclaration.
use std::{
    any::Any,
    pin::Pin,
    sync::Arc,
    task::{RawWaker, Wake, Waker},
};

use crate::{
    channel::SinkError,
    patch::{HashPatch, ListPatch},
    prelude::Contravariant,
};
use anyhow::Context;
pub use anyhow::Error;
use futures::{stream, Future, Sink, Stream, StreamExt};
struct DummyWaker;

impl Wake for DummyWaker {
    fn wake(self: std::sync::Arc<Self>) {}
}

/// Resources needed to build a view `V` from a [`ViewBuilder`].
pub trait ViewResources<V>
where
    V: View,
{
    /// Initialize a new view.
    fn init(&self, identity: ViewIdentity) -> anyhow::Result<V>;

    /// Convert a view builder into a view.
    fn build(&self, builder: ViewBuilder) -> anyhow::Result<V> {
        let ViewBuilder {
            identity,
            updates,
            tasks,
        } = builder;

        let element = self.init(identity)?;

        let (mut update_stream, initial_values) = exhaust(updates);

        for update in initial_values.into_iter() {
            element.update(update)?;
        }

        let node = element.clone();
        self.spawn(async move {
            while let Some(update) = update_stream.next().await {
                node.update(update).unwrap();
            }
        });

        for task in tasks.into_iter() {
            self.spawn(task);
        }

        Ok(element)
    }

    ///// Spawn an asynchronous task.
    fn spawn(&self, action: impl Future<Output = ()> + Send + 'static);
}

/// An interface for a domain-specific view.
///
/// A view should be a type that can be cheaply cloned, where clones all refer
/// to the same underlying user interface node.
pub trait View
where
    Self: Any + Sized + Clone + Unpin + Send + Sync,
{
    /// Update the view
    fn update(&self, update: Update) -> anyhow::Result<()>;
}

/// A type erased view.
///
/// Used to write view builders in a domain-agnostic way.
pub struct AnyView {
    inner: Box<dyn Any + Send + Sync>,
    clone_fn: fn(&AnyView) -> AnyView,
}

impl Clone for AnyView {
    fn clone(&self) -> Self {
        let cloned_view = (self.clone_fn)(self);
        Self {
            inner: cloned_view.inner,
            clone_fn: self.clone_fn.clone(),
        }
    }
}

pub fn any_view_clone<V: View>(any_view: &AnyView) -> AnyView {
    let v: &V = any_view.downcast_ref().unwrap();
    AnyView {
        inner: Box::new(v.clone()) as Box<dyn Any + Send + Sync>,
        clone_fn: any_view_clone::<V>
    }
}

impl AnyView {
    pub fn new<V: View>(inner: V) -> Self {
        AnyView {
            inner: Box::new(inner),
            clone_fn: any_view_clone::<V>
        }
    }

    pub fn downcast_ref<V: View>(&self) -> Option<&V> {
        self.inner.downcast_ref()
    }

    pub fn downcast<V: View>(self) -> anyhow::Result<V> {
        let v: Box<V> = self.inner.downcast().ok().with_context(|| {
            format!(
                "could not downcast AnyView to '{}'",
                std::any::type_name::<V>()
            )
        })?;
        Ok(*v)
    }
}

/// A type erased view event.
///
/// Used to write view builders in a domain-agnostic way.
pub struct AnyEvent {
    inner: Box<dyn Any + Send + Sync>,
}

impl AnyEvent {
    pub fn new(inner: impl Any + Send + Sync) -> Self {
        AnyEvent {
            inner: Box::new(inner),
        }
    }

    pub fn downcast<Ev: Any + Send + Sync>(self) -> anyhow::Result<Ev> {
        let v: Box<Ev> = self.inner.downcast().ok().with_context(|| {
            format!(
                "could not downcast AnyEvent to '{}'",
                std::any::type_name::<Ev>()
            )
        })?;
        Ok(*v)
    }
}

/// Exhaust a stream until polling returns pending or ends.
///
/// Returns the stream and the gathered items.
///
/// Useful for getting the starting values of a view.
pub fn exhaust<T, St>(mut stream: St) -> (St, Vec<T>)
where
    St: Stream<Item = T> + Unpin + Send + Sync + 'static,
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

/// Try to get an available `T` from the given stream by polling it.
///
/// This proxies to [`futures::stream::StreamExt::poll_next_unpin`].
pub fn try_next<T, V: View, St: Stream<Item = T> + Unpin>(
    stream: &mut St,
) -> std::task::Poll<Option<T>> {
    let raw_waker = RawWaker::from(Arc::new(DummyWaker));
    let waker = unsafe { Waker::from_raw(raw_waker) };
    let mut cx = std::task::Context::from_waker(&waker);

    stream.poll_next_unpin(&mut cx)
}

#[cfg(test)]
mod exhaust {
    use crate::view::exhaust;
    use futures::StreamExt;

    #[test]
    fn exhaust_items() {
        let stream = Box::pin(
            futures::stream::iter(vec![0, 1, 2])
                .chain(futures::stream::once(async { 3 }))
                .chain(futures::stream::once(async { 4 }))
                .chain(futures::stream::once(async {
                    let _ = crate::time::wait_millis(2).await;
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

/// An enumeration of values that ViewBuilders accept.
pub enum MogwaiValue<S, St> {
    /// An owned string.
    Owned(S),
    /// A stream of values.
    Stream(St),
    /// An owned value and a stream of values.
    OwnedAndStream(S, St),
}

pub type PinBoxStream<T> = Pin<Box<dyn Stream<Item = T> + Unpin + Send + Sync + 'static>>;

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
    S: Send + Sync + 'static,
    St: Stream<Item = S> + Unpin + Send + Sync + 'static,
{
    fn from(s: St) -> Self {
        MogwaiValue::Stream(s)
    }
}

impl<'a, St> From<(&'a str, St)> for MogwaiValue<String, St>
where
    St: Stream<Item = String> + Unpin + Send + Sync + 'static,
{
    fn from(s: (&'a str, St)) -> Self {
        MogwaiValue::OwnedAndStream(s.0.to_owned(), s.1)
    }
}

impl<S: Send + Sync + 'static, St: Stream<Item = S> + Unpin + Send + Sync + 'static>
    From<MogwaiValue<S, St>> for PinBoxStream<S>
{
    fn from(v: MogwaiValue<S, St>) -> Self {
        match v {
            MogwaiValue::Owned(s) => Box::pin(futures::stream::iter(std::iter::once(s))),
            MogwaiValue::Stream(s) => Box::pin(s),
            MogwaiValue::OwnedAndStream(s, st) => {
                Box::pin(futures::stream::iter(std::iter::once(s)).chain(st))
            }
        }
    }
}

/// The starting identity of a view.
pub enum ViewIdentity {
    Branch(String),
    NamespacedBranch(String, String),
    Leaf(String),
}

pub type MogwaiFuture<T> = Pin<Box<dyn Future<Output = T> + Send + Sync + 'static>>;
pub type MogwaiStream<T> = Pin<Box<dyn Stream<Item = T> + Unpin + Send + Sync + 'static>>;
pub type MogwaiSink<T> = Pin<Box<dyn Sink<T, Error = SinkError> + Unpin + Send + Sync + 'static>>;
pub type PostBuild = Box<dyn FnOnce(AnyView) -> anyhow::Result<()> + Send + Sync + 'static>;

/// All the updates that a view can undergo.
pub enum Update {
    Text(String),
    Attribute(HashPatch<String, String>),
    BooleanAttribute(HashPatch<String, bool>),
    Style(HashPatch<String, String>),
    Child(ListPatch<ViewBuilder>),
    Listener {
        event_name: String,
        event_target: String,
        sink: MogwaiSink<AnyEvent>,
    },
    PostBuild(PostBuild),
}

/// An un-built mogwai view.
/// A ViewBuilder is the most generic view representation in the mogwai library.
/// It is the the blueprint of a view - everything needed to create, hydrate or serialize the view.
pub struct ViewBuilder {
    /// The identity of the view.
    ///
    /// Either a name or a tuple of a name and a namespace.
    pub identity: ViewIdentity,
    /// All declarative updates this view will undergo.
    pub updates: stream::SelectAll<MogwaiStream<Update>>,
    ///// Post build operations/computations that run and mutate the view after initialization.
    //pub ops: Vec<Box<dyn FnOnce(&mut Box<dyn Any>) -> anyhow::Result<()> + Send + Sync + 'static>>,
    ///// Sinks that want a clone of the view once it is initialized.
    //pub view_sinks: Vec<MogwaiSink<Box<dyn Any + Send + Sync + 'static>>>,
    /// Asynchronous tasks that run after the view has been initialized.
    pub tasks: Vec<MogwaiFuture<()>>,
}

impl ViewBuilder {
    /// Create a new container element builder.
    pub fn element(tag: impl Into<String>) -> Self {
        ViewBuilder {
            identity: ViewIdentity::Branch(tag.into()),
            updates: Default::default(),
            tasks: vec![],
        }
    }

    /// Create a new namespaced container element builder.
    pub fn element_ns(tag: impl Into<String>, ns: impl Into<String>) -> Self {
        ViewBuilder {
            identity: ViewIdentity::NamespacedBranch(tag.into(), ns.into()),
            updates: Default::default(),
            tasks: vec![],
        }
    }

    /// Create a new node builder.
    pub fn text<St: Stream<Item = String> + Unpin + Send + Sync + 'static>(
        st: impl Into<MogwaiValue<String, St>>,
    ) -> Self {
        let (st, texts) = exhaust(PinBoxStream::from(st.into()));
        let identity = texts
            .into_iter()
            .fold(None, |_, text| Some(text))
            .unwrap_or_else(|| String::new());
        let mut updates: stream::SelectAll<MogwaiStream<Update>> = stream::SelectAll::default();
        updates.push(Box::pin(st.map(Update::Text)));

        ViewBuilder {
            identity: ViewIdentity::Leaf(identity),
            updates,
            tasks: vec![],
        }
    }

    /// Adds an asynchronous task.
    pub fn with_task(mut self, f: impl Future<Output = ()> + Send + Sync + 'static) -> Self {
        self.tasks.push(Box::pin(f));
        self
    }

    /// Add a stream to set the text of this builder.
    pub fn with_text_stream<St: Stream<Item = String> + Unpin + Send + Sync + 'static>(
        mut self,
        st: impl Into<MogwaiValue<String, St>>,
    ) -> Self {
        let mv = st.into();
        let st = PinBoxStream::<String>::from(mv);
        self.updates.push(Box::pin(st.map(Update::Text)));
        self
    }

    /// Add a stream to patch the attributes of this builder.
    pub fn with_attrib_stream<
        St: Stream<Item = HashPatch<String, String>> + Unpin + Send + Sync + 'static,
    >(
        mut self,
        st: impl Into<MogwaiValue<HashPatch<String, String>, St>>,
    ) -> Self {
        self.updates.push(Box::pin(
            PinBoxStream::from(st.into()).map(Update::Attribute),
        ));
        self
    }

    /// Add a stream to patch a single attribute of this builder.
    pub fn with_single_attrib_stream<St: Stream<Item = String> + Unpin + Send + Sync + 'static>(
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
    pub fn with_bool_attrib_stream<
        St: Stream<Item = HashPatch<String, bool>> + Unpin + Send + Sync + 'static,
    >(
        mut self,
        st: impl Into<MogwaiValue<HashPatch<String, bool>, St>>,
    ) -> Self {
        self.updates.push(Box::pin(
            PinBoxStream::from(st.into()).map(Update::BooleanAttribute),
        ));
        self
    }

    /// Add a stream to patch a single boolean attribute of this builder.
    pub fn with_single_bool_attrib_stream<
        St: Stream<Item = bool> + Unpin + Send + Sync + 'static,
    >(
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
    pub fn with_style_stream<St: Stream<Item = String> + Unpin + Send + Sync + 'static>(
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
            stream::iter(kvs)
        });
        self.updates.push(Box::pin(st));
        self
    }

    /// Add a stream to patch a single style of this builder.
    pub fn with_single_style_stream<St: Stream<Item = String> + Unpin + Send + Sync + 'static>(
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
    pub fn with_child_stream<
        St: Stream<Item = ListPatch<ViewBuilder>> + Unpin + Send + Sync + 'static,
    >(
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
        let stream = Box::pin(futures::stream::iter(
            bldrs.into_iter().map(|b| ListPatch::push(b)),
        ));
        self.with_child_stream(stream)
    }

    /// Add an operation to perform after the view has been built.
    pub fn with_post_build<V, F>(mut self, f: F) -> Self
    where
        V: View,
        F: FnOnce(V) -> anyhow::Result<()> + Send + Sync + 'static,
    {
        let g = |any_view: AnyView| {
            let v: V = any_view.downcast::<V>()?;
            f(v)
        };
        self.updates
            .push(Box::pin(stream::iter(std::iter::once(Update::PostBuild(
                Box::new(g),
            )))));
        self
    }

    ///// Send a clone of the inner view once it is built.
    /////
    ///// ## Panics
    ///// If the domain specific view cannot be downcast a panic will happen when
    ///// the boxed view is sent into the sink.
    //pub fn with_capture_view<V: Any + Unpin + Send + Sync>(
    //    mut self,
    //    sink: impl Sink<V, Error = SinkError> + Unpin + Send + Sync + 'static,
    //) -> Self {
    //    let sink = sink.contra_map(|any: Box<dyn Any + Send + Sync + 'static>| {
    //        let box_v: Box<V> = any.downcast::<V>().unwrap();
    //        *box_v
    //    });
    //    self.view_sinks.push(Box::pin(sink));
    //    self
    //}

    /// Add a sink into which view events of the given name will be sent.
    ///
    /// ## Panics
    /// If the domain specific view cannot be downcast a panic will happen when
    /// the boxed view is sent into the sink.
    pub fn with_event<Event: Any + Unpin + Send + Sync>(
        mut self,
        name: impl Into<String>,
        target: impl Into<String>,
        si: impl Sink<Event, Error = SinkError> + Unpin + Send + Sync + 'static,
    ) -> Self {
        let sink = Box::pin(si.contra_map(|any: AnyEvent| {
            let event: Event = any.downcast::<Event>().unwrap();
            event
        }));

        let listener = Update::Listener {
            event_name: name.into(),
            event_target: target.into(),
            sink,
        };

        self.updates
            .push(Box::pin(stream::iter(std::iter::once(listener))));
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

//impl<V> From<&String> for ViewBuilder
//where
//    V: View + Unpin,
//{
//    fn from(s: &String) -> Self {
//        ViewBuilder::text(stream::iter(std::iter::once(s.clone())))
//    }
//}
//
//impl<V> From<String> for ViewBuilder
//where
//    V: View + Unpin,
//{
//    fn from(s: String) -> Self {
//        ViewBuilder::text(stream::iter(std::iter::once(s)))
//    }
//}
//
//impl<V> From<&str> for ViewBuilder
//where
//    V: View + Unpin,
//{
//    fn from(s: &str) -> Self {
//        ViewBuilder::text(stream::iter(std::iter::once(s.to_string())))
//    }
//}
//
impl<S, St> From<(S, St)> for ViewBuilder
where
    S: AsRef<str>,
    St: Stream<Item = String> + Unpin + Send + Sync + 'static,
{
    fn from((s, st): (S, St)) -> Self {
        let iter = stream::iter(std::iter::once(s.as_ref().to_string())).chain(st);
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