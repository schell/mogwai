//! A low cost intermediate structure for creating views.
use crate::{
    patch::{HashPatch, ListPatch},
    view::View,
};
use async_channel::Sender;
use futures::{Stream, StreamExt};
use std::{convert::TryFrom, marker::PhantomData, ops::RangeBounds, pin::Pin};
use wasm_bindgen::JsCast;

/// Inner text stream.
pub type TextStream = Pin<Box<dyn Stream<Item = String> + 'static>>;

/// HashPatch updates for String attributes.
pub type AttribStream = Pin<Box<dyn Stream<Item = HashPatch<String, String>> + 'static>>;

/// HashPatch updates for boolean attributes.
pub type BooleanAttribStream = Pin<Box<dyn Stream<Item = HashPatch<String, bool>> + 'static>>;

/// HashPatch updates for style key value pairs.
pub type StyleStream = Pin<Box<dyn Stream<Item = HashPatch<String, String>> + 'static>>;

/// An event target declaration.
#[derive(Clone)]
pub enum EventTargetType {
    /// This target is the view it is declared on.
    Myself,
    /// This target is the window.
    Window,
    /// This target is the document.
    Document,
}

/// An output event declaration.
#[derive(Clone)]
pub struct EventCmd<Event> {
    /// The target of the event.
    /// In other words this is the target that a listener will be placed on.
    pub type_is: EventTargetType,
    /// The event name.
    pub name: String,
    /// The [`Sender`] that the event should be sent on.
    pub transmitter: Sender<Event>,
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

impl<T, Child, Event> ViewBuilder<T, Child, Event> {
    pub fn element(tag: &str) -> Self {
        ViewBuilder {
            _type_is: PhantomData,
            construct_with: tag.to_string(),
            ns: None,
            texts: vec![],
            attribs: vec![],
            bool_attribs: vec![],
            styles: vec![],
            events: vec![],
            patches: vec![],
        }
    }

    pub fn text(s: impl Stream<Item = String> + 'static) -> Self {
        ViewBuilder::element("").with_text_stream(s)
    }

    pub fn with_namespace(mut self, ns: &str) -> Self {
        self.ns = Some(ns.to_string());
        self
    }

    pub fn with_text_stream(mut self, s: impl Stream<Item = String> + 'static) -> Self {
        self.texts.push(s.boxed_local());
        self
    }

    pub fn with_attrib_stream(
        mut self,
        s: impl Stream<Item = HashPatch<String, String>> + 'static,
    ) -> Self {
        self.attribs.push(s.boxed_local());
        self
    }

    pub fn with_bool_attrib_stream(
        mut self,
        s: impl Stream<Item = HashPatch<String, bool>> + 'static,
    ) -> Self {
        self.bool_attribs.push(s.boxed_local());
        self
    }

    pub fn with_style_stream(
        mut self,
        s: impl Stream<Item = HashPatch<String, String>> + 'static,
    ) -> Self {
        self.styles.push(s.boxed_local());
        self
    }

    pub fn with_child_stream(
        mut self,
        s: impl Stream<Item = ListPatch<ViewBuilder<Child, Child, Event>>> + 'static,
    ) -> Self {
        self.patches.push(s.boxed_local());
        self
    }

    pub fn with_event(mut self, name: &str, tx: Sender<Event>) -> Self {
        self.events.push(EventCmd {
            type_is: EventTargetType::Myself,
            name: name.into(),
            transmitter: tx,
        });
        self
    }

    pub fn with_window_event(mut self, name: &str, tx: Sender<Event>) -> Self {
        self.events.push(EventCmd {
            type_is: EventTargetType::Window,
            name: name.into(),
            transmitter: tx,
        });
        self
    }

    pub fn with_document_event(mut self, name: &str, tx: Sender<Event>) -> Self {
        self.events.push(EventCmd {
            type_is: EventTargetType::Document,
            name: name.into(),
            transmitter: tx,
        });
        self
    }

    pub fn with_type<V>(self) -> ViewBuilder<V, Child, Event> {
        let ViewBuilder {
            _type_is: _,
            construct_with,
            ns,
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
            texts,
            attribs,
            bool_attribs,
            styles,
            patches,
            events,
        }
    }
}

fn stream_set<T, A, F>(t: &T, s: impl Stream<Item = A> + 'static, mut f: F)
where
    T: Clone + 'static,
    A: 'static,
    F: FnMut(&T, A) + 'static,
{
    let t = t.clone();
    let mut s = s.boxed_local();
    wasm_bindgen_futures::spawn_local(async move {
        loop {
            match s.next().await {
                Some(a) => {
                    f(&t, a);
                }
                None => break,
            }
        }
    })
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
            attribs,
            bool_attribs,
            styles,
            events,
            patches,
            texts,
        } = builder;

        let el: T = if !texts.is_empty() {
            let node = web_sys::Text::new().unwrap();
            let stream = futures::stream::select_all(texts);
            stream_set(&node, stream, |node, text| node.set_data(&text));
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
            let stream = futures::stream::select_all(attribs);
            stream_set(
                el.dyn_ref::<web_sys::HtmlElement>()
                    .ok_or_else(|| "could not cast to HtmlElement".to_string())?,
                stream,
                |view, patch| match patch {
                    crate::patch::HashPatch::Insert(k, v) => {
                        view.set_attribute(&k, &v).unwrap();
                    }
                    crate::patch::HashPatch::Remove(k) => {
                        view.remove_attribute(&k).unwrap();
                    }
                },
            );
        }

        if !bool_attribs.is_empty() {
            let bool_attrib_stream = futures::stream::select_all(bool_attribs);
            stream_set(
                el.dyn_ref::<web_sys::HtmlElement>()
                    .ok_or_else(|| "could not cast to HtmlElement".to_string())?,
                bool_attrib_stream,
                |view, patch| match patch {
                    crate::patch::HashPatch::Insert(k, v) => {
                        if v {
                            view.set_attribute(&k, "").unwrap();
                        } else {
                            view.remove_attribute(&k).unwrap();
                        }
                    }
                    crate::patch::HashPatch::Remove(k) => {
                        view.remove_attribute(&k).unwrap();
                    }
                },
            );
        }

        if !styles.is_empty() {
            let stream = futures::stream::select_all(styles);

            stream_set(
                &el.dyn_ref::<web_sys::HtmlElement>()
                    .ok_or_else(|| "could not cast to HtmlElement".to_string())?
                    .style(),
                stream,
                move |style, patch| match patch {
                    crate::patch::HashPatch::Insert(k, v) => {
                        style.set_property(&k, &v).unwrap();
                    }
                    crate::patch::HashPatch::Remove(k) => {
                        style.remove_property(&k).unwrap();
                    }
                },
            );
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
                        crate::event::add_event(&name, target, transmitter, |e| e);
                    }
                    EventTargetType::Window => {
                        crate::event::add_event(
                            &name,
                            &web_sys::window().unwrap(),
                            transmitter,
                            |e| e,
                        );
                    }
                    EventTargetType::Document => {
                        crate::event::add_event(
                            &name,
                            &web_sys::window().unwrap().document().unwrap(),
                            transmitter,
                            |e| e,
                        );
                    }
                }
            }
        }

        if !patches.is_empty() {
            let stream = futures::stream::select_all(patches);

            stream_set(
                el.dyn_ref::<web_sys::Node>()
                    .ok_or_else(|| "could not cast to Node".to_string())?,
                stream,
                |view, patch| match patch {
                    crate::patch::ListPatch::Splice {
                        range,
                        mut replace_with,
                    } => {
                        let list: web_sys::NodeList = view.child_nodes();
                        for i in 0..list.length() {
                            if range.contains(&(i as usize)) {
                                if let Some(old) = list.get(i) {
                                    let may_replacement = if replace_with.is_empty() {
                                        None
                                    } else {
                                        Some(replace_with.remove(0))
                                    };
                                    if let Some(new_builder) = may_replacement {
                                        let new: View<web_sys::Node> =
                                            View::try_from(new_builder).unwrap();
                                        view.replace_child(&new.inner, &old).unwrap();
                                    } else {
                                        let _ = view.remove_child(&old).unwrap();
                                    }
                                }
                            }
                        }
                    }
                    crate::patch::ListPatch::Push(new_builder) => {
                        let new = View::try_from(new_builder).unwrap();
                        let _ = view.append_child(&new.inner).unwrap();
                    }
                    crate::patch::ListPatch::Pop => {
                        if let Some(child) = view.last_child() {
                            let _ = view.remove_child(&child).unwrap();
                        }
                    }
                },
            );
        }

        Ok(View { inner: el })
    }
}
