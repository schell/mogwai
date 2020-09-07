//! Types and [`TryFrom`] instances that can 're-animate' views or portions of views from the DOM.
use snafu::{OptionExt, Snafu};
pub use std::convert::TryFrom;
pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
pub use web_sys::{Element, Event, EventTarget, HtmlElement, HtmlInputElement};
use web_sys::{Node, Text};

use crate::{
    prelude::{Effect, Receiver, Transmitter, View, IsDomNode},
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


pub struct HydrateView<T: IsDomNode> {
    create: Box<dyn FnOnce() -> Result<View<T>, Error>>,
    update: Option<Box<dyn FnOnce(&mut View<T>) -> Result<(), Error>>>,
}


impl<T: IsDomNode> HydrateView<T> {
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

    pub(crate) fn cast<To: IsDomNode>(self) -> HydrateView<To> {
        let HydrateView {
            create: prev_create,
            update: prev_update,
        } = self;

        HydrateView {
            create: Box::new(|| {
                let view: View<T> = prev_create()?;
                view.try_cast::<To>().map_err(|view| Error::Conversion {
                    from: std::any::type_name::<T>().to_string(),
                    to: std::any::type_name::<To>().to_string(),
                    node: view.element.as_ref().clone(),
                })
            }),
            update: match prev_update {
                Some(prev_update) => Some(Box::new(|view: &mut View<To>| -> Result<(), Error> {
                    if view.element.unchecked_ref::<To>().has_type::<T>() {
                        let mut prev_view: View<T> = View::default();
                        view.swap(&mut prev_view);
                        prev_update(&mut prev_view)?;
                        view.swap(&mut prev_view);
                        Ok(())
                    } else {
                        Conversion {
                            from: std::any::type_name::<T>().to_string(),
                            to: std::any::type_name::<To>().to_string(),
                            node: view.element.as_ref().clone(),
                        }.fail()
                    }
                })),
                _ => None,
            },
        }
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
        let (may_now, may_later) = eff.into();
        let mut hydrate_view = HydrateView::from_create_fn(|| {
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


impl<T: IsDomNode + AsRef<Node>> From<HydrationKey> for HydrateView<T> {
    fn from(key: HydrationKey) -> Self {
        HydrateView::from_create_fn(move || key.hydrate::<T>())
    }
}


impl<T: IsDomNode> TryFrom<HydrateView<T>> for View<T> {
    type Error = Error;

    fn try_from(hydrate_view: HydrateView<T>) -> Result<View<T>, Self::Error> {
        let mut view = (hydrate_view.create)()?;
        if let Some(update) = hydrate_view.update {
            update(&mut view)?
        }
        Ok(view)
    }
}


/// # ElementView


impl<T: IsDomNode + AsRef<Node>> ElementView for HydrateView<T> {
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


impl<T: IsDomNode + AsRef<Node> + AsRef<Element> + 'static> AttributeView for HydrateView<T> {
    fn attribute<E: Into<Effect<String>>>(&mut self, name: &str, eff: E) {
        let (may_now, may_later) = eff.into().into();
        if let Some(now) = may_now {
            if name == "id" {
                self.create = HydrateView::from(HydrationKey::Id(now.to_string())).create;
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


impl<T: IsDomNode + AsRef<HtmlElement>> StyleView for HydrateView<T> {
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


impl<T: IsDomNode + AsRef<EventTarget>> EventTargetView for HydrateView<T> {
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


impl<P, C> ParentView<HydrateView<C>> for HydrateView<P>
where
    P: IsDomNode + AsRef<Node>,
    C: IsDomNode + AsRef<Node>,
{
    fn with(&mut self, mut child: HydrateView<C>) {
        self.append_update(|v: &mut View<P>| {
            let node = (v.as_ref() as &Node).clone();
            let index = v.children.len() as u32;
            child.create = HydrateView::from(HydrationKey::IndexedChildOf { node, index }).create;
            let child_view: View<C> = View::try_from(child)?;
            v.children.push(child_view.upcast());
            Ok(())
        });
    }
}


/// # PostBuildView


impl<T: JsCast + Clone + 'static> PostBuildView for HydrateView<T> {
    type DomNode = T;

    fn post_build(&mut self, tx: Transmitter<T>) {
        self.append_update(move |v| Ok(v.post_build(tx)));
    }
}
