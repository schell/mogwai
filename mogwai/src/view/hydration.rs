//! Types and [`TryFrom`] instances that can 're-animate' views or portions of views from the DOM.
use snafu::{OptionExt, Snafu};
pub use std::convert::TryFrom;
pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
pub use web_sys::{Element, Event, EventTarget, HtmlElement, HtmlInputElement};
use web_sys::{Node, Text};

use crate::{
    prelude::{Effect, Receiver, Transmitter, View},
    utils,
    view::interface::*,
};


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

    #[snafu(display("Could not convert from '{}' to '{}' for node '{}' containing '{:?}", from, to, node.node_name(), node.node_value()))]
    Conversion {
        from: String,
        to: String,
        node: Node,
    },

    #[snafu(display("View cannot be hydrated"))]
    ViewOnly
}


pub enum HydrationKey {
    Id(String),
    IndexedChildOf { node: Node, index: u32 },
}


impl HydrationKey {
    pub fn hydrate<T: JsCast + AsRef<Node>>(self) -> Result<View<T>, Error> {
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
                    .with_context(|| MissingChild { node: node.clone(), index })?
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


pub struct HydrateView<T: JsCast> {
    create: Box<dyn FnOnce() -> Result<View<T>, Error>>,
    update: Option<Box<dyn FnOnce(View<T>) -> Result<View<T>, Error>>>,
}


impl<T: JsCast + 'static> HydrateView<T> {
    pub fn from_create_fn<F>(f: F) -> Self
    where
        F: FnOnce() -> Result<View<T>, Error> + 'static,
    {
        HydrateView {
            create: Box::new(f),
            update: None,
        }
    }

    pub fn append_update<F>(&mut self, f: F)
    where
        F: FnOnce(View<T>) -> Result<View<T>, Error> + 'static,
    {
        let prev_update = self.update.take();
        self.update = Some(Box::new(|view: View<T>| {
            let view = if let Some(prev) = prev_update {
                prev(view)
            } else {
                Ok(view)
            }?;
            f(view)
        }));
    }
}


/// # [`From`] instances for [`HydrateView`]
///
/// Most of these mimic the corresponding [`From`] instances for [`View`],
/// the rest are here for the operation of this module.


impl From<Effect<String>> for HydrateView<Text> {
    fn from(eff: Effect<String>) -> Self {
        // Text alone is not enough to hydrate a view, so we start
        // out with a HydrateView that will err if it is converted to
        // a View.
        let (may_now, may_later) = eff.into_some();
        let mut hydrate_view = HydrateView::from_create_fn(|| {
            NoHydrationOption {
                tag: may_now.unwrap_or_else(|| "#text".to_string()),
            }
            .fail()
        });
        if let Some(rx) = may_later {
            hydrate_view.append_update(|mut v: View<Text>| {
                v.rx_text(rx);
                Ok(v)
            })
        }

        hydrate_view
    }
}


impl From<(&str, Receiver<String>)> for HydrateView<Text> {
    fn from(tuple: (&str, Receiver<String>)) -> Self {
        let eff: Effect<String> = tuple.into();
        eff.into()
    }
}


impl From<(String, Receiver<String>)> for HydrateView<Text> {
    fn from(tuple: (String, Receiver<String>)) -> Self {
        let eff: Effect<String> = tuple.into();
        eff.into()
    }
}


impl From<(&String, Receiver<String>)> for HydrateView<Text> {
    fn from((now, later): (&String, Receiver<String>)) -> Self {
        let tuple = (now.clone(), later);
        let eff: Effect<String> = tuple.into();
        eff.into()
    }
}


impl From<&String> for HydrateView<Text> {
    fn from(text: &String) -> Self {
        let tag = text.to_owned();
        HydrateView::from_create_fn(|| NoHydrationOption { tag }.fail())
    }
}


impl From<String> for HydrateView<Text> {
    fn from(text: String) -> Self {
        HydrateView::from_create_fn(|| NoHydrationOption { tag: text }.fail())
    }
}


impl From<&str> for HydrateView<Text> {
    fn from(tag_or_text: &str) -> Self {
        let tag = tag_or_text.to_owned();
        HydrateView::from_create_fn(|| NoHydrationOption { tag }.fail())
    }
}


impl<T: JsCast + AsRef<Node> + 'static> From<HydrationKey> for HydrateView<T> {
    fn from(key: HydrationKey) -> Self {
        HydrateView::from_create_fn(move || key.hydrate::<T>())
    }
}


impl<T: JsCast> TryFrom<HydrateView<T>> for View<T> {
    type Error = Error;

    fn try_from(hydrate_view: HydrateView<T>) -> Result<View<T>, Self::Error> {
        let view = (hydrate_view.create)()?;
        if let Some(update) = hydrate_view.update {
            update(view)
        } else {
            Ok(view)
        }
    }
}


/// # ElementView


impl<T: JsCast + AsRef<Node> + 'static> ElementView for HydrateView<T> {
    fn element(tag: &str) -> Self {
        let tag = tag.to_owned();
        HydrateView::from_create_fn(|| NoHydrationOption { tag }.fail())
    }

    fn element_ns(tag: &str, ns: &str) -> Self {
        let tag = format!("{}:{}", tag, ns);
        HydrateView::from_create_fn(|| NoHydrationOption { tag }.fail())
    }
}


/// # AttributeView


impl<T: JsCast + AsRef<Node> + AsRef<Element> + 'static> AttributeView for HydrateView<T> {
    fn attribute<E: Into<Effect<String>>>(mut self, name: &str, eff: E) -> Self {
        let (may_now, may_later) = eff.into().into_some();
        if let Some(now) = may_now {
            if name == "id" {
                self.create = HydrateView::from(HydrationKey::Id(now.to_string())).create;
            }
        }

        if let Some(later) = may_later {
            let name = name.to_string();
            self.append_update(move |v| Ok(v.attribute(&name, later)));
        }
        self
    }


    fn boolean_attribute<E: Into<Effect<bool>>>(mut self, name: &str, eff: E) -> Self {
        let (_may_now, may_later) = eff.into().into_some();
        if let Some(later) = may_later {
            let name = name.to_string();
            self.append_update(move |v| Ok(v.boolean_attribute(&name, later)));
        }
        self
    }
}


/// # StyleView


impl<T: JsCast + AsRef<HtmlElement> + 'static> StyleView for HydrateView<T> {
    fn style<E: Into<Effect<String>>>(mut self, name: &str, eff: E) -> Self {
        if let Some(later) = eff.into().into_some().1 {
            let name = name.to_string();
            self.append_update(move |v| Ok(v.style(&name, later)));
        }
        self
    }
}


/// # EventTargetView


impl<T: JsCast + AsRef<EventTarget> + 'static> EventTargetView for HydrateView<T> {
    fn on(mut self, ev_name: &str, tx: Transmitter<Event>) -> Self {
        let ev_name = ev_name.to_string();
        self.append_update(move |v: View<T>| {
            Ok(v.on(&ev_name, tx))
        });
        self
    }

    fn window_on(mut self, ev_name: &str, tx: Transmitter<Event>) -> Self {
        let ev_name = ev_name.to_string();
        self.append_update(move |v| Ok(v.window_on(&ev_name, tx)));
        self
    }

    fn document_on(mut self, ev_name: &str, tx: Transmitter<Event>) -> Self {
        let ev_name = ev_name.to_string();
        self.append_update(move |v| Ok(v.document_on(&ev_name, tx)));
        self
    }
}


/// # ParentView


impl<P, C> ParentView<HydrateView<C>> for HydrateView<P>
where
    P: JsCast + AsRef<Node> + 'static,
    C: JsCast + Clone + AsRef<Node> + 'static,
{
    fn with(mut self, mut child: HydrateView<C>) -> Self {
        self.append_update(|mut v: View<P>| {
            let node = (v.as_ref() as &Node).clone();
            let index = v.children.len() as u32;
            child.create = HydrateView::from(HydrationKey::IndexedChildOf { node, index }).create;
            let child_view: View<C> = View::try_from(child)?;
            v.children.push(child_view.upcast());
            Ok(v)
        });
        self
    }
}


/// # PostBuildView


impl<T: JsCast + Clone + 'static> PostBuildView for HydrateView<T> {
    type DomNode = T;

    fn post_build(mut self, tx: Transmitter<T>) -> Self {
        self.append_update(move |v| Ok(v.post_build(tx)));
        self
    }
}
