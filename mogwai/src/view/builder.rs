//! A low cost intermediate structure for creating views either by
//! hydration from the DOM or by creating a fresh view from scratch.
//!
//! Here we attempt to have our cake and eat it too.
pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
use web_sys::Node;
pub use web_sys::{Element, Event, EventTarget, HtmlInputElement, Text};

use crate::{
    prelude::{Effect, Receiver, Transmitter, View, IsDomNode},
    view::interface::*,
};


#[derive(Clone)]
pub enum AttributeCmd {
    Attrib {
        name: String,
        effect: Effect<String>,
    },
    Bool {
        name: String,
        effect: Effect<bool>,
    },
}


#[derive(Clone)]
pub struct StyleCmd {
    pub name: String,
    pub effect: Effect<String>,
}


#[derive(Clone)]
pub enum EventTargetType {
    Myself,
    Window,
    Document,
}


#[derive(Clone)]
pub struct EventTargetCmd {
    pub type_is: EventTargetType,
    pub name: String,
    pub transmitter: Transmitter<Event>,
}


pub struct ViewBuilder<T: IsDomNode> {
    pub element: Option<String>,
    pub ns: Option<String>,
    pub text: Option<Effect<String>>,
    pub attribs: Vec<AttributeCmd>,
    pub styles: Vec<StyleCmd>,
    pub events: Vec<EventTargetCmd>,
    pub posts: Vec<Transmitter<T>>,
    pub replaces: Vec<Receiver<View<T>>>,
    pub children: Vec<ViewBuilder<Node>>,
}


impl<T: IsDomNode> Clone for ViewBuilder<T> {
    fn clone(&self) -> Self {
        ViewBuilder {
            element: self.element.clone(),
            ns: self.ns.clone(),
            text: self.text.clone(),
            attribs: self.attribs.clone(),
            styles: self.styles.clone(),
            events: self.events.clone(),
            posts: self.posts.clone(),
            children: self.children.clone(),
            replaces: self
                .replaces
                .iter()
                .map(Receiver::branch)
                .collect(),
        }
    }
}


impl<T: IsDomNode> Default for ViewBuilder<T> {
    fn default() -> Self {
        ViewBuilder {
            element: None,
            ns: None,
            text: None,
            attribs: vec![],
            styles: vec![],
            events: vec![],
            posts: vec![],
            replaces: vec![],
            children: vec![],
        }
    }
}


impl<T: IsDomNode + AsRef<Node>> ViewBuilder<T> {
    fn to_node(self) -> ViewBuilder<Node> {
        ViewBuilder {
            element: self.element,
            ns: self.ns,
            text: self.text,
            attribs: self.attribs,
            styles: self.styles,
            events: self.events,
            children: self.children,
            replaces: self
                .replaces
                .into_iter()
                .map(|update| update.branch_map(|builder| {
                    View::from(builder.clone()).upcast::<Node>()
                }))
                .collect(),
            posts: self
                .posts
                .into_iter()
                .map(|tx: Transmitter<T>| -> Transmitter<Node> {
                    tx.contra_map(|node: &Node| -> T {
                        let t: &T = node.unchecked_ref();
                        t.clone()
                    })
                })
                .collect(),
        }
    }
}


/// [`ViewBuilder`] can be converted into a fresh [`View`].
impl<T: IsDomNode + AsRef<Node>> From<ViewBuilder<T>> for View<T> {
    fn from(builder: ViewBuilder<T>) -> View<T> {
        let ViewBuilder {
            element,
            ns,
            attribs,
            styles,
            events,
            replaces,
            children,
            posts,
            text,
        } = builder;
        let mut view: View<T> = match element {
            Some(tag) => match ns {
                Some(ns) => View::element_ns(&tag, &ns),
                _ => View::element(&tag),
            },
            _ => match text {
                Some(effect) => {
                    let text = View::from(effect);
                    text.try_cast::<T>()
                        .unwrap_or_else(|_| panic!("not text - impossible!"))
                }
                _ => panic!("not an element - impossible!"),
            },
        };

        {
            fn has_type<X: IsDomNode>(val: &JsValue) -> bool {
                if cfg!(target_arch = "wasm32") {
                    val.has_type::<X>()
                } else {
                    true
                }
            }

            let mut internals = view.internals.borrow_mut();

            if events.len() > 0 && has_type::<EventTarget>(&internals.element) {
                for cmd in events.into_iter() {
                    match cmd.type_is {
                        EventTargetType::Myself => {
                            internals.add_event_on_this(&cmd.name, cmd.transmitter);
                        }
                        EventTargetType::Window => {
                            internals.add_event_on_window(&cmd.name, cmd.transmitter);
                        }
                        EventTargetType::Document => {
                            internals.add_event_on_document(&cmd.name, cmd.transmitter);
                        }
                    }
                }
            }

            if styles.len() > 0 && has_type::<HtmlElement>(&internals.element) {
                for cmd in styles.into_iter() {
                    internals.add_style(&cmd.name, cmd.effect);
                }
            }

            if attribs.len() > 0 && has_type::<Element>(&internals.element) {
                for cmd in attribs.into_iter() {
                    match cmd {
                        AttributeCmd::Attrib { name, effect } => {
                            internals.add_attribute(&name, effect);
                        }
                        AttributeCmd::Bool { name, effect } => {
                            internals.add_boolean_attribute(&name, effect);
                        }
                    }
                }
            }
        }

        for builder in children.into_iter() {
            let child: View<Node> = View::from(builder);
            view.with(child);
        }

        for update in replaces.into_iter() {
            view.this_later(update);
        }

        for tx in posts.into_iter() {
            view.post_build(tx);
        }

        view
    }
}


/// # [`From`] instances for [`Hydrator`]
///
/// Most of these mimic the corresponding [`From`] instances for [`View`],
/// the rest are here for the operation of this module.


impl From<Effect<String>> for ViewBuilder<Text> {
    fn from(effect: Effect<String>) -> Self {
        let mut builder = ViewBuilder::default();
        builder.text = Some(effect);
        builder
    }
}


impl From<(&str, Receiver<String>)> for ViewBuilder<Text> {
    fn from(tuple: (&str, Receiver<String>)) -> Self {
        let eff: Effect<String> = tuple.into();
        eff.into()
    }
}


impl From<(String, Receiver<String>)> for ViewBuilder<Text> {
    fn from(tuple: (String, Receiver<String>)) -> Self {
        let eff: Effect<String> = tuple.into();
        eff.into()
    }
}


impl From<(&String, Receiver<String>)> for ViewBuilder<Text> {
    fn from((now, later): (&String, Receiver<String>)) -> Self {
        let tuple = (now.clone(), later);
        let eff: Effect<String> = tuple.into();
        eff.into()
    }
}


impl From<Receiver<String>> for ViewBuilder<Text> {
    fn from(later: Receiver<String>) -> Self {
        let tuple = ("".to_string(), later);
        let eff: Effect<String> = tuple.into();
        eff.into()
    }
}


impl From<&String> for ViewBuilder<Text> {
    fn from(text: &String) -> Self {
        let text = text.to_owned();
        ViewBuilder::from(text)
    }
}


impl From<&str> for ViewBuilder<Text> {
    fn from(tag_or_text: &str) -> Self {
        let text = tag_or_text.to_owned();
        ViewBuilder::from(text)
    }
}


impl From<String> for ViewBuilder<Text> {
    fn from(text: String) -> Self {
        let effect = Effect::OnceNow { now: text };
        ViewBuilder::from(effect)
    }
}


/// # ElementView


impl<T: IsDomNode + AsRef<Node> + 'static> ElementView for ViewBuilder<T> {
    fn element(tag: &str) -> Self {
        let mut builder = ViewBuilder::default();
        builder.element = Some(tag.into());
        builder
    }

    fn element_ns(tag: &str, ns: &str) -> Self {
        let mut builder = ViewBuilder::default();
        builder.element = Some(tag.into());
        builder.ns = Some(ns.into());
        builder
    }
}


/// # AttributeView


impl<T: IsDomNode + AsRef<Node> + AsRef<Element> + 'static> AttributeView for ViewBuilder<T> {
    fn attribute<E: Into<Effect<String>>>(&mut self, name: &str, eff: E) {
        let effect = eff.into();
        self.attribs.push(AttributeCmd::Attrib {
            name: name.to_string(),
            effect,
        });
    }

    fn boolean_attribute<E: Into<Effect<bool>>>(&mut self, name: &str, eff: E) {
        let effect = eff.into();
        self.attribs.push(AttributeCmd::Bool {
            name: name.to_string(),
            effect,
        });
    }
}


/// # StyleView


impl<T: IsDomNode + AsRef<HtmlElement> + 'static> StyleView for ViewBuilder<T> {
    fn style<E: Into<Effect<String>>>(&mut self, name: &str, eff: E) {
        let effect = eff.into();
        self.styles.push(StyleCmd {
            name: name.to_string(),
            effect,
        });
    }
}


/// # EventTargetView


impl<T: IsDomNode + AsRef<EventTarget> + 'static> EventTargetView for ViewBuilder<T> {
    fn on(&mut self, ev_name: &str, tx: Transmitter<Event>) {
        self.events.push(EventTargetCmd {
            type_is: EventTargetType::Myself,
            name: ev_name.to_string(),
            transmitter: tx,
        });
    }

    fn window_on(&mut self, ev_name: &str, tx: Transmitter<Event>) {
        self.events.push(EventTargetCmd {
            type_is: EventTargetType::Window,
            name: ev_name.to_string(),
            transmitter: tx,
        });
    }

    fn document_on(&mut self, ev_name: &str, tx: Transmitter<Event>) {
        self.events.push(EventTargetCmd {
            type_is: EventTargetType::Document,
            name: ev_name.to_string(),
            transmitter: tx,
        });
    }
}


/// # ParentView


impl<P, C> ParentView<ViewBuilder<C>> for ViewBuilder<P>
where
    P: IsDomNode + AsRef<Node>,
    C: IsDomNode + AsRef<Node>,
{
    fn with(&mut self, child: ViewBuilder<C>) {
        self.children.push(child.to_node());
    }
}


/// # PostBuildView


impl<T: IsDomNode + Clone + 'static> PostBuildView for ViewBuilder<T> {
    type DomNode = T;

    fn post_build(&mut self, transmitter: Transmitter<T>) {
        self.posts.push(transmitter);
    }
}
