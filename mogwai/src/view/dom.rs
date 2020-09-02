//! Widgets for the browser.
use std::{cell::RefCell, collections::HashMap, marker::PhantomData, ops::Deref, rc::Rc};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
pub use web_sys::{Element, Event, EventTarget, HtmlElement, HtmlInputElement};
use web_sys::{Node, Text};

use crate::{
    utils,
    view::interface::*,
    prelude::{Component, Effect, Gizmo, Receiver, Transmitter},
    ssr::Node as SsrNode,
};


#[derive(Clone)]
#[allow(dead_code)]
pub(crate) enum NameOrText {
    Name(Rc<RefCell<String>>),
    Text(Rc<RefCell<String>>),
}


/// A wrapper for closures that can be used as callbacks.
#[derive(Clone)]
pub struct MogwaiCallback {
    #[cfg(target_arch = "wasm32")]
    pub(crate) callback: Rc<Closure<dyn FnMut(JsValue)>>,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) callback: Rc<Box<dyn FnMut(JsValue)>>,
}


impl MogwaiCallback {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new<F>(f: F) -> Self
    where
        F: FnMut(JsValue) + 'static,
    {
        MogwaiCallback {
            callback: Rc::new(Box::new(f)),
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub fn new<F>(f: F) -> Self
    where
        F: FnMut(JsValue) + 'static,
    {
        MogwaiCallback {
            callback: Rc::new(Closure::wrap(Box::new(f))),
        }
    }
}


#[derive(Clone)]
pub(crate) struct ServerNode {
    pub(crate) name_or_text: NameOrText,
    pub(crate) attributes: Vec<(String, Rc<RefCell<Option<String>>>)>,
    pub(crate) styles: Vec<(String, Rc<RefCell<String>>)>,
}


/// A widget that may contain a bundled network of html elements, callback
/// closures and receivers. This wraps a Javascript DOM node and maintains lists
/// and maps needed to orchestrate user interaction.
pub struct View<T: JsCast> {
    pub children: Vec<View<Node>>,

    pub(crate) phantom: PhantomData<T>,
    pub(crate) element: Rc<JsValue>,
    pub(crate) callbacks: HashMap<String, MogwaiCallback>,
    pub(crate) window_callbacks: HashMap<String, MogwaiCallback>,
    pub(crate) document_callbacks: HashMap<String, MogwaiCallback>,
    pub(crate) string_rxs: Vec<Receiver<String>>,
    pub(crate) opt_string_rxs: Vec<Receiver<Option<String>>>,
    pub(crate) bool_rxs: Vec<Receiver<bool>>,

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) server_node: ServerNode,
}


impl<T: JsCast> View<T> {
    #[cfg(target_arch = "wasm32")]
    fn with_node<F>(&mut self, _: F)
    where
        F: FnOnce(&mut ServerNode),
    {
        // Do nothing!
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn with_node<F>(&mut self, f: F)
    where
        F: FnOnce(&mut ServerNode),
    {
        f(&mut self.server_node);
    }

    #[cfg(target_arch = "wasm32")]
    pub fn to_ssr_node(self) -> SsrNode {
        panic!("View::to_ssr_node is only available outside of wasm32")
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn to_ssr_node(self) -> SsrNode {
        let View {
            server_node,
            children,
            ..
        } = self;
        match server_node.name_or_text {
            NameOrText::Name(name) => SsrNode::Container {
                name: name.borrow().clone(),
                attributes: {
                    // TODO: Merge attribute style with explicit styles
                    let mut attributes = server_node
                        .attributes
                        .into_iter()
                        .map(|(k, var)| (k, var.borrow().clone()))
                        .collect::<Vec<_>>();
                    if !server_node.styles.is_empty() {
                        attributes.push((
                            "style".into(),
                            Some(
                                server_node
                                    .styles
                                    .into_iter()
                                    .map(|(k, v)| format!("{}: {};", k, v.borrow()))
                                    .collect::<Vec<_>>()
                                    .join(" "),
                            ),
                        ));
                    }
                    attributes
                },
                children: children.into_iter().map(|g| g.to_ssr_node()).collect(),
            },
            NameOrText::Text(text) => SsrNode::Text(text.borrow().clone()),
        }
    }

    /// Adds a View as a child node.
    pub fn add_child<E: JsCast + AsRef<Node> + Clone>(&mut self, child: View<E>) {
        if cfg!(target_arch = "wasm32") {
            let node: &Node = self.as_ref().unchecked_ref();
            node.append_child(&child.element.as_ref().unchecked_ref())
                .expect("Could not add text node to View");
        }
        self.children.push(child.upcast());
    }

    pub fn into_html_string(self) -> String {
        if cfg!(target_arch = "wasm32") {
            let t: &JsValue = self.element.as_ref();

            if let Some(element) = t.dyn_ref::<Element>() {
                return element.outer_html();
            }

            if let Some(text) = t.dyn_ref::<Text>() {
                return text.data();
            }
            panic!("Dom reference {:#?} could not be turned into a string", t);
        } else {
            let node = self.to_ssr_node();
            return String::from(node);
        }
    }
}


impl View<Text> {
    #[cfg(target_arch = "wasm32")]
    pub fn text(text: &str) -> View<Text> {
        View::wrapping(Text::new_with_data(text).expect("could not create text"))
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn text(text: &str) -> View<Text> {
        View {
            element: Rc::new(JsValue::NULL),
            phantom: std::marker::PhantomData,
            server_node: ServerNode {
                name_or_text: NameOrText::Text(Rc::new(RefCell::new(
                    text.replace("&", "&amp;")
                        .replace("<", "&lt;")
                        .replace(">", "&gt;")
                        .into(),
                ))),
                attributes: vec![],
                styles: vec![],
            },
            children: vec![],

            callbacks: HashMap::default(),
            window_callbacks: HashMap::default(),
            document_callbacks: HashMap::default(),

            string_rxs: vec![],
            opt_string_rxs: vec![],
            bool_rxs: vec![],
        }
    }

    pub fn rx_text(&mut self, rx: Receiver<String>) {
        self.string_rxs.push(crate::txrx::hand_clone(&rx));
        if cfg!(target_arch = "wasm32") {
            let text: Text = (self.as_ref() as &Text).clone();
            rx.respond(move |s| text.set_data(s));
        } else {
            self.with_node(|node| {
                let text = node.name_or_text.clone();
                rx.respond(move |s| match &text {
                    NameOrText::Text(var) => {
                        *var.borrow_mut() = s.into();
                    }
                    _ => {}
                })
            });
        }
    }
}


impl<T: JsCast + Clone> Clone for View<T> {
    fn clone(&self) -> Self {
        self.clone_as()
    }
}


impl<T, S> AsRef<S> for View<T>
where
    T: JsCast + AsRef<S>,
    S: JsCast,
{
    fn as_ref(&self) -> &S {
        self.element.unchecked_ref::<S>()
    }
}


impl<T: JsCast> Deref for View<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.element.unchecked_ref::<T>()
    }
}


/// # From instances for [`View`]
///
/// * String, str etc get converted into [`View<Text>`] - text nodes,
///   with their initial inner text set to the input string.
/// * Receiver<String> get converted into [`View<Text>`] with their
///   inner text set by the receiver.
/// * Effect<String> gets converted into [`View<Text>`] with possibly
///   an initial string and updates through the receiver.
/// * [`Gizmo<C>`] returns its view, a [`View<<C::as Component>::DomNode>`].


impl From<Effect<String>> for View<Text> {
    fn from(eff: Effect<String>) -> Self {
        let (may_now, may_later) = eff.into_some();
        let now = may_now.unwrap_or("".into());
        let mut text = View::text(&now);
        if let Some(rx) = may_later {
            text.rx_text(rx);
        }
        text
    }
}


impl From<(&str, Receiver<String>)> for View<Text> {
    fn from(tuple: (&str, Receiver<String>)) -> Self {
        let eff: Effect<String> = tuple.into();
        eff.into()
    }
}


impl From<(String, Receiver<String>)> for View<Text> {
    fn from(tuple: (String, Receiver<String>)) -> Self {
        let eff: Effect<String> = tuple.into();
        eff.into()
    }
}


impl From<(&String, Receiver<String>)> for View<Text> {
    fn from((now, later): (&String, Receiver<String>)) -> Self {
        let tuple = (now.clone(), later);
        let eff: Effect<String> = tuple.into();
        eff.into()
    }
}


impl<'a> From<&'a str> for View<Text> {
    fn from(s: &'a str) -> Self {
        View::text(s)
    }
}


impl From<&String> for View<Text> {
    fn from(s: &String) -> Self {
        View::text(s)
    }
}


impl From<String> for View<Text> {
    fn from(s: String) -> Self {
        View::text(&s)
    }
}


impl<T> From<Gizmo<T>> for View<<T as Component>::DomNode>
where
    T: Component,
    <T as Component>::DomNode: JsCast + AsRef<Node>,
{
    fn from(gizmo: Gizmo<T>) -> Self {
        gizmo.view
    }
}


/// # ElementView


impl<T: JsCast> ElementView for View<T> {
    #[cfg(not(target_arch = "wasm32"))]
    fn element(name: &str) -> Self {
        View {
            element: Rc::new(JsValue::NULL),
            phantom: std::marker::PhantomData,
            server_node: ServerNode {
                name_or_text: NameOrText::Name(Rc::new(RefCell::new(name.into()))),
                attributes: vec![],
                styles: vec![],
            },
            children: vec![],

            callbacks: HashMap::default(),
            window_callbacks: HashMap::default(),
            document_callbacks: HashMap::default(),

            string_rxs: vec![],
            opt_string_rxs: vec![],
            bool_rxs: vec![],
        }
    }
    #[cfg(target_arch = "wasm32")]
    fn element(name: &str) -> Self {
        let name = name.into();
        let el = utils::document()
            .create_element(name)
            .expect(&format!("cannot create element {:?}", name))
            .unchecked_into();
        View::wrapping(el)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn element_ns(tag: &str, ns: &str) -> Self {
        View {
            element: Rc::new(JsValue::NULL),
            phantom: std::marker::PhantomData,
            server_node: ServerNode {
                name_or_text: NameOrText::Name(Rc::new(RefCell::new(tag.into()))),
                attributes: vec![("xmlns".into(), Rc::new(RefCell::new(Some(ns.into()))))],
                styles: vec![],
            },
            children: vec![],

            callbacks: HashMap::default(),
            window_callbacks: HashMap::default(),
            document_callbacks: HashMap::default(),

            string_rxs: vec![],
            opt_string_rxs: vec![],
            bool_rxs: vec![],
        }
    }
    #[cfg(target_arch = "wasm32")]
    fn element_ns(tag: &str, ns: &str) -> Self {
        let el = utils::document()
            .create_element_ns(Some(ns), tag)
            .expect(&format!("cannot create element_ns '{}' '{}'", tag, ns))
            .unchecked_into();
        View::wrapping(el)
    }
}


impl<T: JsCast + AsRef<EventTarget>> EventTargetView for View<T> {
    fn on(mut self, ev_name: &str, tx: Transmitter<Event>) -> View<T> {
        if cfg!(target_arch = "wasm32") {
            let target: &EventTarget = self.as_ref();
            let cb = utils::add_event(ev_name, target, tx);
            self.callbacks.insert(ev_name.to_string(), cb);
            self
        } else {
            self
        }
    }

    fn window_on(mut self, ev_name: &str, tx: Transmitter<Event>) -> View<T> {
        if cfg!(target_arch = "wasm32") {
            let window = utils::window();
            let target: &EventTarget = window.as_ref();
            let cb = utils::add_event(ev_name, &target, tx);
            self.window_callbacks.insert(ev_name.to_string(), cb);
            self
        } else {
            self
        }
    }

    fn document_on(mut self, ev_name: &str, tx: Transmitter<Event>) -> View<T> {
        if cfg!(target_arch = "wasm32") {
            let doc = utils::document();
            let target: &EventTarget = doc.as_ref();
            let cb = utils::add_event(ev_name, target, tx);
            self.document_callbacks.insert(ev_name.to_string(), cb);
            self
        } else {
            self
        }
    }
}


/// # AttributeView

impl<T: JsCast + AsRef<Element>> AttributeView for View<T> {
    fn attribute<E: Into<Effect<String>>>(mut self, name: &str, eff: E) -> View<T> {
        let (may_now, may_later) = eff.into().into_some();
        if cfg!(target_arch = "wasm32") {
            if let Some(now) = may_now {
                (self.as_ref() as &Element)
                    .set_attribute(name, &now)
                    .unwrap_throw();
            }
            if let Some(later) = may_later {
                let rx = later;

                // Save a clone so we can drop_responder if this gizmo goes out of scope
                self.string_rxs.push(crate::txrx::hand_clone(&rx));

                let element: &Element = self.as_ref();
                let element = element.clone();
                let name = name.to_string();

                rx.respond(move |s| {
                    element
                        .set_attribute(&name, s)
                        .expect("Could not set attribute");
                });
            }
        } else {
            let var = Rc::new(RefCell::new(None));
            self.with_node(|node| {
                node.attributes.push((name.into(), var.clone()));
            });
            if let Some(now) = may_now {
                *var.borrow_mut() = Some(now.to_string());
            }
            if let Some(later) = may_later {
                let rx = later.branch_map(|s| Some(s.to_string()));

                // Save a clone so we can drop_responder if this gizmo goes out of scope
                self.opt_string_rxs.push(crate::txrx::hand_clone(&rx));

                rx.respond(move |may_s: &Option<String>| {
                    *var.borrow_mut() = may_s.clone();
                });
            }
        }

        self
    }

    fn boolean_attribute<E: Into<Effect<bool>>>(mut self, name: &str, eff: E) -> View<T> {
        let (may_now, may_later) = eff.into().into_some();
        if cfg!(target_arch = "wasm32") {
            if let Some(true) = may_now {
                (self.as_ref() as &Element)
                    .set_attribute(name, "")
                    .unwrap_throw();
            }
            if let Some(later) = may_later {
                let rx = later.branch();
                // Save a clone so we can drop_responder if this gizmo goes out of scope
                self.bool_rxs.push(crate::txrx::hand_clone(&rx));

                let element: &Element = self.as_ref();
                let element = element.clone();
                let name = name.to_string();

                rx.respond(move |b| {
                    if *b {
                        element
                            .set_attribute(&name, "")
                            .expect("Could not set boolean attribute");
                    } else {
                        element
                            .remove_attribute(&name)
                            .expect("Could not remove boolean attribute")
                    }
                });
            }
        } else {
            let var = Rc::new(RefCell::new(None));
            self.with_node(|node| {
                node.attributes.push((name.into(), var.clone()));
            });
            if let Some(true) = may_now {
                *var.borrow_mut() = Some("".to_string());
            }
            if let Some(later) = may_later {
                let rx = later.branch();
                // Save a clone so we can drop_responder if this gizmo goes out of scope
                self.bool_rxs.push(crate::txrx::hand_clone(&rx));

                rx.respond(move |is_present| {
                    *var.borrow_mut() = if *is_present {
                        Some("".to_string())
                    } else {
                        None
                    };
                });
            }
        }

        self
    }
}


/// # ParentView


impl<S: JsCast + AsRef<Node> + Clone, T: JsCast + AsRef<Node>> ParentView<View<S>> for View<T> {
    fn with(mut self, view: View<S>) -> Self {
        self.add_child(view);
        self
    }
}


/// # StyleView


impl<T: JsCast + AsRef<HtmlElement>> StyleView for View<T> {
    fn style<E: Into<Effect<String>>>(mut self, name: &str, eff: E) -> Self {
        let (may_now, may_later) = eff.into().into_some();
        if cfg!(target_arch = "wasm32") {
            if let Some(now) = may_now {
                (self.as_ref() as &HtmlElement)
                    .style()
                    .set_property(name, now.as_str())
                    .unwrap_throw();
            }
            if let Some(later) = may_later {
                let rx = later;
                // Save a clone so we can drop_responder if this gizmo goes out of scope
                self.string_rxs.push(crate::txrx::hand_clone(&rx));

                let element: &HtmlElement = self.as_ref();
                let style = element.style();
                let name = name.to_string();

                rx.respond(move |s| {
                    style.set_property(&name, s).expect("Could not set style");
                });
            }
        } else {
            let var = Rc::new(RefCell::new("".to_string()));
            if let Some(now) = may_now {
                *var.borrow_mut() = now.to_string();
                self.with_node(|node| {
                    node.styles.push((name.into(), var.clone()));
                });
            }
            if let Some(later) = may_later {
                let rx = later;

                // Save a clone so we can drop_responder if this gizmo goes out of scope
                self.string_rxs.push(crate::txrx::hand_clone(&rx));

                rx.respond(move |s| {
                    *var.borrow_mut() = s.to_string();
                })
            }
        }

        self
    }
}


/// # PostBuildView

impl<T: JsCast + Clone + 'static> PostBuildView for View<T> {
    type DomNode = T;

    fn post_build(self, tx: Transmitter<T>) -> Self {
        let t: &T = self.element.unchecked_ref();
        let t: T = t.clone();
        tx.send_async(async move { t });
        self
    }
}


impl<T: JsCast> View<T> {
    /// Create a new `View` wrapping a `T` that can be dereferenced to a `Node`.
    ///
    /// # Panics
    /// Panics if used outside of a wasm32 target.
    #[cfg(target_arch = "wasm32")]
    pub fn wrapping(element: T) -> View<T> {
        View {
            element: Rc::new(element.into()),
            phantom: PhantomData,
            callbacks: HashMap::new(),
            window_callbacks: HashMap::new(),
            document_callbacks: HashMap::new(),
            string_rxs: vec![],
            opt_string_rxs: vec![],
            bool_rxs: vec![],
            children: vec![],
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn wrapping(_element: T) -> View<T> {
        panic!("View::wrapping is only available on wasm32")
    }
}


impl<T: JsCast + Clone> View<T> {
    /// Creates a new gizmo with data cloned from the first.
    #[cfg(target_arch = "wasm32")]
    fn clone_as<D>(&self) -> View<D>
    where
        D: JsCast,
    {
        View {
            phantom: PhantomData,
            element: self.element.clone(),

            callbacks: self.callbacks.clone(),
            window_callbacks: self.window_callbacks.clone(),
            document_callbacks: self.document_callbacks.clone(),
            string_rxs: self
                .string_rxs
                .iter()
                .map(|rx| crate::txrx::hand_clone(rx))
                .collect(),
            opt_string_rxs: self
                .opt_string_rxs
                .iter()
                .map(|rx| crate::txrx::hand_clone(rx))
                .collect(),
            bool_rxs: self
                .bool_rxs
                .iter()
                .map(|rx| crate::txrx::hand_clone(rx))
                .collect(),

            children: self.children.clone(),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn clone_as<D>(&self) -> View<D>
    where
        D: JsCast,
    {
        View {
            phantom: PhantomData,
            element: self.element.clone(),

            callbacks: self.callbacks.clone(),
            window_callbacks: self.window_callbacks.clone(),
            document_callbacks: self.document_callbacks.clone(),
            string_rxs: self
                .string_rxs
                .iter()
                .map(|rx| crate::txrx::hand_clone(rx))
                .collect(),
            opt_string_rxs: self
                .opt_string_rxs
                .iter()
                .map(|rx| crate::txrx::hand_clone(rx))
                .collect(),
            bool_rxs: self
                .bool_rxs
                .iter()
                .map(|rx| crate::txrx::hand_clone(rx))
                .collect(),

            server_node: self.server_node.clone(),
            children: self.children.clone(),
        }
    }

    /// Cast the given View to contain the inner DOM node of another type.
    /// That type must be dereferencable from the given View.
    pub fn upcast<D>(self) -> View<D>
    where
        T: AsRef<D>,
        D: JsCast,
    {
        self.clone_as()
    }

    /// Attempt to downcast the inner element.
    ///
    /// # Note
    /// On wasm32 this performs a check on the inner element to determine if the
    /// element can be downcast to the desired type. On other compilation targets
    /// this function always returns Ok.
    #[cfg(target_arch = "wasm32")]
    pub fn downcast<To: JsCast + AsRef<Node>>(self) -> Result<View<To>, View<T>> {
        if self.element.has_type::<To>() {
            Ok(self.clone_as())
        } else {
            Err(self)
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn downcast<To: JsCast + AsRef<Node>>(self) -> Result<View<To>, View<T>> {
        Ok(self.clone_as())
    }
}


impl<T: JsCast + 'static> View<T> {
    /// Run this gizmo forever without appending it to anything.
    pub fn forget(self) -> Result<(), JsValue> {
        if cfg!(target_arch = "wasm32") {
            let gizmo_var = RefCell::new(self);
            utils::timeout(1000, move || {
                gizmo_var.borrow_mut();
                true
            });
            Ok(())
        } else {
            Err("forgetting and running a gizmo is only supported on wasm".into())
        }
    }
}


impl<T: JsCast + AsRef<Node> + Clone + 'static> View<T> {
    /// Run this gizmo in a parent container forever, never dropping it.
    pub fn run_in_container(self, container: &Node) -> Result<(), JsValue> {
        if cfg!(target_arch = "wasm32") {
            let _ = container.append_child(&self.as_ref());
            self.forget()
        } else {
            Err("running gizmos is only supported on wasm".into())
        }
    }

    /// Run this gizmo in the document body forever, never dropping it.
    pub fn run(self) -> Result<(), JsValue> {
        if cfg!(target_arch = "wasm32") {
            self.run_in_container(&utils::body())
        } else {
            Err("running gizmos is only supported on wasm".into())
        }
    }
}


/// View's Drop implementation insures that responders no longer attempt to
/// update the gizmo. It also removes its element from the DOM.
#[cfg(target_arch = "wasm32")]
impl<T: JsCast> Drop for View<T> {
    fn drop(&mut self) {
        let count = Rc::strong_count(&self.element);
        let node = self.element.unchecked_ref::<Node>().clone();
        if count <= 1 {
            if let Some(parent) = node.parent_node() {
                let _ = parent.remove_child(&node);
            }
            self.string_rxs
                .iter_mut()
                .for_each(|rx| rx.drop_responder());
            self.opt_string_rxs
                .iter_mut()
                .for_each(|rx| rx.drop_responder());
            self.bool_rxs.iter_mut().for_each(|rx| rx.drop_responder());
        }
    }
}


#[cfg(test)]
#[allow(unused_braces)]
mod gizmo_tests {
    #[allow(unused_braces)]
    use super::{super::super::prelude::*};
    use crate as mogwai;
    use mogwai_html_macro::target_arch_is_wasm32;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_test::*;
    use web_sys::Element;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn this_arch_is_wasm32() {
        assert!(target_arch_is_wasm32! {});
    }

    #[wasm_bindgen_test]
    fn gizmo_ref_as_child() {
        // Since the pre tag is dropped after the scope block the last assert should
        // show that the div tag has no children.
        let div = {
            let pre = view! { <pre>"this has text"</pre> };
            let div = view! { <div id="parent"></div> };
            (div.as_ref() as &Node).append_child(pre.as_ref()).unwrap();
            assert!(
                div.first_child().is_some(),
                "parent does not contain in-scope child"
            );
            //console::log_1(&"dropping pre".into());
            div
        };
        assert!(
            div.first_child().is_none(),
            "parent does not maintain out-of-scope child"
        );
        //console::log_1(&"dropping parent".into());
    }

    #[wasm_bindgen_test]
    fn gizmo_as_child() {
        // Since the pre tag is *not* dropped after the scope block the last assert
        // should show that the div tag has a child.
        let div = {
            let div = view! {
                <div id="parent-div">
                    <pre>"some text"</pre>
                </div>
            };
            assert!(div.first_child().is_some(), "could not add child gizmo");
            div
        };
        assert!(
            div.first_child().is_some(),
            "could not keep hold of child gizmo"
        );
        assert_eq!(div.children.len(), 1, "parent is missing static_gizmo");
        //console::log_1(&"dropping div and pre".into());
    }

    #[wasm_bindgen_test]
    fn gizmo_tree() {
        let root = view! {
            <div id="root">
                <div id="branch">
                    <div id="leaf">
                        "leaf"
                    </div>
                </div>
            </div>
        };
        if let Some(branch) = root.first_child() {
            if let Some(leaf) = branch.first_child() {
                if let Some(leaf) = leaf.dyn_ref::<Element>() {
                    assert_eq!(leaf.id(), "leaf");
                } else {
                    panic!("leaf is not an Element");
                }
            } else {
                panic!("branch has no leaf");
            }
        } else {
            panic!("root has no branch");
        }
    }

    #[wasm_bindgen_test]
    fn gizmo_texts() {
        let div = view! {
            <div>
                "here is some text "
                // i can use comments, yay!
                {&format!("{}", 66)}
                " <- number"
            </div>
        };
        assert_eq!(
            &div.outer_html(),
            "<div>here is some text 66 &lt;- number</div>"
        );
    }

    #[wasm_bindgen_test]
    fn rx_attribute_jsx() {
        let (tx, rx) = txrx::<String>();
        let div = view! {
            <div class=("now", rx) />
        };
        let div_el: &HtmlElement = div.as_ref();
        assert_eq!(div_el.outer_html(), r#"<div class="now"></div>"#);

        tx.send(&"later".to_string());
        assert_eq!(div_el.outer_html(), r#"<div class="later"></div>"#);
    }

    #[wasm_bindgen_test]
    fn rx_style_plain() {
        let (tx, rx) = txrx::<String>();
        let div = (View::element("div") as View<HtmlElement>).style("display", ("block", rx));
        let div_el: &HtmlElement = div.as_ref();
        assert_eq!(
            div_el.outer_html(),
            r#"<div style="display: block;"></div>"#
        );

        tx.send(&"none".to_string());
        assert_eq!(div_el.outer_html(), r#"<div style="display: none;"></div>"#);
    }

    #[wasm_bindgen_test]
    fn rx_style_jsx() {
        let (tx, rx) = txrx::<String>();
        let div = view! { <div style:display=("block", rx) /> };
        let div_el: &HtmlElement = div.as_ref();
        assert_eq!(
            div_el.outer_html(),
            r#"<div style="display: block;"></div>"#
        );

        tx.send(&"none".to_string());
        assert_eq!(div_el.outer_html(), r#"<div style="display: none;"></div>"#);
    }

    #[wasm_bindgen_test]
    fn rx_text() {
        let (tx, rx) = txrx();
        let div = (View::element("div") as View<HtmlElement>).with(("initial", rx).into());
        let el: &HtmlElement = div.as_ref();
        assert_eq!(el.inner_text().as_str(), "initial");
        tx.send(&"after".into());
        assert_eq!(el.inner_text(), "after");
    }

    #[wasm_bindgen_test]
    fn tx_on_click_plain() {
        let (tx, rx) = txrx_fold(0, |n: &mut i32, _: &Event| -> String {
            *n += 1;
            if *n == 1 {
                "Clicked 1 time".to_string()
            } else {
                format!("Clicked {} times", *n)
            }
        });

        let button = (View::element("button") as View<HtmlElement>)
            .with(("Clicked 0 times", rx).into())
            .on("click", tx);
        let el: &HtmlElement = button.as_ref();

        assert_eq!(el.inner_html(), "Clicked 0 times");
        el.click();
        assert_eq!(el.inner_html(), "Clicked 1 time");
    }

    #[wasm_bindgen_test]
    fn tx_on_click_jsx() {
        let (tx, rx) = txrx_fold(0, |n: &mut i32, _: &Event| -> String {
            *n += 1;
            if *n == 1 {
                "Clicked 1 time".to_string()
            } else {
                format!("Clicked {} times", *n)
            }
        });

        let button = view! { <button on:click=tx>{("Clicked 0 times", rx)}</button> };
        let el: &HtmlElement = button.as_ref();

        assert_eq!(el.inner_html(), "Clicked 0 times");
        el.click();
        assert_eq!(el.inner_html(), "Clicked 1 time");
    }


    #[wasm_bindgen_test]
    fn tx_window_on_click_jsx() {
        let (tx, rx) = txrx();
        let _button = view! {
            <button window:load=tx>
            {(
                "Waiting...",
                rx.branch_map(|_| "Loaded!".into())
            )}
            </button>
        };
    }

    //fn nice_compiler_error() {
    //    let _div = view! {
    //        <div unknown:colon:thing="not ok" />
    //    };
    //}

    #[test]
    #[wasm_bindgen_test]
    fn can_i_alter_views_on_the_server() {
        let (tx_text, rx_text) = txrx::<String>();
        let (tx_style, rx_style) = txrx::<String>();
        let (tx_class, rx_class) = txrx::<String>();
        let view = view! {
            <div style:float=("left", rx_style)><p class=("p_class", rx_class)>{("here", rx_text)}</p></div>
        };
        assert_eq!(
            &view.clone().into_html_string(),
            r#"<div style="float: left;"><p class="p_class">here</p></div>"#
        );

        tx_text.send(&"there".to_string());
        assert_eq!(
            &view.clone().into_html_string(),
            r#"<div style="float: left;"><p class="p_class">there</p></div>"#
        );

        tx_style.send(&"right".to_string());
        assert_eq!(
            &view.clone().into_html_string(),
            r#"<div style="float: right;"><p class="p_class">there</p></div>"#
        );

        tx_class.send(&"my_p_class".to_string());
        assert_eq!(
            &view.clone().into_html_string(),
            r#"<div style="float: right;"><p class="my_p_class">there</p></div>"#
        );
    }


    #[wasm_bindgen_test]
    fn can_hydrate_view() {
        let original_view = view! {
            <div id="my_div">
                <p class="class">"inner text"</p>
            </div>
        };
        let original_el: HtmlElement = (original_view.as_ref() as &HtmlElement).clone();
        original_view.run().unwrap();

        let (tx_class, rx_class) = txrx::<String>();
        let (tx_text, rx_text) = txrx::<String>();
        let hydrated_view = View::try_from(hydrate! {
            <div id="my_div">
                <p class=("unused_class", rx_class)>{("unused inner text", rx_text)}</p>
            </div>
        })
        .unwrap();

        hydrated_view.forget().unwrap();

        tx_class.send(&"new_class".to_string());
        tx_text.send(&"different inner text".to_string());

        assert_eq!(
            original_el.outer_html().as_str(),
            r#"<div id="my_div"><p class="new_class">different inner text</p></div>"#
        );
    }


    #[wasm_bindgen_test]
    fn can_hydrate_or_view() {
        let (tx_class, rx_class) = txrx::<String>();
        let (tx_text, rx_text) = txrx::<String>();
        let count = txrx::new_shared(0 as u32);
        let (tx_pb, rx_pb) =
            txrx_fold_shared(count.clone(), |count: &mut u32, _: &HtmlElement| -> () {
                *count += 1;
                ()
            });

        rx_pb.respond(|_| println!("post build"));

        let fresh_view = || {
            view! {
                <div id="my_div" post:build=(&tx_pb).clone()>
                    <p class=("class", rx_class.branch())>{("inner text", rx_text.branch())}</p>
                </div>
            }
        };
        let hydrate_view = || {
            View::try_from(hydrate! {
                <div id="my_div" post:build=(&tx_pb).clone()>
                    <p class=("class", rx_class.branch())>{("inner text", rx_text.branch())}</p>
                </div>
            })
        };

        let view = fresh_view();

        let original_el: HtmlElement = (view.as_ref() as &HtmlElement).clone();
        view.run().unwrap();

        let hydrated_view = hydrate_view().unwrap();
        hydrated_view.forget().unwrap();

        tx_class.send(&"new_class".to_string());
        tx_text.send(&"different inner text".to_string());

        assert_eq!(
            original_el.outer_html().as_str(),
            r#"<div id="my_div"><p class="new_class">different inner text</p></div>"#
        );

        assert_eq!(*count.borrow(), 2);
    }
}
