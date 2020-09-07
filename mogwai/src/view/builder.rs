//! A low cost intermediate structure for creating views either by
//! hydration from the DOM or by creating a fresh view from scratch.
//!
//! Here we attempt to have our cake and eat it too.
pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
use web_sys::Node;
pub use web_sys::{Element, Event, EventTarget, HtmlInputElement, Text};
use snafu::OptionExt;

use crate::{
    utils,
    prelude::{Effect, HydrateView, Receiver, Transmitter, View},
    view::{hydration, interface::*},
};


enum AttributeCmd {
    Attrib {
        name: String,
        effect: Effect<String>,
    },
    Bool {
        name: String,
        effect: Effect<bool>,
    },
}


struct StyleCmd {
    name: String,
    effect: Effect<String>,
}


enum EventTargetType {
    Myself,
    Window,
    Document,
}


struct EventTargetCmd {
    type_is: EventTargetType,
    name: String,
    transmitter: Transmitter<Event>,
}


pub struct ViewBuilder<T: JsCast> {
    element: Option<String>,
    ns: Option<String>,
    text: Option<Effect<String>>,
    attribs: Vec<AttributeCmd>,
    styles: Vec<StyleCmd>,
    events: Vec<EventTargetCmd>,
    posts: Vec<Transmitter<T>>,
    children: Vec<ViewBuilder<Node>>,
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

    //pub fn new<VF, HF>(view_fn: VF, hydrate_fn: HF) -> Self
    //where
    //    VF: FnOnce() -> View<T> + 'static,
    //    HF: FnOnce() -> HydrateView<T> + 'static,
    //{
    //    let view_fn = Box::new(view_fn);
    //    let hydrate_fn = Box::new(hydrate_fn);
    //    ViewBuilder {
    //        view_fn,
    //        hydrate_fn,
    //    }
    //}

    //pub fn append_update<VF, HF>(&mut self, view_fn: VF, hydrate_fn: HF)
    //where
    //    VF: FnOnce(View<T>) -> View<T> + 'static,
    //    HF: FnOnce(HydrateView<T>) -> HydrateView<T> + 'static,
    //{
    //    let f = std::mem::replace(&mut self.view_fn, Box::new(|| panic!()));
    //    self.view_fn = Box::new(move || view_fn(f()));

    //    let f = std::mem::replace(&mut self.hydrate_fn, Box::new(|| panic!()));
    //    self.hydrate_fn = Box::new(move || hydrate_fn(f()));
    //}
}


/// [`ViewBuilder`] can be converted into a [`HydrateView`].
impl<T> From<ViewBuilder<T>> for HydrateView<T>
where
    T: JsCast + AsRef<Node> + Clone + 'static,
{
    fn from(builder: ViewBuilder<T>) -> HydrateView<T> {
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
        let mut hview: HydrateView<T> = if let Some(tag) = element {
            if let Some(ns) = ns {
                HydrateView::element_ns(&tag, &ns)
            } else {
                HydrateView::element(&tag)
            }
        } else if let Some(effect) = text {
            let text = HydrateView::from(effect);
            text.cast::<T>()
        } else {
            panic!("not hydrating an element - impossible!")
        };

        if events.len() > 0 {
            hview.append_update(|view: &mut View<T>| {
                let t:T = {
                    let t:&T = &view;
                    t.clone()
                };
                let myself = t.dyn_ref::<EventTarget>().with_context(|| hydration::Conversion {
                    from: std::any::type_name::<T>().to_string(),
                    to: std::any::type_name::<EventTarget>().to_string(),
                    node: view.element.as_ref().clone()
                })?;
                let window = utils::window();
                let doc = utils::document();
                for cmd in events.into_iter() {
                    match cmd.type_is {
                        EventTargetType::Myself => view.add_event(myself, &cmd.name, cmd.transmitter),
                        EventTargetType::Window => view.add_event(&window, &cmd.name, cmd.transmitter),
                        EventTargetType::Document => view.add_event(&doc, &cmd.name, cmd.transmitter),
                    }
                }
                Ok(())
            });
        }
        if styles.len() > 0 {
            hview.append_update(|view: &mut View<T>| {
                for cmd in styles.into_iter() {
                    view.add_style(&cmd.name, cmd.effect);
                }
                Ok(())
            });
        }

        if attribs.len() > 0 {
            hview.append_update(|view: &mut View<T>| {
                for cmd in attribs.into_iter() {
                    match cmd {
                        AttributeCmd::Attrib { name, effect } => {
                            view.add_attribute(&name, effect);
                        }
                        AttributeCmd::Bool { name, effect } => {
                            view.add_boolean_attribute(&name, effect);
                        }
                    }
                }
                Ok(())
            });
        }

        for tx in posts.into_iter() {
            hview.post_build(tx);
        }

        for child in children.into_iter() {
            let child = HydrateView::from(child);
            hview.with(child);
        }

        hview
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

        if events.len() > 0 && view.has_type::<EventTarget>() {
            let mut ev_view: View<EventTarget> = View::default();
            view.swap(&mut ev_view);
            for cmd in events.into_iter() {
                match cmd.type_is {
                    EventTargetType::Myself => ev_view.on(&cmd.name, cmd.transmitter),
                    EventTargetType::Window => ev_view.window_on(&cmd.name, cmd.transmitter),
                    EventTargetType::Document => ev_view.document_on(&cmd.name, cmd.transmitter)
                }
            }
            view.swap(&mut ev_view);
        }

        if styles.len() > 0 && view.has_type::<HtmlElement>() {
            let mut style_view: View<HtmlElement> = View::default();
            view.swap(&mut style_view);
            for cmd in styles.into_iter() {
                style_view.style(&cmd.name, cmd.effect);
            }
            view.swap(&mut style_view);
        }

        if attribs.len() > 0 && view.has_type::<Element>() {
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

        for tx in posts.into_iter() {
            view.post_build(tx);
        }

        for child in children.into_iter() {
            let child = View::from(child);
            view.with(child);
        }

        view
    }
}


/// # [`From`] instances for [`HydrateView`]
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
