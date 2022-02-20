//! A low cost intermediate structure for creating views.
use crate::{channel::SinkError, constraints::{SendConstraints, Spawnable, SyncConstraints}, patch::{HashPatch, ListPatch}, view::{EventTargetType, View}};
use futures::{Sink, Stream, StreamExt};
use std::{
    pin::Pin,
    sync::Arc,
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
/// Useful for getting the starting values of a view.
pub fn exhaust<T, St>(mut stream: St) -> (St, Vec<T>)
where
    St: Stream<Item = T> + Unpin,
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

/// An enumeration of values that [`ViewBuilder`]s accept.
pub enum MogwaiValue<S, St> {
    /// A value now
    Now(S),
    /// A stream of values later
    Later(St),
    /// A value now and a stream of values later
    NowAndLater(S, St),
}

/// Marker trait with one convenience function for streams used in [`ViewBuilder`]s.
pub trait MogwaiStream<T>: Stream<Item = T> + SendConstraints + SyncConstraints {
    fn mogwai_stream(self) -> Pin<Box<dyn MogwaiStream<T>>>;
}
impl<S, T: Stream<Item = S> + SendConstraints + SyncConstraints> MogwaiStream<S> for T {
    fn mogwai_stream(self) -> Pin<Box<dyn MogwaiStream<S>>> {
        Box::pin(self)
    }
}

/// Marker trait for sinks used in [`ViewBuilder`]s.
pub trait MogwaiSink<T>:
    Sink<T, Error = SinkError> + SendConstraints + SyncConstraints
{
    fn mogwai_sink(self) -> Pin<Box<dyn MogwaiSink<T>>>;
}
impl<S, T> MogwaiSink<S> for T
where
    T: Sink<S, Error = SinkError> + SendConstraints + SyncConstraints,
{
    fn mogwai_sink(self) -> Pin<Box<dyn MogwaiSink<S>>> {
        Box::pin(self)
    }
}

impl From<bool> for MogwaiValue<bool, Pin<Box<dyn MogwaiStream<bool>>>> {
    fn from(b: bool) -> Self {
        MogwaiValue::Now(b)
    }
}

impl<'a> From<&'a str> for MogwaiValue<String, Pin<Box<dyn MogwaiStream<String>>>> {
    fn from(s: &'a str) -> Self {
        MogwaiValue::Now(s.to_string())
    }
}

impl From<&String> for MogwaiValue<String, Pin<Box<dyn MogwaiStream<String>>>> {
    fn from(s: &String) -> Self {
        MogwaiValue::Now(s.clone())
    }
}

impl From<String> for MogwaiValue<String, Pin<Box<dyn MogwaiStream<String>>>> {
    fn from(s: String) -> Self {
        MogwaiValue::Now(s)
    }
}

impl<S, St: MogwaiStream<S>> From<St> for MogwaiValue<S, St> {
    fn from(s: St) -> Self {
        MogwaiValue::Later(s)
    }
}

impl<S, St: Stream<Item = S>, X: Into<S>> From<(X, St)> for MogwaiValue<S, St> {
    fn from((x, s): (X, St)) -> Self {
        MogwaiValue::NowAndLater(x.into(), s)
    }
}

impl<S, St> From<MogwaiValue<S, St>> for Pin<Box<dyn MogwaiStream<S>>>
where
    S: SendConstraints + SyncConstraints,
    St: MogwaiStream<S>,
{
    fn from(v: MogwaiValue<S, St>) -> Self {
        match v {
            MogwaiValue::Now(s) => Box::pin(futures::stream::once(async move { s })),
            MogwaiValue::Later(s) => Box::pin(s),
            MogwaiValue::NowAndLater(s, st) => {
                Box::pin(futures::stream::once(async move { s }).chain(st))
            }
        }
    }
}

/// An enumeration of types that can be appended as children to [`ViewBuilder`].
pub enum AppendArg<T: View> {
    /// A single static child.
    Single(ViewBuilder<T>),
    /// A collection of static children.
    Iter(Vec<ViewBuilder<T>>),
}

impl<T, V> From<Vec<V>> for AppendArg<T>
where
    T: View,
    ViewBuilder<T>: From<V>,
{
    fn from(bldrs: Vec<V>) -> Self {
        AppendArg::Iter(bldrs.into_iter().map(ViewBuilder::from).collect())
    }
}

impl<T: View> From<&String> for ViewBuilder<T> {
    fn from(s: &String) -> Self {
        ViewBuilder::text(s.as_str())
    }
}

impl<T: View> From<String> for ViewBuilder<T> {
    fn from(s: String) -> Self {
        ViewBuilder::text(s.as_str())
    }
}

impl<T: View> From<&str> for ViewBuilder<T> {
    fn from(s: &str) -> Self {
        ViewBuilder::text(s)
    }
}

impl<T: View, S, St> From<(S, St)> for ViewBuilder<T>
where
    S: AsRef<str>,
    St: MogwaiStream<String>,
{
    fn from((s, st): (S, St)) -> Self {
        ViewBuilder::text(s.as_ref()).with_text_stream(st)
    }
}

impl<T: View, V: Into<ViewBuilder<T>>> From<V> for AppendArg<T> {
    fn from(v: V) -> Self {
        AppendArg::Single(v.into())
    }
}

/// Marker trait for operations that mutate a domain specific view.
pub trait PostBuild<T>: FnOnce(&mut T) + SendConstraints + SyncConstraints {}
impl<T, F: FnOnce(&mut T) + SendConstraints + SyncConstraints> PostBuild<T> for F {}

/// The starting identity of a view.
pub enum ViewIdentity {
    Branch(String),
    NamespacedBranch(String, String),
    Leaf(String),
}

/// An un-built mogwai view.
/// A ViewBuilder is the most generic view representation in the mogwai library.
/// It is the the blueprint of a view - everything needed to create, hydrate or serialize the view.
pub struct ViewBuilder<T: View> {
    /// The identity of the view.
    ///
    /// Either a name or a tuple of a name and a namespace.
    pub identity: ViewIdentity,
    /// Text declarations.
    pub texts: Vec<Pin<Box<dyn MogwaiStream<String>>>>,
    /// Attribute declarations.
    pub attribs: Vec<Pin<Box<dyn MogwaiStream<HashPatch<String, String>>>>>,
    /// Boolean attribute declarations.
    pub bool_attribs: Vec<Pin<Box<dyn MogwaiStream<HashPatch<String, bool>>>>>,
    /// Style declarations.
    pub styles: Vec<Pin<Box<dyn MogwaiStream<HashPatch<String, String>>>>>,
    /// Child patch declarations.
    pub children: Vec<Pin<Box<dyn MogwaiStream<ListPatch<ViewBuilder<T>>>>>>,
    /// Event sinks.
    pub events: Vec<(String, EventTargetType, Pin<Box<dyn MogwaiSink<T::Event>>>)>,
    /// Post build operations/computations that run and mutate the view after initialization.
    pub ops: Vec<Box<dyn PostBuild<T>>>,
    /// Sinks that want access to the view once it is initialized.
    pub view_sinks: Vec<Pin<Box<dyn MogwaiSink<T>>>>,
    /// Asynchronous tasks that run after the view has been initialized.
    pub tasks: Vec<Pin<Box<dyn Spawnable<()>>>>,
}

impl<T: View> ViewBuilder<T> {
    /// Create a new container element builder.
    pub fn element(tag: impl Into<String>) -> Self {
        ViewBuilder {
            identity: ViewIdentity::Branch(tag.into()),
            texts: vec![],
            attribs: vec![],
            bool_attribs: vec![],
            styles: vec![],
            ops: vec![],
            children: vec![],
            events: vec![],
            view_sinks: vec![],
            tasks: vec![],
        }
    }

    /// Create a new namespaced container element builder.
    pub fn element_ns(tag: impl Into<String>, ns: impl Into<String>) -> Self {
        ViewBuilder {
            identity: ViewIdentity::NamespacedBranch(tag.into(), ns.into()),
            texts: vec![],
            attribs: vec![],
            bool_attribs: vec![],
            styles: vec![],
            children: vec![],
            events: vec![],
            ops: vec![],
            view_sinks: vec![],
            tasks: vec![],
        }
    }

    /// Create a new node builder.
    pub fn text<St>(t: impl Into<MogwaiValue<String, St>>) -> Self
    where
        St: MogwaiStream<String>,
    {
        let mv = t.into();
        let (identity, texts) = match mv {
            MogwaiValue::Now(s) => (s, vec![]),
            MogwaiValue::Later(st) => (String::new(), vec![st.mogwai_stream()]),
            MogwaiValue::NowAndLater(s, st) => (s, vec![st.mogwai_stream()]),
        };
        ViewBuilder {
            identity: ViewIdentity::Leaf(identity),
            texts,
            attribs: vec![],
            bool_attribs: vec![],
            styles: vec![],
            ops: vec![],
            children: vec![],
            events: vec![],
            view_sinks: vec![],
            tasks: vec![],
        }
    }

    /// Adds an asynchronous task.
    pub fn with_task(mut self, t: impl Spawnable<()>) -> Self {
        self.tasks.push(Box::pin(t));
        self
    }

    /// Add a stream to set the text of this builder.
    pub fn with_text_stream<St>(mut self, t: impl Into<MogwaiValue<String, St>>) -> Self
    where
        St: MogwaiStream<String>,
    {
        let mv = t.into();
        let st: Pin<Box<_>> = mv.into();
        self.texts.push(st);
        self
    }

    /// Add a stream to patch the attributes of this builder.
    pub fn with_attrib_stream<St>(
        mut self,
        t: impl Into<MogwaiValue<HashPatch<String, String>, St>>,
    ) -> Self
    where
        St: MogwaiStream<HashPatch<String, String>>,
    {
        let mv = t.into();
        let st: Pin<Box<_>> = mv.into();
        self.attribs.push(st);
        self
    }

    /// Add a stream to patch a single attribute of this builder.
    pub fn with_single_attrib_stream<St>(
        mut self,
        k: impl Into<String>,
        t: impl Into<MogwaiValue<String, St>>,
    ) -> Self
    where
        St: MogwaiStream<String>,
    {
        let key = k.into();
        let mv = t.into();
        let st: Pin<Box<_>> = mv.into();
        let st = Box::pin(st.map(move |v| HashPatch::Insert(key.clone(), v)));
        self.attribs.push(st);
        self
    }

    /// Add a stream to patch the boolean attributes of this builder.
    pub fn with_bool_attrib_stream<St>(
        mut self,
        t: impl Into<MogwaiValue<HashPatch<String, bool>, St>>,
    ) -> Self
    where
        St: MogwaiStream<HashPatch<String, bool>>,
    {
        let mv = t.into();
        let st: Pin<Box<_>> = mv.into();
        self.bool_attribs.push(st);
        self
    }

    /// Add a stream to patch a single boolean attribute of this builder.
    pub fn with_single_bool_attrib_stream<St>(
        mut self,
        k: impl Into<String>,
        t: impl Into<MogwaiValue<bool, St>>,
    ) -> Self
    where
        St: MogwaiStream<bool>,
    {
        let key = k.into();
        let st = t.into();
        let st: Pin<Box<_>> = st.into();
        let st = Box::pin(st.map(move |b| HashPatch::Insert(key.clone(), b)));
        self.bool_attribs.push(st);
        self
    }

    /// Add a stream to patch the style attribute of this builder.
    pub fn with_style_stream<St>(mut self, t: impl Into<MogwaiValue<String, St>>) -> Self
    where
        St: MogwaiStream<String>,
    {
        let t = t.into();
        let st: Pin<Box<dyn MogwaiStream<String>>> = t.into();
        let st = Box::pin(st.flat_map(|v: String| {
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
        }));
        self.styles.push(st);
        self
    }

    /// Add a stream to patch a single style of this builder.
    pub fn with_single_style_stream<St>(
        mut self,
        k: impl Into<String>,
        t: impl Into<MogwaiValue<String, St>>,
    ) -> Self
    where
        St: MogwaiStream<String>,
    {
        let key = k.into();
        let mv = t.into();
        let st: Pin<Box<_>> = mv.into();
        let st = Box::pin(st.map(move |v| HashPatch::Insert(key.clone(), v)));
        self.styles.push(st);
        self
    }

    /// Add a stream to patch the list of children of this builder.
    pub fn with_child_stream<St>(
        mut self,
        t: impl Into<MogwaiValue<ListPatch<ViewBuilder<T>>, St>>,
    ) -> Self
    where
        St: MogwaiStream<ListPatch<ViewBuilder<T>>>,
    {
        let mv = t.into();
        let st: Pin<Box<_>> = mv.into();
        self.children.push(st);
        self
    }

    /// Append a child or iterator of children.
    pub fn append<A>(self, children: A) -> Self
    where
        AppendArg<T>: From<A>,
    {
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
    pub fn with_post_build<F>(mut self, run: F) -> Self
    where
        F: PostBuild<T>,
    {
        self.ops.push(Box::new(run));
        self
    }

    /// Send a clone of the inner view once it is built.
    pub fn with_capture_view(mut self, sink: impl MogwaiSink<T>) -> Self {
        self.view_sinks.push(Box::pin(sink));
        self
    }

    /// Add a sink into which view events of the given name will be sent.
    pub fn with_event(
        mut self,
        name: impl Into<String>,
        target: EventTargetType,
        tx: impl MogwaiSink<T::Event>,
    ) -> Self {
        self.events.push((name.into(), target, Box::pin(tx)));
        self
    }
}

impl<T: View, V> From<Option<V>> for AppendArg<T>
where
    ViewBuilder<T>: From<V>,
{
    fn from(may_vb: Option<V>) -> Self {
        AppendArg::Iter(
            may_vb
                .into_iter()
                .map(ViewBuilder::from)
                .collect::<Vec<_>>(),
        )
    }
}
