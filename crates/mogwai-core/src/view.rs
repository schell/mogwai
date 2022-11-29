//! Trait supporting domain specific views.
use std::{future::Future, pin::Pin};

use futures::{Sink, SinkExt, Stream, StreamExt};

pub use anyhow::Error;

use crate::{
    builder::{exhaust, ViewBuilder, ViewIdentity},
    channel::SinkError,
    patch::{HashPatch, ListPatch},
};

pub type FuturePinBox<T> = Pin<Box<dyn Future<Output = T> + Unpin + 'static>>;
pub type FuturePinBoxSend<T> = Pin<Box<dyn Future<Output = T> + Unpin + Send + 'static>>;
pub type FuturePinBoxSendSync<T> = Pin<Box<dyn Future<Output = T> + Unpin + Send + Sync + 'static>>;

pub type StreamPinBox<T> = Pin<Box<dyn Stream<Item = T> + Unpin + 'static>>;
pub type StreamPinBoxSend<T> = Pin<Box<dyn Stream<Item = T> + Unpin + Send + 'static>>;
pub type StreamPinBoxSendSync<T> = Pin<Box<dyn Stream<Item = T> + Unpin + Send + Sync + 'static>>;

pub type SinkPinBox<T> = Pin<Box<dyn Sink<T, Error = SinkError> + Unpin + 'static>>;
pub type SinkPinBoxSend<T> = Pin<Box<dyn Sink<T, Error = SinkError> + Unpin + Send + 'static>>;
pub type SinkPinBoxSendSync<T> =
    Pin<Box<dyn Sink<T, Error = SinkError> + Unpin + Send + Sync + 'static>>;

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

pub trait ViewResources<V>
where
    V: View<Resources = Self>,
    V::Child: View<Resources = Self>,
{
    /// Initialize a new view.
    fn init(&self, identity: ViewIdentity) -> anyhow::Result<V>;

    /// Convert a view builder into a view.
    fn build(&self, builder: ViewBuilder<V>) -> anyhow::Result<V> {
        let ViewBuilder {
            identity,
            texts,
            attribs,
            bool_attribs,
            styles,
            ops,
            children,
            events,
            view_sinks,
            tasks,
        } = builder;

        let mut element = self.init(identity)?;

        let (text_stream, texts) = exhaust(futures::stream::select_all(texts));
        let (attrib_stream, attribs) = exhaust(futures::stream::select_all(attribs));
        let (bool_attrib_stream, bool_attribs) = exhaust(futures::stream::select_all(bool_attribs));
        let (style_stream, styles) = exhaust(futures::stream::select_all(styles));
        let (child_stream, children) = exhaust(futures::stream::select_all(children));

        element.set_initial_values(
            self,
            texts.into_iter(),
            attribs.into_iter(),
            bool_attribs.into_iter(),
            styles.into_iter(),
            children.into_iter(),
        )?;

        element.set_streaming_values(
            text_stream,
            attrib_stream,
            bool_attrib_stream,
            style_stream,
            child_stream,
        )?;

        for (event_name, event_target, event_sink) in events.into_iter() {
            element.set_event(event_target, &event_name, event_sink);
        }

        for op in ops.into_iter() {
            (op)(&mut element);
        }

        for mut sink in view_sinks.into_iter() {
            let view = element.clone();
            element.spawn(async move {
                // Try to send the dom but don't panic because
                // the recv may have been dropped already, and that's ok.
                let _ = sink.send(view).await;
            });
        }

        for task in tasks.into_iter() {
            element.spawn(task);
        }

        Ok(element)
    }
}

/// An interface for a domain-specific view.
///
/// A view should be a type that can be cheaply cloned, where clones all refer
/// to the same underlying user interface node.
pub trait View
where
    Self: Sized + Clone + Unpin + Send + Sync + 'static,
{
    /// The type of events supported by this view.
    type Event;

    /// The type of child views that can be nested inside this view.
    type Child: View<Resources = Self::Resources>;

    /// The type of asyncronous task this view can spawn.
    type Task<T>;

    /// The type that holds domain specific resources used to
    /// construct views.
    type Resources: ViewResources<Self> + ViewResources<Self::Child>;

    /// Possibly asynchronous and scoped acquisition of resources.
    ///
    /// Used to build children before patching.
    fn with_acquired_resources<T: Send + Sync + 'static>(
        &self,
        f: impl FnOnce(Self::Resources) -> anyhow::Result<T>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<T>> + Send + Sync + 'static>>;

    /// Set the text of this view.
    fn set_text(&self, s: &str) -> anyhow::Result<()>;

    /// Patch the attributes of this view.
    fn patch_attribs(&self, patch: HashPatch<String, String>) -> anyhow::Result<()>;

    /// Patch the boolean attributes of this view.
    fn patch_bool_attribs(&self, patch: HashPatch<String, bool>) -> anyhow::Result<()>;

    /// Patch the style attributes of this view.
    fn patch_styles(&self, patch: HashPatch<String, String>) -> anyhow::Result<()>;

    /// Patch the nested children of this view.
    ///
    /// Returns a vector of the children removed.
    fn patch_children(&self, patch: ListPatch<Self::Child>) -> anyhow::Result<Vec<Self::Child>>;

    /// Builds and patches nested child views.
    ///
    /// Returns a vector of the children removed.
    fn build_and_patch_children(
        &self,
        resources: &Self::Resources,
        patch: ListPatch<ViewBuilder<Self::Child>>,
    ) -> anyhow::Result<Vec<Self::Child>> {
        let patch: ListPatch<Self::Child> =
            patch.try_map::<_, Self::Child, Error>(|builder: ViewBuilder<Self::Child>| {
                let child: Self::Child = resources.build(builder)?;
                Ok(child)
            })?;
        self.patch_children(patch)
    }

    /// Add an event to the element, document or window.
    ///
    /// When an event occurs it will be sent into the given sink.
    fn set_event(
        &self,
        type_is: EventTargetType,
        name: &str,
        sink: impl Sink<Self::Event, Error = SinkError>,
    );

    /// Set all the initial values of a view.
    fn set_initial_values(
        &self,
        resources: &Self::Resources,
        texts: impl Iterator<Item = String>,
        attribs: impl Iterator<Item = HashPatch<String, String>>,
        bool_attribs: impl Iterator<Item = HashPatch<String, bool>>,
        styles: impl Iterator<Item = HashPatch<String, String>>,
        children: impl Iterator<Item = ListPatch<ViewBuilder<Self::Child>>>,
    ) -> anyhow::Result<()> {
        for text in texts {
            self.set_text(&text)?;
        }

        for patch in attribs {
            self.patch_attribs(patch)?;
        }

        for patch in bool_attribs {
            self.patch_bool_attribs(patch)?;
        }

        for patch in styles {
            self.patch_styles(patch)?;
        }

        for patch in children {
            self.build_and_patch_children(resources, patch)?;
        }

        Ok(())
    }

    ///// Spawn an asynchronous task.
    fn spawn<T: Send + Sync + 'static>(&self, action: impl Future<Output = T> + Send + 'static) -> Self::Task<T>;

    /// Set all the streaming values of a view, spawning async task loops that update
    /// values as items come down the streams.
    ///
    /// Returns each async task loop in a vector.
    fn set_streaming_values(
        &self,
        mut text_stream: impl Stream<Item = String> + Send + Sync + Unpin + 'static,
        mut attrib_stream: impl Stream<Item = HashPatch<String, String>> + Send + Sync + Unpin + 'static,
        mut bool_attrib_stream: impl Stream<Item = HashPatch<String, bool>> + Send + Sync + Unpin + 'static,
        mut style_stream: impl Stream<Item = HashPatch<String, String>> + Send + Unpin + Sync + 'static,
        mut child_stream: impl Stream<Item = ListPatch<ViewBuilder<Self::Child>>> + Send + Unpin + Sync + 'static,
    ) -> anyhow::Result<Vec<Self::Task<anyhow::Result<()>>>> {
        let text_node = self.clone();
        let text_task = self.spawn(async move {
            while let Some(msg) = text_stream.next().await {
                text_node.set_text(&msg)?;
            }
            Ok(())
        });

        let attrib_node = self.clone();
        let attrib_task = self.spawn(async move {
            while let Some(patch) = attrib_stream.next().await {
                attrib_node.patch_attribs(patch)?;
            }
            Ok(())
        });

        let bool_attrib_node = self.clone();
        let bool_attrib_task = self.spawn(async move {
            while let Some(patch) = bool_attrib_stream.next().await {
                bool_attrib_node.patch_bool_attribs(patch)?;
            }
            Ok(())
        });

        let style_node = self.clone();
        let style_task = self.spawn(async move {
            while let Some(patch) = style_stream.next().await {
                style_node.patch_styles(patch)?;
            }
            Ok(())
        });

        let parent_node = self.clone();
        let parent_task = self.spawn(async move {
            while let Some(patch) = child_stream.next().await {
                parent_node.with_acquired_resources(|resources| {
                    parent_node.build_and_patch_children(&resources, patch)
                }).await?;
            }
            anyhow::Ok(())
        });

        Ok(vec![
            text_task,
            attrib_task,
            bool_attrib_task,
            style_task,
            parent_task,
        ])
    }
}
