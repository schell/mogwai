//! Types and [`TryFrom`] instances that can 're-animate' views or portions of views from the DOM.
use mogwai::{
    prelude::{Component, Effect, Gizmo, IsDomNode, Receiver, Transmitter, View},
    utils,
    view::{builder::*, interface::*},
};
use snafu::{OptionExt, Snafu};
pub use std::{convert::TryFrom, ops::Deref};
pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
pub use web_sys::{Element, Event, EventTarget, HtmlElement, HtmlInputElement};
use web_sys::{Node, Text};


#[snafu(visibility = "pub(crate)")]
#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display(
        "Missing any hydration option for node '{}' - must be the child of a node or have an id",
        tag
    ))]
    NoHydrationOption { tag: String },

    #[snafu(display("Could not find an element with id '{}'", id))]
    MissingId { id: String },

    #[snafu(display("Child at index {} could not be found in node '{}' containing '{:?}'", index, node.node_name(), node.node_value()))]
    MissingChild { node: Node, index: u32 },

    #[snafu(display("Could not convert from '{}' to '{}' for value: {:#?}", from, to, node))]
    Conversion {
        from: String,
        to: String,
        node: JsValue,
    },

    #[snafu(display("View cannot be hydrated"))]
    ViewOnly,
}


pub enum HydrationKey {
    Id(String),
    IndexedChildOf { node: Node, index: u32 },
}


impl HydrationKey {
    pub fn hydrate<T: IsDomNode + AsRef<Node>>(self) -> Result<View<T>, Error> {
        let el: T = match self {
            HydrationKey::Id(id) => {
                let el = utils::document()
                    .get_element_by_id(&id)
                    .with_context(|| MissingId { id })?;
                el.clone().dyn_into::<T>().or_else(|_| {
                    Conversion {
                        from: "Element",
                        to: std::any::type_name::<T>(),
                        node: el,
                    }
                    .fail()
                })?
            }
            HydrationKey::IndexedChildOf { node, index } => {
                let children = node.child_nodes();
                let mut non_empty_children = vec![];
                for i in 0..children.length() {
                    let child = children.get(i).with_context(|| MissingChild {
                        node: node.clone(),
                        index,
                    })?;
                    if child.node_type() == 3 {
                        // This is a text node
                        let has_text: bool = child
                            .node_value()
                            .map(|s| !s.trim().is_empty())
                            .unwrap_or_else(|| false);
                        if has_text {
                            non_empty_children.push(child);
                        }
                    } else {
                        non_empty_children.push(child);
                    }
                }
                let el = non_empty_children
                    .get(index as usize)
                    .with_context(|| MissingChild {
                        node: node.clone(),
                        index,
                    })?
                    .clone();
                el.clone().dyn_into::<T>().or_else(|_| {
                    Conversion {
                        from: "Node",
                        to: std::any::type_name::<T>(),
                        node: el,
                    }
                    .fail()
                })?
            }
        };

        Ok(View::wrapping(el))
    }
}


pub struct Hydrator<T: IsDomNode> {
    pub(crate) create: Box<dyn FnOnce() -> Result<View<T>, Error>>,
    pub(crate) update: Option<Box<dyn FnOnce(&mut View<T>) -> Result<(), Error>>>,
}


impl<T: IsDomNode + AsRef<JsValue>> Hydrator<T> {
    pub fn from_create_fn<F>(f: F) -> Self
    where
        F: FnOnce() -> Result<View<T>, Error> + 'static,
    {
        Hydrator {
            create: Box::new(f),
            update: None,
        }
    }

    pub fn append_update<F>(&mut self, f: F)
    where
        F: FnOnce(&mut View<T>) -> Result<(), Error> + 'static,
    {
        let prev_update = self.update.take();
        self.update = Some(Box::new(|view: &mut View<T>| {
            if let Some(prev) = prev_update {
                prev(view)?
            }
            f(view)
        }));
    }

    pub(crate) fn cast<To: IsDomNode>(self) -> Hydrator<To> {
        let Hydrator {
            create: prev_create,
            update: prev_update,
        } = self;

        Hydrator {
            create: Box::new(|| {
                let view: View<T> = prev_create()?;
                view.try_cast::<To>().map_err(|view| Error::Conversion {
                    from: std::any::type_name::<T>().to_string(),
                    to: std::any::type_name::<To>().to_string(),
                    node: view.dom_ref().as_ref().clone(),
                })
            }),
            update: match prev_update {
                Some(prev_update) => Some(Box::new(|view: &mut View<To>| -> Result<(), Error> {
                    let view = view.clone();
                    match view.try_cast::<T>() {
                        Ok(mut prev_view) => {
                            prev_update(&mut prev_view)?;
                            Ok(())
                        }
                        Err(view) => Conversion {
                            from: std::any::type_name::<T>().to_string(),
                            to: std::any::type_name::<To>().to_string(),
                            node: view.dom_ref().as_ref().clone(),
                        }
                        .fail(),
                    }
                })),
                _ => None,
            },
        }
    }

    /// Hydrates a new [`Gizmo`] from a stateful [`Component`].
    /// If the view cannot be hydrated an error is returned.
    pub fn gizmo<C: Component>(init: C) -> Result<Gizmo<C>, Error> {
        let tx_in = Transmitter::new();
        let rx_out = Receiver::new();
        let view_builder = init.view(&tx_in, &rx_out);
        let hydrated = Hydrator::from(view_builder);
        let view = View::try_from(hydrated)?;

        Ok(Gizmo::from_parts(init, tx_in, rx_out, view))
    }
}


/// [`ViewBuilder`] can be converted into a [`Hydrator`].
impl<T> From<ViewBuilder<T>> for Hydrator<T>
where
    T: JsCast + AsRef<Node> + Clone + 'static,
{
    fn from(builder: ViewBuilder<T>) -> Hydrator<T> {
        let ViewBuilder {
            element,
            ns,
            attribs,
            styles,
            events,
            children,
            replaces,
            posts,
            text,
        } = builder;
        let mut hview: Hydrator<T> = if let Some(tag) = element {
            if let Some(ns) = ns {
                Hydrator::element_ns(&tag, &ns)
            } else {
                Hydrator::element(&tag)
            }
        } else if let Some(effect) = text {
            let text = Hydrator::from(effect);
            text.cast::<T>()
        } else {
            panic!("not hydrating an element - impossible!")
        };

        if events.len() > 0 {
            hview.append_update(|view: &mut View<T>| {
                let t: T = view.dom_ref().clone();
                let mut view: View<EventTarget> = view
                    .clone()
                    .try_cast::<EventTarget>()
                    .map_err(|_| Error::Conversion {
                        from: std::any::type_name::<T>().to_string(),
                        to: std::any::type_name::<EventTarget>().to_string(),
                        node: t.unchecked_into(),
                    })?;
                for cmd in events.into_iter() {
                    match cmd.type_is {
                        EventTargetType::Myself => view.on(&cmd.name, cmd.transmitter),
                        EventTargetType::Window => view.window_on(&cmd.name, cmd.transmitter),
                        EventTargetType::Document => view.document_on(&cmd.name, cmd.transmitter),
                    }
                }
                Ok(())
            });
        }
        if styles.len() > 0 {
            hview.append_update(|view: &mut View<T>| {
                let t: T = view.dom_ref().clone();
                let mut view: View<HtmlElement> = view
                    .clone()
                    .try_cast::<HtmlElement>()
                    .map_err(|_| Error::Conversion {
                        from: std::any::type_name::<T>().to_string(),
                        to: std::any::type_name::<HtmlElement>().to_string(),
                        node: t.unchecked_into(),
                    })?;

                for cmd in styles.into_iter() {
                    view.style(&cmd.name, cmd.effect);
                }
                Ok(())
            });
        }

        if attribs.len() > 0 {
            let may_id = attribs
                .iter()
                .filter_map(|att| match att {
                    AttributeCmd::Attrib { name, effect } if name.as_str() == "id" => {
                        match effect {
                            Effect::OnceNow { now } => Some(now),
                            Effect::OnceNowAndManyLater { now, .. } => Some(now),
                            _ => None,
                        }
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
                .pop();
            if let Some(id) = may_id {
                hview.create = Hydrator::from(HydrationKey::Id(id.to_string())).create;
            }
            hview.append_update(|view: &mut View<T>| {
                let t: T = view.dom_ref().clone();
                let mut view: View<Element> = view
                    .clone()
                    .try_cast::<Element>()
                    .map_err(|_| Error::Conversion {
                        from: std::any::type_name::<T>().to_string(),
                        to: std::any::type_name::<Element>().to_string(),
                        node: t.unchecked_into(),
                    })?;

                for cmd in attribs.into_iter() {
                    match cmd {
                        AttributeCmd::Attrib { name, effect } => {
                            view.attribute(&name, effect);
                        }
                        AttributeCmd::Bool { name, effect } => {
                            view.boolean_attribute(&name, effect);
                        }
                    }
                }
                Ok(())
            });
        }

        for child in children.into_iter() {
            let child = Hydrator::from(child);
            hview.with(child);
        }

        for update in replaces.into_iter() {
            hview.append_update(|view: &mut View<T>| {
                view.this_later(update);
                Ok(())
            });
        }

        for tx in posts.into_iter() {
            hview.post_build(tx);
        }

        hview
    }
}


/// # [`From`] instances for [`Hydrator`]
///
/// Most of these mimic the corresponding [`From`] instances for [`View`],
/// the rest are here for the operation of this module.


impl From<Effect<String>> for Hydrator<Text> {
    fn from(eff: Effect<String>) -> Self {
        // Text alone is not enough to hydrate a view, so we start
        // out with a Hydrator that will err if it is converted to
        // a View.
        let (may_now, may_later) = eff.into();
        let mut hydrate_view = Hydrator::from_create_fn(|| {
            NoHydrationOption {
                tag: may_now.unwrap_or_else(|| "#text".to_string()),
            }
            .fail()
        });

        if let Some(rx) = may_later {
            hydrate_view.append_update(|v: &mut View<Text>| {
                v.rx_text(rx);
                Ok(())
            })
        }

        hydrate_view
    }
}


impl From<Receiver<String>> for Hydrator<Text> {
    fn from(later: Receiver<String>) -> Self {
        let mut hydrate_view = Hydrator::from_create_fn(|| {
            NoHydrationOption {
                tag: "#text".to_string(),
            }
            .fail()
        });
        hydrate_view.append_update(|v: &mut View<Text>| {
            v.rx_text(later);
            Ok(())
        });
        hydrate_view
    }
}


impl From<(&str, Receiver<String>)> for Hydrator<Text> {
    fn from(tuple: (&str, Receiver<String>)) -> Self {
        let eff: Effect<String> = tuple.into();
        eff.into()
    }
}


impl From<(String, Receiver<String>)> for Hydrator<Text> {
    fn from(tuple: (String, Receiver<String>)) -> Self {
        let eff: Effect<String> = tuple.into();
        eff.into()
    }
}


impl From<(&String, Receiver<String>)> for Hydrator<Text> {
    fn from((now, later): (&String, Receiver<String>)) -> Self {
        let tuple = (now.clone(), later);
        let eff: Effect<String> = tuple.into();
        eff.into()
    }
}


impl From<&String> for Hydrator<Text> {
    fn from(text: &String) -> Self {
        let tag = text.to_owned();
        Hydrator::from_create_fn(|| NoHydrationOption { tag }.fail())
    }
}


impl From<String> for Hydrator<Text> {
    fn from(text: String) -> Self {
        Hydrator::from_create_fn(|| NoHydrationOption { tag: text }.fail())
    }
}


impl From<&str> for Hydrator<Text> {
    fn from(tag_or_text: &str) -> Self {
        let tag = tag_or_text.to_owned();
        Hydrator::from_create_fn(|| NoHydrationOption { tag }.fail())
    }
}


impl<T: IsDomNode + AsRef<Node>> From<HydrationKey> for Hydrator<T> {
    fn from(key: HydrationKey) -> Self {
        Hydrator::from_create_fn(move || key.hydrate::<T>())
    }
}


impl<T: IsDomNode> TryFrom<Hydrator<T>> for View<T> {
    type Error = Error;

    fn try_from(hydrate_view: Hydrator<T>) -> Result<View<T>, Self::Error> {
        let mut view = (hydrate_view.create)()?;
        if let Some(update) = hydrate_view.update {
            update(&mut view)?
        }
        Ok(view)
    }
}


/// # ElementView


impl<T: IsDomNode + AsRef<Node>> ElementView for Hydrator<T> {
    fn element(tag: &str) -> Self {
        let tag = tag.to_owned();
        Hydrator::from_create_fn(|| NoHydrationOption { tag }.fail())
    }

    fn element_ns(tag: &str, ns: &str) -> Self {
        let tag = format!("{}:{}", tag, ns);
        Hydrator::from_create_fn(|| NoHydrationOption { tag }.fail())
    }
}


/// # AttributeView


impl<T: IsDomNode + AsRef<Node> + AsRef<Element> + 'static> AttributeView for Hydrator<T> {
    fn attribute<E: Into<Effect<String>>>(&mut self, name: &str, eff: E) {
        let (may_now, may_later) = eff.into().into();
        if let Some(now) = may_now {
            if name == "id" {
                self.create = Hydrator::from(HydrationKey::Id(now.to_string())).create;
            }
        }

        if let Some(later) = may_later {
            let name = name.to_string();
            self.append_update(move |v| Ok(v.attribute(&name, later)));
        }
    }

    fn boolean_attribute<E: Into<Effect<bool>>>(&mut self, name: &str, eff: E) {
        let (_may_now, may_later) = eff.into().into();
        if let Some(later) = may_later {
            let name = name.to_string();
            self.append_update(move |v| Ok(v.boolean_attribute(&name, later)));
        }
    }
}


/// # StyleView


impl<T: IsDomNode + AsRef<HtmlElement>> StyleView for Hydrator<T> {
    fn style<E: Into<Effect<String>>>(&mut self, name: &str, eff: E) {
        let eff: Effect<_> = eff.into();
        let (_, may_later) = eff.into();
        if let Some(later) = may_later {
            let name = name.to_string();
            self.append_update(move |v| Ok(v.style(&name, later)));
        }
    }
}


/// # EventTargetView


impl<T: IsDomNode + AsRef<EventTarget>> EventTargetView for Hydrator<T> {
    fn on(&mut self, ev_name: &str, tx: Transmitter<Event>) {
        let ev_name = ev_name.to_string();
        self.append_update(move |v: &mut View<T>| {
            v.on(&ev_name, tx);
            Ok(())
        });
    }

    fn window_on(&mut self, ev_name: &str, tx: Transmitter<Event>) {
        let ev_name = ev_name.to_string();
        self.append_update(move |v| Ok(v.window_on(&ev_name, tx)));
    }

    fn document_on(&mut self, ev_name: &str, tx: Transmitter<Event>) {
        let ev_name = ev_name.to_string();
        self.append_update(move |v| Ok(v.document_on(&ev_name, tx)));
    }
}


/// # ParentView


impl<P, C> ParentView<Hydrator<C>> for Hydrator<P>
where
    P: IsDomNode + AsRef<Node>,
    C: IsDomNode + AsRef<Node>,
{
    fn with(&mut self, mut child: Hydrator<C>) {
        self.append_update(|v: &mut View<P>| {
            let node: Node = (v.dom_ref().as_ref() as &Node).clone();
            let index = v.stored_views_len() as u32;
            child.create = Hydrator::from(HydrationKey::IndexedChildOf { node, index }).create;
            let child_view: View<C> = View::try_from(child)?;
            v.store_view(child_view.upcast());
            Ok(())
        });
    }
}


/// # PostBuildView


impl<T: JsCast + Clone + 'static> PostBuildView for Hydrator<T> {
    type DomNode = T;

    fn post_build(&mut self, tx: Transmitter<T>) {
        self.append_update(move |v| Ok(v.post_build(tx)));
    }
}
