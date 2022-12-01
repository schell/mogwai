//! A low cost intermediate structure for creating views.
use crate::{
    channel::SinkError,
    patch::{HashPatch, ListPatch},
    view::{EventTargetType, View},
};
use futures::{stream, Future, Sink, Stream, StreamExt};
use std::{
    pin::Pin,
    sync::Arc,
    task::{RawWaker, Wake, Waker},
};

struct DummyWaker;

impl Wake for DummyWaker {
    fn wake(self: std::sync::Arc<Self>) {}
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

/// Marker trait for operations that mutate a domain specific view.
pub trait PostBuild<T>: FnOnce(&mut T) + Send + Sync + 'static {}
impl<T, F: FnOnce(&mut T) + Send + Sync + 'static> PostBuild<T> for F {}

/// The starting identity of a view.
pub enum ViewIdentity {
    Branch(String),
    NamespacedBranch(String, String),
    Leaf(String),
}

pub type MogwaiFuture<T> = Pin<Box<dyn Future<Output = T> + Send + Sync + 'static>>;

pub type MogwaiStream<T> = Pin<Box<dyn Stream<Item = T> + Unpin + Send + Sync + 'static>>;

pub type MogwaiSink<T> = Pin<Box<dyn Sink<T, Error = SinkError> + Unpin + Send + Sync + 'static>>;

/// An un-built mogwai view.
/// A ViewBuilder is the most generic view representation in the mogwai library.
/// It is the the blueprint of a view - everything needed to create, hydrate or serialize the view.
pub struct ViewBuilder<V: View> {
    /// The identity of the view.
    ///
    /// Either a name or a tuple of a name and a namespace.
    pub identity: ViewIdentity,
    /// Text declarations.
    pub texts: Vec<MogwaiStream<String>>,
    /// Attribute declarations.
    pub attribs: Vec<MogwaiStream<HashPatch<String, String>>>,
    /// Boolean attribute declarations.
    pub bool_attribs: Vec<MogwaiStream<HashPatch<String, bool>>>,
    /// Style declarations.
    pub styles: Vec<MogwaiStream<HashPatch<String, String>>>,
    /// Child patch declarations.
    pub children: Vec<MogwaiStream<ListPatch<ViewBuilder<V::Child>>>>,
    /// Event sinks.
    pub events: Vec<(String, EventTargetType, MogwaiSink<V::Event>)>,
    /// Post build operations/computations that run and mutate the view after initialization.
    pub ops: Vec<Box<dyn PostBuild<V>>>,
    /// Sinks that want access to the view once it is initialized.
    pub view_sinks: Vec<MogwaiSink<V>>,
    /// Asynchronous tasks that run after the view has been initialized.
    pub tasks: Vec<MogwaiFuture<()>>,
}

impl<V: View> ViewBuilder<V> {
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
    pub fn text<St: Stream<Item = String> + Unpin + Send + Sync + 'static>(
        st: impl Into<MogwaiValue<String, St>>,
    ) -> Self {
        let (st, texts) = exhaust(PinBoxStream::from(st.into()));
        let identity = texts
            .into_iter()
            .fold(None, |_, text| Some(text))
            .unwrap_or_else(|| String::new());

        ViewBuilder {
            identity: ViewIdentity::Leaf(identity),
            texts: vec![st],
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
    pub fn with_task(
        mut self,
        f: impl Future<Output = ()> + Send + Sync + 'static,
    ) -> Self {
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
        self.texts.push(st);
        self
    }

    /// Add a stream to patch the attributes of this builder.
    pub fn with_attrib_stream<
        St: Stream<Item = HashPatch<String, String>> + Unpin + Send + Sync + 'static,
    >(
        mut self,
        st: impl Into<MogwaiValue<HashPatch<String, String>, St>>,
    ) -> Self {
        self.attribs.push(PinBoxStream::from(st.into()));
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
        self.attribs.push(Box::pin(st));
        self
    }

    /// Add a stream to patch the boolean attributes of this builder.
    pub fn with_bool_attrib_stream<
        St: Stream<Item = HashPatch<String, bool>> + Unpin + Send + Sync + 'static,
    >(
        mut self,
        st: impl Into<MogwaiValue<HashPatch<String, bool>, St>>,
    ) -> Self {
        self.bool_attribs.push(PinBoxStream::from(st.into()));
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
        let st = PinBoxStream::from(st.into()).map(move |b| HashPatch::Insert(key.clone(), b));
        self.bool_attribs.push(Box::pin(st));
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
                    Some(HashPatch::Insert(
                        k.trim().to_string(),
                        v.trim().to_string(),
                    ))
                })
                .collect::<Vec<_>>();
            stream::iter(kvs)
        });
        self.styles.push(Box::pin(st));
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
        let st = st.map(move |v| HashPatch::Insert(key.clone(), v));
        self.styles.push(Box::pin(st));
        self
    }

    /// Add a stream to patch the list of children of this builder.
    pub fn with_child_stream<
        St: Stream<Item = ListPatch<ViewBuilder<V::Child>>> + Unpin + Send + Sync + 'static,
    >(
        mut self,
        st: impl Into<MogwaiValue<ListPatch<ViewBuilder<V::Child>>, St>>,
    ) -> Self {
        self.children.push(PinBoxStream::from(st.into()));
        self
    }

    /// Append a child or iterator of children.
    pub fn append(self, children: impl Into<AppendArg<V::Child>>) -> Self {
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
        F: PostBuild<V>,
    {
        self.ops.push(Box::new(run));
        self
    }

    /// Send a clone of the inner view once it is built.
    pub fn with_capture_view(
        mut self,
        sink: impl Sink<V, Error = SinkError> + Unpin + Send + Sync + 'static,
    ) -> Self {
        self.view_sinks.push(Box::pin(sink));
        self
    }

    /// Add a sink into which view events of the given name will be sent.
    pub fn with_event(
        mut self,
        name: impl Into<String>,
        target: EventTargetType,
        si: impl Sink<V::Event, Error = SinkError> + Unpin + Send + Sync + 'static,
    ) -> Self {
        self.events.push((name.into(), target, Box::pin(si)));
        self
    }
}

/// An enumeration of types that can be appended as children to [`ViewBuilder`].
pub enum AppendArg<V: View> {
    /// A single static child.
    Single(ViewBuilder<V>),
    /// A collection of static children.
    Iter(Vec<ViewBuilder<V>>),
}

impl<T, V> From<Vec<T>> for AppendArg<V>
where
    V: View,
    ViewBuilder<V>: From<T>,
{
    fn from(bldrs: Vec<T>) -> Self {
        AppendArg::Iter(bldrs.into_iter().map(ViewBuilder::from).collect())
    }
}

//impl<V> From<&String> for ViewBuilder<V>
//where
//    V: View + Unpin,
//{
//    fn from(s: &String) -> Self {
//        ViewBuilder::text(stream::iter(std::iter::once(s.clone())))
//    }
//}
//
//impl<V> From<String> for ViewBuilder<V>
//where
//    V: View + Unpin,
//{
//    fn from(s: String) -> Self {
//        ViewBuilder::text(stream::iter(std::iter::once(s)))
//    }
//}
//
//impl<V> From<&str> for ViewBuilder<V>
//where
//    V: View + Unpin,
//{
//    fn from(s: &str) -> Self {
//        ViewBuilder::text(stream::iter(std::iter::once(s.to_string())))
//    }
//}
//
impl<S, St, V: View + Unpin> From<(S, St)> for ViewBuilder<V>
where
    S: AsRef<str>,
    St: Stream<Item = String> + Unpin + Send + Sync + 'static,
{
    fn from((s, st): (S, St)) -> Self {
        let iter = stream::iter(std::iter::once(s.as_ref().to_string())).chain(st);
        ViewBuilder::text(iter)
    }
}

impl<T: Into<ViewBuilder<V>>, V: View> From<T> for AppendArg<V> {
    fn from(t: T) -> Self {
        AppendArg::Single(t.into())
    }
}

impl<T, V> From<Option<T>> for AppendArg<V>
where
    V: View,
    ViewBuilder<V>: From<T>,
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
