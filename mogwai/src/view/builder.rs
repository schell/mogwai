//! A low cost intermediate structure for creating views either by
//! hydration from the DOM or by creating a fresh view from scratch.
//!
//! Here we attempt to have our cake and eat it too.
pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
use web_sys::Node;
pub use web_sys::{Element, Event, EventTarget, HtmlInputElement, Text};

use crate::{
    prelude::{Effect, Receiver, Transmitter, View},
    view::interface::*,
};


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


pub struct StyleCmd {
    pub name: String,
    pub effect: Effect<String>,
}


pub enum EventTargetType {
    Myself,
    Window,
    Document,
}


pub struct EventTargetCmd {
    pub type_is: EventTargetType,
    pub name: String,
    pub transmitter: Transmitter<Event>,
}


pub struct ViewBuilder<T: JsCast> {
    pub element: Option<String>,
    pub ns: Option<String>,
    pub text: Option<Effect<String>>,
    pub attribs: Vec<AttributeCmd>,
    pub styles: Vec<StyleCmd>,
    pub events: Vec<EventTargetCmd>,
    pub posts: Vec<Transmitter<T>>,
    pub children: Vec<ViewBuilder<Node>>,
}


impl<T: JsCast> Default for ViewBuilder<T> {
    fn default() -> Self {
        ViewBuilder {
            element: None,
            ns: None,
            text: None,
            attribs: vec![],
            styles: vec![],
            events: vec![],
            posts: vec![],
            children: vec![],
        }
    }
}


impl<T: Clone + JsCast + AsRef<Node> + 'static> ViewBuilder<T> {
    fn to_node(self) -> ViewBuilder<Node> {
        ViewBuilder {
            element: self.element,
            ns: self.ns,
            text: self.text,
            attribs: self.attribs,
            styles: self.styles,
            events: self.events,
            children: self.children,
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
impl<T> From<ViewBuilder<T>> for View<T>
where
    T: JsCast + Clone + AsRef<Node> + 'static,
{
    fn from(builder: ViewBuilder<T>) -> View<T> {
        let ViewBuilder {
            element,
            ns,
            attribs,
            styles,
            events,
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

        let has_event_target = cfg!(not(target_arch = "wasm32")) || view.has_type::<EventTarget>();
        if events.len() > 0 && has_event_target {
            let mut ev_view: View<EventTarget> = View::default();
            view.swap(&mut ev_view);
            for cmd in events.into_iter() {
                match cmd.type_is {
                    EventTargetType::Myself => ev_view.on(&cmd.name, cmd.transmitter),
                    EventTargetType::Window => ev_view.window_on(&cmd.name, cmd.transmitter),
                    EventTargetType::Document => ev_view.document_on(&cmd.name, cmd.transmitter),
                }
            }
            view.swap(&mut ev_view);
        }

        let has_html_element = cfg!(not(target_arch = "wasm32")) || view.has_type::<HtmlElement>();
        if styles.len() > 0 && has_html_element {
            let mut style_view: View<HtmlElement> = View::default();
            view.swap(&mut style_view);
            for cmd in styles.into_iter() {
                style_view.style(&cmd.name, cmd.effect);
            }
            view.swap(&mut style_view);
        }

        let has_element = cfg!(not(target_arch = "wasm32")) || view.has_type::<Element>();
        if attribs.len() > 0 && has_element {
            let mut att_view: View<Element> = View::default();
            view.swap(&mut att_view);
            for cmd in attribs.into_iter() {
                match cmd {
                    AttributeCmd::Attrib { name, effect } => {
                        att_view.attribute(&name, effect);
                    }
                    AttributeCmd::Bool { name, effect } => {
                        att_view.boolean_attribute(&name, effect);
                    }
                }
            }
            view.swap(&mut att_view);
        }

        for child in children.into_iter() {
            let child = View::from(child);
            view.with(child);
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


impl<T: JsCast + AsRef<Node> + 'static> ElementView for ViewBuilder<T> {
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


impl<T: JsCast + AsRef<Node> + AsRef<Element> + 'static> AttributeView for ViewBuilder<T> {
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


impl<T: JsCast + AsRef<HtmlElement> + 'static> StyleView for ViewBuilder<T> {
    fn style<E: Into<Effect<String>>>(&mut self, name: &str, eff: E) {
        let effect = eff.into();
        self.styles.push(StyleCmd {
            name: name.to_string(),
            effect,
        });
    }
}


/// # EventTargetView


impl<T: JsCast + AsRef<EventTarget> + 'static> EventTargetView for ViewBuilder<T> {
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
    P: JsCast + AsRef<Node> + 'static,
    C: JsCast + Clone + AsRef<Node> + 'static,
{
    fn with(&mut self, child: ViewBuilder<C>) {
        let child: ViewBuilder<Node> = child.to_node();
        self.children.push(child);
    }
}


/// # PostBuildView


impl<T: JsCast + Clone + 'static> PostBuildView for ViewBuilder<T> {
    type DomNode = T;

    fn post_build(&mut self, transmitter: Transmitter<T>) {
        self.posts.push(transmitter);
    }
}
