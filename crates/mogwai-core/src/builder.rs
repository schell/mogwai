//! A low cost intermediate structure for creating views.
use crate::{
    channel::SinkError,
    futures::stream,
    patch::{HashPatch, ListPatch},
    traits::{
        ConstrainedFuture, ConstrainedSink, ConstrainedStream, ConstraintType, NoConstraint,
        SendConstraint, SendSyncConstraint,
    },
    view::{EventTargetType, View},
};
use futures::{Future, FutureExt, Sink, SinkExt, Stream, StreamExt};
use std::{
    marker::PhantomData,
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

/// Try to get an available `T` from the given stream by polling it.
///
/// This proxies to [`futures::stream::StreamExt::poll_next_unpin`].
pub fn try_next<T, C: ConstraintType<StreamType<T> = St>, St: Stream<Item = T> + Unpin>(
    stream: &mut St,
) -> std::task::Poll<Option<T>> {
    let raw_waker = RawWaker::from(Arc::new(DummyWaker));
    let waker = unsafe { Waker::from_raw(raw_waker) };
    let mut cx = std::task::Context::from_waker(&waker);

    stream.poll_next_unpin(&mut cx)
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

/// Stream type.
///
/// ## TODO: write about constraints.
pub struct MogwaiStream<T, St: Stream<Item = T> + Unpin, C: Unpin> {
    inner: St,
    _phantom: PhantomData<C>,
}

impl<T, St: Stream<Item = T> + Unpin, C: Unpin> Stream for MogwaiStream<T, St, C> {
    type Item = T;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.inner.poll_next_unpin(cx)
    }
}

impl<T, St: Stream<Item = T> + Unpin + 'static> From<MogwaiStream<T, St, NoConstraint>>
    for Pin<Box<dyn Stream<Item = T> + Unpin + 'static>>
{
    fn from(value: MogwaiStream<T, St, NoConstraint>) -> Self {
        Box::pin(value.inner)
    }
}

impl<T, St: Stream<Item = T> + Unpin + Send + 'static> From<MogwaiStream<T, St, SendConstraint>>
    for Pin<Box<dyn Stream<Item = T> + Unpin + Send + 'static>>
{
    fn from(value: MogwaiStream<T, St, SendConstraint>) -> Self {
        Box::pin(value.inner)
    }
}

impl<T, St: Stream<Item = T> + Unpin + Send + Sync + 'static>
    From<MogwaiStream<T, St, SendSyncConstraint>>
    for Pin<Box<dyn Stream<Item = T> + Unpin + Send + Sync + 'static>>
{
    fn from(value: MogwaiStream<T, St, SendSyncConstraint>) -> Self {
        Box::pin(value.inner)
    }
}

impl<T, C: Unpin> MogwaiStream<T, stream::Iter<std::iter::Once<T>>, C> {
    pub fn from_value(t: T) -> Self {
        MogwaiStream {
            inner: futures::stream::iter(std::iter::once(t)),
            _phantom: PhantomData,
        }
    }
}

impl<T, St: Stream<Item = T> + Unpin, C: Unpin> MogwaiStream<T, St, C> {
    pub fn from_stream(st: St) -> Self {
        MogwaiStream {
            inner: st,
            _phantom: PhantomData,
        }
    }

    pub fn from_value_and_stream(
        t: T,
        st: St,
    ) -> MogwaiStream<T, stream::Chain<stream::Iter<std::iter::Once<T>>, St>, C> {
        MogwaiStream {
            inner: futures::stream::iter(std::iter::once(t)).chain(st),
            _phantom: PhantomData,
        }
    }
}

impl<C: Unpin> From<bool> for MogwaiStream<bool, stream::Iter<std::iter::Once<bool>>, C> {
    fn from(b: bool) -> Self {
        MogwaiStream::from_value(b)
    }
}

impl<'a, C: Unpin> From<&'a str>
    for MogwaiStream<String, stream::Iter<std::iter::Once<String>>, C>
{
    fn from(s: &'a str) -> Self {
        MogwaiStream::from_value(s.to_string())
    }
}

impl<Constraint: Unpin> From<&String>
    for MogwaiStream<String, stream::Iter<std::iter::Once<String>>, Constraint>
{
    fn from(s: &String) -> Self {
        MogwaiStream::from_value(s.clone())
    }
}

impl<Constraint: Unpin> From<String>
    for MogwaiStream<String, stream::Iter<std::iter::Once<String>>, Constraint>
{
    fn from(s: String) -> Self {
        MogwaiStream::from_value(s)
    }
}

impl<S, St: Stream<Item = S> + Send + Sync + Unpin + 'static> From<St>
    for MogwaiStream<S, St, SendSyncConstraint>
{
    fn from(s: St) -> Self {
        MogwaiStream::from_stream(s)
    }
}

impl<S, St: Stream<Item = S> + Send + Unpin + 'static> From<St>
    for MogwaiStream<S, St, SendConstraint>
{
    fn from(s: St) -> Self {
        MogwaiStream::from_stream(s)
    }
}

impl<S, St: Stream<Item = S> + Unpin + 'static> From<St> for MogwaiStream<S, St, NoConstraint> {
    fn from(s: St) -> Self {
        MogwaiStream::from_stream(s)
    }
}

impl<S, St: Stream<Item = S> + Send + Sync + Unpin + 'static, X: Into<S>> From<(X, St)>
    for MogwaiStream<S, stream::Chain<stream::Iter<std::iter::Once<S>>, St>, SendSyncConstraint>
{
    fn from((x, s): (X, St)) -> Self {
        MogwaiStream::from_value_and_stream(x.into(), s)
    }
}

impl<S, St: Stream<Item = S> + Send + Unpin + 'static, X: Into<S>> From<(X, St)>
    for MogwaiStream<S, stream::Chain<stream::Iter<std::iter::Once<S>>, St>, SendConstraint>
{
    fn from((x, s): (X, St)) -> Self {
        MogwaiStream::from_value_and_stream(x.into(), s)
    }
}

impl<S, St: Stream<Item = S> + Unpin + 'static, X: Into<S>> From<(X, St)>
    for MogwaiStream<S, stream::Chain<stream::Iter<std::iter::Once<S>>, St>, NoConstraint>
{
    fn from((x, s): (X, St)) -> Self {
        MogwaiStream::from_value_and_stream(x.into(), s)
    }
}

/// Mogwai's future type, constrained by a type parameter.
pub struct MogwaiFuture<T, Fut, C> {
    inner: Fut,
    _phantom: PhantomData<(T, C)>,
}

impl<T: Unpin, Fut: Future<Output = T> + Unpin, C: Unpin> Future for MogwaiFuture<T, Fut, C> {
    type Output = Fut::Output;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.get_mut().inner.poll_unpin(cx)
    }
}

impl<T, Fut: Future<Output = T> + 'static> From<Fut> for MogwaiFuture<T, Fut, NoConstraint> {
    fn from(inner: Fut) -> Self {
        MogwaiFuture {
            inner,
            _phantom: PhantomData,
        }
    }
}

impl<T, Fut: Future<Output = T> + Send + 'static> From<Fut>
    for MogwaiFuture<T, Fut, SendConstraint>
{
    fn from(inner: Fut) -> Self {
        MogwaiFuture {
            inner,
            _phantom: PhantomData,
        }
    }
}

impl<T, Fut: Future<Output = T> + Send + Sync + 'static> From<Fut>
    for MogwaiFuture<T, Fut, SendSyncConstraint>
{
    fn from(inner: Fut) -> Self {
        MogwaiFuture {
            inner,
            _phantom: PhantomData,
        }
    }
}

/// Sink type.
pub struct MogwaiSink<T, Si: Sink<T, Error = SinkError>, Constraint> {
    inner: Si,
    _phantom: PhantomData<(T, Constraint)>,
}

impl<T: Unpin, Si: Sink<T, Error = SinkError> + Unpin, C: Unpin> Sink<T> for MogwaiSink<T, Si, C> {
    type Error = SinkError;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.get_mut().inner.poll_ready_unpin(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        self.get_mut().inner.start_send_unpin(item)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.get_mut().inner.poll_flush_unpin(cx)
    }

    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.get_mut().inner.poll_close_unpin(cx)
    }
}

/// Marker trait for operations that mutate a domain specific view.
pub trait PostBuild<T>: FnOnce(&mut T) {}
impl<T, F: FnOnce(&mut T)> PostBuild<T> for F {}

/// The starting identity of a view.
pub enum ViewIdentity {
    Branch(String),
    NamespacedBranch(String, String),
    Leaf(String),
}

/// An un-built mogwai view.
/// A ViewBuilder is the most generic view representation in the mogwai library.
/// It is the the blueprint of a view - everything needed to create, hydrate or serialize the view.
pub struct ViewBuilder<T: View, C: ConstraintType> {
    /// The identity of the view.
    ///
    /// Either a name or a tuple of a name and a namespace.
    pub identity: ViewIdentity,
    /// Text declarations.
    pub texts: Vec<ConstrainedStream<String, C>>,
    /// Attribute declarations.
    pub attribs: Vec<ConstrainedStream<HashPatch<String, String>, C>>,
    /// Boolean attribute declarations.
    pub bool_attribs: Vec<ConstrainedStream<HashPatch<String, bool>, C>>,
    /// Style declarations.
    pub styles: Vec<ConstrainedStream<HashPatch<String, String>, C>>,
    /// Child patch declarations.
    pub children: Vec<ConstrainedStream<ListPatch<ViewBuilder<T, C>>, C>>,
    /// Event sinks.
    pub events: Vec<(String, EventTargetType, ConstrainedSink<T::Event, C>)>,
    /// Post build operations/computations that run and mutate the view after initialization.
    pub ops: Vec<Box<dyn PostBuild<T>>>,
    /// Sinks that want access to the view once it is initialized.
    pub view_sinks: Vec<ConstrainedSink<T, C>>,
    /// Asynchronous tasks that run after the view has been initialized.
    pub tasks: Vec<ConstrainedFuture<(), C>>,
}

impl<T: View, C: ConstraintType + Unpin> ViewBuilder<T, C> {
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
    pub fn text<St: Stream<Item = String> + Unpin>(
        t: impl Into<MogwaiStream<String, St, C>>,
    ) -> Self
    where
        MogwaiStream<String, St, C>: Into<ConstrainedStream<String, C>>,
    {
        let mv: MogwaiStream<_, _, _> = t.into();
        let pinbox: ConstrainedStream<String, C> = mv.into();
        let (pinbox, texts) = exhaust(pinbox);
        let identity = texts
            .into_iter()
            .fold(None, |_, text| Some(text))
            .unwrap_or_else(|| String::new());

        ViewBuilder {
            identity: ViewIdentity::Leaf(identity),
            texts: vec![pinbox],
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
    pub fn with_task<Fut: Future<Output = ()>>(
        mut self,
        f: impl Into<MogwaiFuture<(), Fut, C>>,
    ) -> Self
    where
        MogwaiFuture<(), Fut, C>: Into<ConstrainedFuture<(), C>>,
    {
        self.tasks.push(f.into().into());
        self
    }

    /// Add a stream to set the text of this builder.
    pub fn with_text_stream<St: Stream<Item = String> + Unpin>(
        mut self,
        st: impl Into<MogwaiStream<String, St, C>>,
    ) -> Self
    where
        MogwaiStream<String, St, C>: Into<ConstrainedStream<String, C>>,
    {
        self.texts.push(st.into().into());
        self
    }

    /// Add a stream to patch the attributes of this builder.
    pub fn with_attrib_stream<St: Stream<Item = HashPatch<String, String>> + Unpin>(
        mut self,
        st: impl Into<MogwaiStream<HashPatch<String, String>, St, C>>,
    ) -> Self
    where
        MogwaiStream<HashPatch<String, String>, St, C>:
            Into<ConstrainedStream<HashPatch<String, String>, C>>,
    {
        self.attribs.push(st.into().into());
        self
    }

    /// Add a stream to patch a single attribute of this builder.
    pub fn with_single_attrib_stream<St: Stream<Item = String> + Unpin>(
        mut self,
        k: impl Into<String>,
        st: impl Into<MogwaiStream<String, St, C>>,
    ) -> Self
    where
        ConstrainedStream<HashPatch<String, String>, C>: From<
            MogwaiStream<
                HashPatch<String, String>,
                stream::Map<
                    MogwaiStream<String, St, C>,
                    Box<dyn Fn(String) -> HashPatch<String, String>>,
                >,
                C,
            >,
        >,
    {
        let key = k.into();
        let st = st.into();
        let st: MogwaiStream<
            HashPatch<String, String>,
            stream::Map<
                MogwaiStream<String, St, C>,
                Box<dyn Fn(String) -> HashPatch<String, String>>,
            >,
            C,
        > = MogwaiStream::from_stream(st.map(Box::new(move |v| HashPatch::Insert(key.clone(), v))));
        self.attribs.push(st.into());
        self
    }

    /// Add a stream to patch the boolean attributes of this builder.
    pub fn with_bool_attrib_stream<St: Stream<Item = HashPatch<String, bool>> + Unpin>(
        mut self,
        st: impl Into<MogwaiStream<HashPatch<String, bool>, St, C>>,
    ) -> Self
    where
        MogwaiStream<HashPatch<String, bool>, St, C>:
            Into<ConstrainedStream<HashPatch<String, bool>, C>>,
    {
        self.bool_attribs.push(st.into().into());
        self
    }

    /// Add a stream to patch a single boolean attribute of this builder.
    pub fn with_single_bool_attrib_stream<St: Stream<Item = bool> + Unpin>(
        mut self,
        k: impl Into<String>,
        st: impl Into<MogwaiStream<bool, St, C>>,
    ) -> Self
    where
        ConstrainedStream<HashPatch<String, bool>, C>: From<
            MogwaiStream<
                HashPatch<String, bool>,
                stream::Map<
                    MogwaiStream<bool, St, C>,
                    Box<dyn Fn(bool) -> HashPatch<String, bool>>,
                >,
                C,
            >,
        >,
    {
        let key = k.into();
        let st = st.into();
        let st: MogwaiStream<
            HashPatch<String, bool>,
            stream::Map<MogwaiStream<bool, St, C>, Box<dyn Fn(bool) -> HashPatch<String, bool>>>,
            C,
        > = MogwaiStream::from_stream(st.map(Box::new(move |b| HashPatch::Insert(key.clone(), b))));
        self.bool_attribs.push(st.into());
        self
    }

    /// Add a stream to patch the style attribute of this builder.
    pub fn with_style_stream<St: Stream<Item = String> + Unpin>(
        mut self,
        st: impl Into<MogwaiStream<String, St, C>>,
    ) -> Self
    where
        ConstrainedStream<HashPatch<String, String>, C>: From<
            MogwaiStream<
                HashPatch<String, String>,
                stream::FlatMap<
                    MogwaiStream<String, St, C>,
                    stream::Iter<std::vec::IntoIter<HashPatch<String, String>>>,
                    fn(String) -> stream::Iter<std::vec::IntoIter<HashPatch<String, String>>>,
                >,
                C,
            >,
        >,
    {
        let st = st.into();
        let st: MogwaiStream<
            HashPatch<String, String>,
            stream::FlatMap<
                MogwaiStream<String, St, C>,
                stream::Iter<std::vec::IntoIter<HashPatch<String, String>>>,
                fn(String) -> stream::Iter<std::vec::IntoIter<HashPatch<String, String>>>,
            >,
            C,
        > = MogwaiStream::from_stream(st.flat_map(|v: String| {
            let kvs = str::split(&v, ';')
                .filter_map(|style| {
                    let (k, v) = style.split_once(':')?;
                    Some(HashPatch::Insert(
                        k.trim().to_string(),
                        v.trim().to_string(),
                    ))
                })
                .collect::<Vec<_>>();
            stream::iter(kvs)
        }));
        self.styles.push(st.into());
        self
    }

    /// Add a stream to patch a single style of this builder.
    pub fn with_single_style_stream<St: Stream<Item = String> + Unpin>(
        mut self,
        k: impl Into<String>,
        st: impl Into<MogwaiStream<String, St, C>>,
    ) -> Self
    where
        <C as ConstraintType>::StreamType<HashPatch<String, String>>: From<
            MogwaiStream<
                HashPatch<String, String>,
                futures::stream::Map<
                    MogwaiStream<String, St, C>,
                    Box<dyn Fn(String) -> HashPatch<String, String>>,
                >,
                C,
            >,
        >,
    {
        let key = k.into();
        let st = st.into();
        let st =
            MogwaiStream::from_stream(st.map(Box::new(move |v| HashPatch::Insert(key.clone(), v))
                as Box<dyn Fn(String) -> HashPatch<String, String>>));
        self.styles.push(st.into());
        self
    }

    /// Add a stream to patch the list of children of this builder.
    pub fn with_child_stream<St: Stream<Item = ListPatch<ViewBuilder<T, C>>> + Unpin>(
        mut self,
        st: impl Into<MogwaiStream<ListPatch<ViewBuilder<T, C>>, St, C>>,
    ) -> Self
    where
        ConstrainedStream<ListPatch<ViewBuilder<T, C>>, C>:
            From<MogwaiStream<ListPatch<ViewBuilder<T, C>>, St, C>>,
    {
        let st = st.into();
        self.children.push(st.into());
        self
    }

    //    /// Append a child or iterator of children.
    //    pub fn append<A>(self, children: A) -> Self
    //    where
    //        AppendArg<T>: From<A>,
    //    {
    //        let arg = children.into();
    //
    //        let bldrs = match arg {
    //            AppendArg::Single(bldr) => vec![bldr],
    //            AppendArg::Iter(bldrs) => bldrs,
    //        };
    //        let stream = Box::pin(futures::stream::iter(
    //            bldrs.into_iter().map(|b| ListPatch::push(b)),
    //        ));
    //        self.with_child_stream(stream)
    //    }

    /// Add an operation to perform after the view has been built.
    pub fn with_post_build<F>(mut self, run: F) -> Self
    where
        F: PostBuild<T> + 'static,
    {
        self.ops.push(Box::new(run));
        self
    }

    /// Send a clone of the inner view once it is built.
    pub fn with_capture_view<Si: Sink<T, Error = SinkError>>(
        mut self,
        sink: impl Into<MogwaiSink<T, Si, C>>,
    ) -> Self
    where
        ConstrainedSink<T, C>: From<MogwaiSink<T, Si, C>>,
    {
        self.view_sinks.push(sink.into().into());
        self
    }

    /// Add a sink into which view events of the given name will be sent.
    pub fn with_event<Si: Sink<T::Event, Error = SinkError>>(
        mut self,
        name: impl Into<String>,
        target: EventTargetType,
        si: impl Into<MogwaiSink<T::Event, Si, C>>,
    ) -> Self
    where
        ConstrainedSink<<T as View>::Event, C>: From<MogwaiSink<<T as View>::Event, Si, C>>,
    {
        self.events.push((name.into(), target, si.into().into()));
        self
    }
}

/// An enumeration of types that can be appended as children to [`ViewBuilder`].
pub enum AppendArg<T: View, C: ConstraintType> {
    /// A single static child.
    Single(ViewBuilder<T, C>),
    /// A collection of static children.
    Iter(Vec<ViewBuilder<T, C>>),
}

impl<T, V, C> From<Vec<V>> for AppendArg<T, C>
where
    T: View,
    C: ConstraintType,
    ViewBuilder<T, C>: From<V>,
{
    fn from(bldrs: Vec<V>) -> Self {
        AppendArg::Iter(bldrs.into_iter().map(ViewBuilder::from).collect())
    }
}

impl<T, C> From<&String> for ViewBuilder<T, C>
where
    ConstrainedStream<String, C>:
        From<MogwaiStream<String, stream::Iter<std::iter::Once<String>>, C>>,
    T: View,
    C: ConstraintType + Unpin,
{
    fn from(s: &String) -> Self {
        ViewBuilder::text(s.as_str())
    }
}

impl<T, C> From<String> for ViewBuilder<T, C>
where
    ConstrainedStream<String, C>:
        From<MogwaiStream<String, stream::Iter<std::iter::Once<String>>, C>>,
    T: View,
    C: ConstraintType + Unpin,
{
    fn from(s: String) -> Self {
        ViewBuilder::text(s.as_str())
    }
}

impl<T, C> From<&str> for ViewBuilder<T, C>
where
    ConstrainedStream<String, C>:
        From<MogwaiStream<String, stream::Iter<std::iter::Once<String>>, C>>,
    T: View,
    C: ConstraintType + Unpin,
{
    fn from(s: &str) -> Self {
        ViewBuilder::text(s)
    }
}

impl<T: View, S, St, C: ConstraintType + Unpin> From<(S, St)> for ViewBuilder<T, C>
where
    ConstrainedStream<String, C>:
        From<MogwaiStream<String, stream::Iter<std::iter::Once<String>>, C>>,
    ConstrainedStream<String, C>: From<MogwaiStream<String, St, C>>,
    S: AsRef<str>,
    St: Stream<Item = String> + Unpin + 'static,
    MogwaiStream<String, St, C>: From<St>,
{
    fn from((s, st): (S, St)) -> Self {
        ViewBuilder::text(s.as_ref()).with_text_stream(st)
    }
}

impl<T: View, C: ConstraintType, V: Into<ViewBuilder<T, C>>> From<V> for AppendArg<T, C> {
    fn from(v: V) -> Self {
        AppendArg::Single(v.into())
    }
}

impl<T: View, C: ConstraintType, V> From<Option<V>> for AppendArg<T, C>
where
    ViewBuilder<T, C>: From<V>,
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
