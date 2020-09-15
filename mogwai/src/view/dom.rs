//! Widgets for the browser.
use std::{cell::RefCell, collections::HashMap, marker::PhantomData, ops::Deref, rc::Rc};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
pub use web_sys::{Element, Event, EventTarget, HtmlElement, HtmlInputElement};
use web_sys::{Node, Text};

#[cfg(not(target_arch = "wasm32"))]
use crate::ssr::Node as SsrNode;
use crate::{
    prelude::{Component, Effect, Gizmo, IsDomNode, Receiver, Transmitter},
    utils,
    view::interface::*,
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
pub struct View<T: IsDomNode> {
    pub children: Vec<View<Node>>,

    pub(crate) phantom: PhantomData<*const T>,
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


impl<T: IsDomNode> Default for View<T> {
    #[cfg(target_arch = "wasm32")]
    fn default() -> Self {
        View {
            phantom: PhantomData,
            element: Rc::new(JsValue::NULL),
            children: vec![],
            callbacks: HashMap::new(),
            window_callbacks: HashMap::new(),
            document_callbacks: HashMap::new(),
            string_rxs: vec![],
            opt_string_rxs: vec![],
            bool_rxs: vec![],
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn default() -> Self {
        View {
            phantom: PhantomData,
            element: Rc::new(JsValue::NULL),
            children: vec![],
            callbacks: HashMap::new(),
            window_callbacks: HashMap::new(),
            document_callbacks: HashMap::new(),
            string_rxs: vec![],
            opt_string_rxs: vec![],
            bool_rxs: vec![],
            server_node: ServerNode {
                name_or_text: NameOrText::Name(Rc::new(RefCell::new("".to_string()))),
                attributes: vec![],
                styles: vec![],
            },
        }
    }
}


impl<T: IsDomNode> View<T> {
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
    pub fn add_child<E: JsCast + AsRef<Node> + Clone + 'static>(&mut self, child: View<E>) {
        if cfg!(target_arch = "wasm32") {
            let node: &Node = self.as_ref().unchecked_ref();
            node.append_child(&child.element.as_ref().unchecked_ref())
                .expect("Could not add text node to View");
        }
        self.children.push(child.upcast());
    }

    /// Adds an event that will be stored by this view.
    pub fn add_event(&mut self, target: &EventTarget, ev_name: &str, tx: Transmitter<Event>) {
        let cb = utils::add_event(ev_name, target, tx);
        self.callbacks.insert(ev_name.to_string(), cb);
    }

    /// Attempt to add a style to this view. If the underlying DOM type `T` does not
    /// implement [`AsRef<HtmlElement>`] then this function is a noop.
    pub fn add_style(&mut self, name: &str, effect: Effect<String>) {
        let (may_now, may_later) = effect.into();
        if cfg!(target_arch = "wasm32") {
            let t: T = {
                let t: &T = &self;
                t.clone()
            };
            if let Ok(element) = t.dyn_into::<HtmlElement>() {
                if let Some(now) = may_now {
                    element
                        .style()
                        .set_property(name, now.as_str())
                        .unwrap_throw();
                }
                if let Some(later) = may_later {
                    let rx = later;
                    // Save a clone so we can drop_responder if this gizmo goes out of scope
                    self.string_rxs.push(crate::txrx::hand_clone(&rx));

                    let style = element.style();
                    let name = name.to_string();

                    rx.respond(move |s| {
                        style.set_property(&name, s).expect("Could not set style");
                    });
                }
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
    }

    /// Attempts to add an attribute to this view. If the underlying DOM type `T` does not
    /// implement [`AsRef<Element>`] then this function is a noop.
    pub fn add_attribute(&mut self, name: &str, effect: Effect<String>) {
        let (may_now, may_later) = effect.into();
        if cfg!(target_arch = "wasm32") {
            let t: &T = self.element.unchecked_ref();
            if let Some(element) = t.dyn_ref::<Element>() {
                if let Some(now) = may_now {
                    element.set_attribute(name, &now).unwrap_throw();
                }
                if let Some(later) = may_later {
                    let rx = later;

                    // Save a clone so we can drop_responder if this gizmo goes out of scope
                    self.string_rxs.push(crate::txrx::hand_clone(&rx));

                    let element = element.clone();
                    let name = name.to_string();

                    rx.respond(move |s| {
                        element
                            .set_attribute(&name, s)
                            .expect("Could not set attribute");
                    });
                }
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
    }

    /// Attempts to add a boolean attribute to this view. If the underlying DOM type `T`
    /// does not implement [`AsRef<Element>`] then this function is a noop.
    pub fn add_boolean_attribute(&mut self, name: &str, effect: Effect<bool>) {
        let (may_now, may_later) = effect.into();
        if cfg!(target_arch = "wasm32") {
            let t: &T = self.element.unchecked_ref();
            if let Some(element) = t.dyn_ref::<Element>() {
                if let Some(true) = may_now {
                    element.set_attribute(name, "").unwrap_throw();
                }
                if let Some(later) = may_later {
                    let rx = later.branch();
                    // Save a clone so we can drop_responder if this gizmo goes out of scope
                    self.bool_rxs.push(crate::txrx::hand_clone(&rx));

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
    }

    #[cfg(target_arch = "wasm32")]
    pub fn into_html_string(self) -> String {
        let t: &JsValue = self.element.as_ref();

        if let Some(element) = t.dyn_ref::<Element>() {
            return element.outer_html();
        }

        if let Some(text) = t.dyn_ref::<Text>() {
            return text.data();
        }
        panic!("Dom reference {:#?} could not be turned into a string", t);
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn into_html_string(self) -> String {
        String::from(self.to_ssr_node())
    }

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

    /// Swap all data with another view.
    /// BEWARE: This function is for internal library use and for the use of
    /// helper libraries. Using this function without care may result in
    /// JavaScript errors.
    #[cfg(target_arch = "wasm32")]
    pub fn swap<To: IsDomNode>(&mut self, other: &mut View<To>) {
        std::mem::swap(&mut self.element, &mut other.element);
        std::mem::swap(&mut self.callbacks, &mut other.callbacks);
        std::mem::swap(&mut self.window_callbacks, &mut other.window_callbacks);
        std::mem::swap(&mut self.document_callbacks, &mut other.document_callbacks);
        std::mem::swap(&mut self.string_rxs, &mut other.string_rxs);
        std::mem::swap(&mut self.opt_string_rxs, &mut other.opt_string_rxs);
        std::mem::swap(&mut self.bool_rxs, &mut other.bool_rxs);
        std::mem::swap(&mut self.children, &mut other.children);
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn swap<To: IsDomNode>(&mut self, other: &mut View<To>) {
        std::mem::swap(&mut self.element, &mut other.element);
        std::mem::swap(&mut self.callbacks, &mut other.callbacks);
        std::mem::swap(&mut self.window_callbacks, &mut other.window_callbacks);
        std::mem::swap(&mut self.document_callbacks, &mut other.document_callbacks);
        std::mem::swap(&mut self.string_rxs, &mut other.string_rxs);
        std::mem::swap(&mut self.opt_string_rxs, &mut other.opt_string_rxs);
        std::mem::swap(&mut self.bool_rxs, &mut other.bool_rxs);
        std::mem::swap(&mut self.children, &mut other.children);
        std::mem::swap(&mut self.server_node, &mut other.server_node);
    }

    #[cfg(target_arch = "wasm32")]
    pub fn try_cast<To: IsDomNode>(mut self) -> Result<View<To>, Self> {
        if self.element.has_type::<To>() {
            let mut view: View<To> = View::default();
            self.swap(&mut view);
            Ok(view)
        } else {
            Err(self)
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn try_cast<To: IsDomNode>(mut self) -> Result<View<To>, Self> {
        let mut view: View<To> = View::default();
        self.swap(&mut view);
        Ok(view)
    }

    /// Creates a new gizmo with data cloned from the first.
    #[cfg(target_arch = "wasm32")]
    fn clone_as<D: IsDomNode>(&self) -> View<D> {
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
    fn clone_as<D: IsDomNode>(&self) -> View<D> {
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
    pub fn upcast<To: IsDomNode>(mut self) -> View<To>
    where
        T: AsRef<To>,
        To: IsDomNode,
    {
        let mut to: View<To> = View::default();
        self.swap(&mut to);
        to
    }

    /// Attempt to downcast the inner element.
    ///
    /// # Note
    /// On wasm32 this performs a check on the inner element to determine if the
    /// element can be downcast to the desired type. On other compilation targets
    /// this function always returns Ok.
    pub fn downcast<To: IsDomNode + AsRef<Node>>(self) -> Result<View<To>, View<T>> {
        self.try_cast()
    }

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


impl<T: JsCast + Clone + 'static> Clone for View<T> {
    fn clone(&self) -> Self {
        self.clone_as()
    }
}


impl<T, S> AsRef<S> for View<T>
where
    T: IsDomNode + AsRef<S>,
    S: IsDomNode,
{
    fn as_ref(&self) -> &S {
        self.element.unchecked_ref::<S>()
    }
}


impl<T: IsDomNode> Deref for View<T> {
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
        let (may_now, may_later) = eff.into();
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


impl<T: IsDomNode> ElementView for View<T> {
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


impl<T: IsDomNode + AsRef<EventTarget>> EventTargetView for View<T> {
    fn on(&mut self, ev_name: &str, tx: Transmitter<Event>) {
        if cfg!(target_arch = "wasm32") {
            let target = (self.as_ref() as &EventTarget).clone();
            self.add_event(&target, ev_name, tx);
        }
    }

    fn window_on(&mut self, ev_name: &str, tx: Transmitter<Event>) {
        if cfg!(target_arch = "wasm32") {
            self.add_event(utils::window().as_ref(), ev_name, tx);
        }
    }

    fn document_on(&mut self, ev_name: &str, tx: Transmitter<Event>) {
        if cfg!(target_arch = "wasm32") {
            self.add_event(utils::document().as_ref(), ev_name, tx);
        }
    }
}


/// # AttributeView

impl<T: IsDomNode + AsRef<Element>> AttributeView for View<T> {
    fn attribute<E: Into<Effect<String>>>(&mut self, name: &str, eff: E) {
        let effect = eff.into();
        self.add_attribute(name, effect);
    }

    fn boolean_attribute<E: Into<Effect<bool>>>(&mut self, name: &str, eff: E) {
        let effect = eff.into();
        self.add_boolean_attribute(name, effect);
    }
}


/// # ParentView


impl<S: IsDomNode + AsRef<Node>, T: IsDomNode + AsRef<Node>> ParentView<View<S>> for View<T> {
    fn with(&mut self, view: View<S>) {
        self.add_child(view);
    }
}


/// # StyleView


impl<T: IsDomNode + AsRef<HtmlElement>> StyleView for View<T> {
    fn style<E: Into<Effect<String>>>(&mut self, name: &str, eff: E) {
        let effect = eff.into().into();
        self.add_style(name, effect);
    }
}


/// # PostBuildView

impl<T: IsDomNode> PostBuildView for View<T> {
    type DomNode = T;

    fn post_build(&mut self, tx: Transmitter<T>) {
        let t: &T = self.element.unchecked_ref();
        let t: T = t.clone();
        tx.send_async(async move { t });
    }
}


/// View's Drop implementation insures that responders no longer attempt to
/// update the gizmo. It also removes its element from the DOM.
#[cfg(target_arch = "wasm32")]
impl<T: IsDomNode> Drop for View<T> {
    fn drop(&mut self) {
        let count = Rc::strong_count(&self.element);
        if count <= 1 {
            if let Some(node) = self.element.dyn_ref::<Node>() {
                if let Some(parent) = node.parent_node() {
                    let _ = parent.remove_child(&node);
                }
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
