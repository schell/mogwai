//! Widgets for the browser.
use std::{
    cell::{Ref, RefCell, RefMut},
    marker::PhantomData,
    rc::Rc,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
pub use web_sys::{Element, Event, EventTarget, HtmlElement, HtmlInputElement};
use web_sys::{Node, Text};

#[cfg(not(target_arch = "wasm32"))]
use crate::ssr::Node as SsrNode;
use crate::{
    prelude::{Component, Effect, Gizmo, IsDomNode, Patch, Receiver, Transmitter},
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


enum StoredRx {
    String(Receiver<String>),
    OptionString(Receiver<Option<String>>),
    Bool(Receiver<bool>),
    View(Receiver<View<Node>>),
    Patch(Receiver<Patch<View<Node>>>),
}


impl StoredRx {
    fn drop_responder(&self) {
        match self {
            StoredRx::String(rx) => rx.drop_responder(),
            StoredRx::OptionString(rx) => rx.drop_responder(),
            StoredRx::Bool(rx) => rx.drop_responder(),
            StoredRx::View(rx) => rx.drop_responder(),
            StoredRx::Patch(rx) => rx.drop_responder(),
        }
    }
}


impl Clone for StoredRx {
    fn clone(&self) -> Self {
        match self {
            StoredRx::String(rx) => StoredRx::String(crate::txrx::hand_clone(rx)),
            StoredRx::OptionString(rx) => StoredRx::OptionString(crate::txrx::hand_clone(rx)),
            StoredRx::Bool(rx) => StoredRx::Bool(crate::txrx::hand_clone(rx)),
            StoredRx::View(rx) => StoredRx::View(crate::txrx::hand_clone(rx)),
            StoredRx::Patch(rx) => StoredRx::Patch(crate::txrx::hand_clone(rx)),
        }
    }
}


#[derive(Clone)]
enum StoredCb {
    This(String, MogwaiCallback),
    Window(String, MogwaiCallback),
    Document(String, MogwaiCallback),
}


#[derive(Clone)]
pub(crate) struct ViewInternals {
    pub(crate) slots: Vec<View<Node>>,
    pub(crate) element: Rc<JsValue>,
    callbacks: Vec<StoredCb>,
    rxs: Vec<StoredRx>,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) server_node: ServerNode,
}


/// ViewInternal's Drop implementation insures that responders no longer attempt to
/// update the gizmo. It also removes its element from the DOM.
#[cfg(target_arch = "wasm32")]
impl Drop for ViewInternals {
    fn drop(&mut self) {
        let count = Rc::strong_count(&self.element);
        if count <= 1 {
            if let Some(el) = self.element.dyn_ref::<HtmlElement>() {
                log::trace!("dropping {}", el.outer_html());
            }

            if let Some(node) = self.element.dyn_ref::<Node>() {
                if let Some(parent) = node.parent_node() {
                    let _ = parent.remove_child(&node);
                }
            }
            // TODO: Investigate whether we can drop rx responders before the element Rc
            // count drops to 1
            self.rxs.iter().for_each(|rx| rx.drop_responder());
            // TODO: Remove callbacks on drop
        }
    }
}


impl Default for ViewInternals {
    #[cfg(target_arch = "wasm32")]
    fn default() -> Self {
        ViewInternals {
            element: Rc::new(JsValue::NULL),
            callbacks: vec![],
            rxs: vec![],
            slots: vec![],
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn default() -> Self {
        ViewInternals {
            element: Rc::new(JsValue::NULL),
            callbacks: vec![],
            rxs: vec![],
            slots: vec![],
            server_node: ServerNode {
                name_or_text: NameOrText::Name(Rc::new(RefCell::new("".to_string()))),
                attributes: vec![],
                styles: vec![],
            },
        }
    }
}


impl ViewInternals {
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

    /// Adds a View as a child node by storing the View and adding its element to the DOM.
    pub fn add_child<E: IsDomNode + AsRef<Node>>(&mut self, child: View<E>) {
        if cfg!(target_arch = "wasm32") {
            let node: &Node = self.element.unchecked_ref();
            let child_internals: Ref<ViewInternals> = child.internals.as_ref().borrow();
            node.append_child(&child_internals.element.as_ref().unchecked_ref())
                .expect("Could not add text node to View");
        }
        let child = child.upcast();
        self.slots.push(child);
    }

    /// Adds a View as a child node at a certain index by storing the View and adding its element to the DOM.
    /// If the index given is greater than the number of children, the given child is appended to the end of
    /// the node list.
    pub fn add_child_at<E: IsDomNode + AsRef<Node>>(&mut self, index: usize, child: View<E>) {
        if index >= self.slots.len() {
            return self.add_child(child);
        }

        if cfg!(target_arch = "wasm32") {
            let node: &Node = self.element.unchecked_ref();
            self.slots
                .get(index)
                .into_iter()
                .for_each(|view_after: &View<Node>| {
                    node.insert_before(child.dom_ref().as_ref(), Some(&view_after.dom_ref()))
                        .unwrap_throw();
                });
        }

        self.slots.insert(index, child.upcast());
    }

    /// Removes a child View node from a certain index.
    /// If the index given is greater than the number of children the result is `None`.
    pub fn remove_child_at(&mut self, index: usize) -> Option<View<Node>> {
        if index >= self.slots.len() {
            return None;
        }

        let child: View<Node> = self.slots.remove(index);
        if cfg!(target_arch = "wasm32") {
            let node: &Node = self.element.unchecked_ref();
            node.remove_child(&child.dom_ref()).unwrap_throw();
        }
        Some(child)
    }

    /// Adds an event on this view's element that will be stored by this view.
    pub fn add_event_on_this(&mut self, ev_name: &str, tx: Transmitter<Event>) {
        if let Some(target) = self.element.dyn_ref::<EventTarget>() {
            let cb = utils::add_event(ev_name, &target, tx);
            self.callbacks.push(StoredCb::This(ev_name.to_string(), cb));
        }
    }

    /// Adds an event on the window that will be stored by this view.
    pub fn add_event_on_window(&mut self, ev_name: &str, tx: Transmitter<Event>) {
        let target = utils::window();
        let cb = utils::add_event(ev_name, &target, tx);
        self.callbacks
            .push(StoredCb::Window(ev_name.to_string(), cb));
    }

    /// Adds an event on the window that will be stored by this view.
    pub fn add_event_on_document(&mut self, ev_name: &str, tx: Transmitter<Event>) {
        let target = utils::document();
        let cb = utils::add_event(ev_name, &target, tx);
        self.callbacks
            .push(StoredCb::Document(ev_name.to_string(), cb));
    }

    /// Attempt to add a style to this view. If the underlying DOM type `T` does not
    /// implement [`AsRef<HtmlElement>`] then this function is a noop.
    pub fn add_style(&mut self, name: &str, effect: Effect<String>) {
        let (may_now, may_later) = effect.into();
        if cfg!(target_arch = "wasm32") {
            if let Some(element) = self.element.dyn_ref::<HtmlElement>() {
                if let Some(now) = may_now {
                    element
                        .style()
                        .set_property(name, now.as_str())
                        .unwrap_throw();
                }
                if let Some(later) = may_later {
                    let rx = later;
                    // Save a clone so we can drop_responder if this gizmo goes out of scope
                    self.rxs
                        .push(StoredRx::String(crate::txrx::hand_clone(&rx)));

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
                self.rxs
                    .push(StoredRx::String(crate::txrx::hand_clone(&rx)));

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
            if let Some(element) = self.element.dyn_ref::<Element>() {
                if let Some(now) = may_now {
                    element.set_attribute(name, &now).unwrap_throw();
                }
                if let Some(later) = may_later {
                    let rx = later;

                    // Save a clone so we can drop_responder if this gizmo goes out of scope
                    self.rxs
                        .push(StoredRx::String(crate::txrx::hand_clone(&rx)));

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
                self.rxs
                    .push(StoredRx::OptionString(crate::txrx::hand_clone(&rx)));

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
            if let Some(element) = self.element.dyn_ref::<Element>() {
                if let Some(true) = may_now {
                    element.set_attribute(name, "").unwrap_throw();
                }
                if let Some(later) = may_later {
                    let rx = later.branch();
                    // Save a clone so we can drop_responder if this gizmo goes out of scope
                    self.rxs.push(StoredRx::Bool(crate::txrx::hand_clone(&rx)));

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
                self.rxs.push(StoredRx::Bool(crate::txrx::hand_clone(&rx)));

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
}


/// A widget that may contain a bundled network of html elements, callback
/// closures and receivers. This wraps a Javascript DOM node and maintains lists
/// and maps needed to orchestrate user interaction.
#[derive(Clone, Default)]
pub struct View<T: IsDomNode> {
    pub(crate) phantom: PhantomData<*const T>,
    pub(crate) internals: Rc<RefCell<ViewInternals>>,
}


impl<T: IsDomNode> View<T> {
    /// Return a reference to the underlying DOM element.
    pub fn dom_ref(&self) -> Ref<T> {
        Ref::map(self.internals.borrow(), |internals| {
            internals.element.unchecked_ref::<T>()
        })
    }

    /// Stores a View without adding its element to the DOM.
    ///
    /// ## NOTE: This is for use by helper libraries like `mogwai-hydrator` and is not
    /// intended to be called by downsteam users.
    pub fn store_view(&mut self, child: View<Node>) {
        self.internals.borrow_mut().slots.push(child);
    }

    /// Returns the number of views being stored in this view.
    pub fn stored_views_len(&self) -> usize {
        self.internals.borrow().slots.len()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn as_ssr_node(&self) -> SsrNode {
        let View {
            server_node, slots, ..
        } = self;
        match &server_node.name_or_text {
            NameOrText::Name(name) => SsrNode::Container {
                name: name.borrow().clone(),
                attributes: {
                    // TODO: Merge attribute style with explicit styles
                    let mut attributes = server_node
                        .attributes
                        .iter()
                        .map(|(k, var)| (k.clone(), var.borrow().clone()))
                        .collect::<Vec<_>>();
                    if !server_node.styles.is_empty() {
                        attributes.push((
                            "style".to_string(),
                            Some(
                                server_node
                                    .styles
                                    .iter()
                                    .map(|(k, v)| format!("{}: {};", k, v.borrow()))
                                    .collect::<Vec<_>>()
                                    .join(" "),
                            ),
                        ));
                    }
                    attributes
                },
                children: slots
                    .into_iter()
                    .map(|rc_ref_view| rc_ref_view.as_ref().borrow().as_ssr_node())
                    .collect(),
            },
            NameOrText::Text(text) => SsrNode::Text(text.borrow().clone()),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn html_string(&self) -> String {
        let value: Ref<JsValue> = Ref::map(self.internals.borrow(), |internals| {
            internals.element.as_ref()
        });

        if let Some(element) = value.dyn_ref::<Element>() {
            return element.outer_html();
        }

        if let Some(text) = value.dyn_ref::<Text>() {
            return text.data();
        }
        panic!(
            "Dom reference {:#?} could not be turned into a string",
            value
        );
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn html_string(&self) -> String {
        String::from(self.as_ssr_node())
    }

    /// Create a new `View` wrapping a `T` that can be dereferenced to a `Node`.
    ///
    /// # Panics
    /// Panics if used outside of a wasm32 target.
    #[cfg(target_arch = "wasm32")]
    pub fn wrapping(element: T) -> View<T> {
        let mut internals = ViewInternals::default();
        internals.element = Rc::new(element.unchecked_into());

        View {
            phantom: PhantomData,
            internals: Rc::new(RefCell::new(internals)),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn wrapping(_element: T) -> View<T> {
        panic!("View::wrapping is only available on wasm32")
    }

    /// # Note
    /// On wasm32 this performs a check on the inner element to determine if the
    /// element can be downcast to the desired type. On other compilation targets
    /// this function always returns Ok.
    #[cfg(target_arch = "wasm32")]
    pub fn try_cast<To: IsDomNode>(self) -> Result<View<To>, Self> {
        if self.internals.borrow().element.has_type::<To>() {
            Ok(View {
                phantom: PhantomData,
                internals: self.internals,
            })
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

    /// Cast the given View to contain the inner DOM node of another type.
    /// That type must be dereferencable from the given View.
    pub fn upcast<To: IsDomNode>(self) -> View<To>
    where
        T: AsRef<To>,
        To: IsDomNode,
    {
        View {
            phantom: PhantomData,
            internals: self.internals,
        }
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


impl<T: IsDomNode + AsRef<Node>> View<T> {
    /// Run this gizmo in a parent container forever, never dropping it.
    pub fn run_in_container(self, container: &Node) -> Result<(), JsValue> {
        if cfg!(target_arch = "wasm32") {
            let _ = container.append_child(self.dom_ref().as_ref());
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
        let mut internals = ViewInternals::default();
        internals.server_node = ServerNode {
            name_or_text: NameOrText::Text(Rc::new(RefCell::new(
                text.replace("&", "&amp;")
                    .replace("<", "&lt;")
                    .replace(">", "&gt;")
                    .into(),
            ))),
            attributes: vec![],
            styles: vec![],
        };
        View {
            phantom: PhantomData,
            internals: Rc::new(RefCell::new(internals)),
        }
    }

    pub fn rx_text(&mut self, rx: Receiver<String>) {
        self.internals
            .borrow_mut()
            .rxs
            .push(StoredRx::String(crate::txrx::hand_clone(&rx)));
        if cfg!(target_arch = "wasm32") {
            let text: Text = self.dom_ref().clone();
            rx.respond(move |s| text.set_data(s));
        } else {
            self.internals.borrow_mut().with_node(|node| {
                let text = node.name_or_text.clone();
                rx.respond(move |s| match &text {
                    NameOrText::Text(var) => {
                        *var.borrow_mut() = s.into();
                    }
                    _ => {}
                });
            });
        }
    }
}


/// # From instances for [`View`]
///
/// * String, str etc get converted into [`View<Text>`] - ie text nodes,
///   with their initial inner text set to the input string.
/// * Receiver<String> get converted into [`View<Text>`] with their
///   inner text set by the receiver.
/// * Effect<String> gets converted into [`View<Text>`] with possibly
///   an initial string and updates through the receiver.
/// * Any raw DOM element `T` gets wrapped by a view to make [`View<T>`]
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


impl<T: IsDomNode + AsRef<Node>> From<T> for View<T> {
    fn from(el: T) -> View<T> {
        View::wrapping(el)
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
            slots: vec![],

            callbacks: HashMap::default(),
            window_callbacks: HashMap::default(),
            document_callbacks: HashMap::default(),

            string_rxs: vec![],
            opt_string_rxs: vec![],
            bool_rxs: vec![],
            self_rxs: vec![],
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
            slots: vec![],

            callbacks: HashMap::default(),
            window_callbacks: HashMap::default(),
            document_callbacks: HashMap::default(),

            string_rxs: vec![],
            opt_string_rxs: vec![],
            bool_rxs: vec![],
            self_rxs: vec![],
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
            self.internals.borrow_mut().add_event_on_this(ev_name, tx);
        }
    }

    fn window_on(&mut self, ev_name: &str, tx: Transmitter<Event>) {
        if cfg!(target_arch = "wasm32") {
            self.internals.borrow_mut().add_event_on_window(ev_name, tx);
        }
    }

    fn document_on(&mut self, ev_name: &str, tx: Transmitter<Event>) {
        if cfg!(target_arch = "wasm32") {
            self.internals
                .borrow_mut()
                .add_event_on_document(ev_name, tx);
        }
    }
}


/// # AttributeView

impl<T: IsDomNode + AsRef<Element>> AttributeView for View<T> {
    fn attribute<E: Into<Effect<String>>>(&mut self, name: &str, eff: E) {
        let effect = eff.into();
        self.internals.borrow_mut().add_attribute(name, effect);
    }

    fn boolean_attribute<E: Into<Effect<bool>>>(&mut self, name: &str, eff: E) {
        let effect = eff.into();
        self.internals
            .borrow_mut()
            .add_boolean_attribute(name, effect);
    }
}


/// # ParentView


impl<S: IsDomNode + AsRef<Node>, T: IsDomNode + AsRef<Node>> ParentView<View<S>> for View<T> {
    fn with(&mut self, view: View<S>) {
        self.internals.borrow_mut().add_child(view);
    }
}


/// # StyleView


impl<T: IsDomNode + AsRef<HtmlElement>> StyleView for View<T> {
    fn style<E: Into<Effect<String>>>(&mut self, name: &str, eff: E) {
        let effect = eff.into().into();
        self.internals.borrow_mut().add_style(name, effect);
    }
}


/// # PostBuildView


impl<T: IsDomNode> PostBuildView for View<T> {
    type DomNode = T;

    fn post_build(&mut self, tx: Transmitter<T>) {
        let t: Ref<T> = Ref::map(self.internals.borrow(), |internals| {
            internals.element.unchecked_ref()
        });
        let t: T = t.clone();
        tx.send_async(async move { t });
    }
}


/// # ReplaceView


impl<T: IsDomNode + AsRef<Node>> ReplaceView<View<T>> for View<T> {
    fn this_later(&mut self, rx: Receiver<View<T>>) {
        let rx = rx.branch_map(|view| view.clone().upcast::<Node>());
        let internals: Rc<RefCell<ViewInternals>> = self.internals.clone();
        {
            let mut rxs: RefMut<Vec<StoredRx>> =
                RefMut::map(internals.borrow_mut(), |i| &mut i.rxs);
            rxs.push(StoredRx::View(crate::txrx::hand_clone(&rx)));
        }
        rx.respond(move |new_view| {
            {
                log::trace!("responded to new view: {}", new_view.html_string());

                let old_dom: Ref<Node> =
                    Ref::map(internals.borrow(), |i| i.element.unchecked_ref::<Node>());
                let new_dom: Ref<Node> = new_view.dom_ref();
                log::trace!("have old {:#?} and new {:#?}", old_dom, new_dom);
                if let Some(parent) = (old_dom.as_ref() as &Node).parent_node() {
                    log::trace!(
                        "  {:#?} is replacing {:#?} with {:#?}",
                        parent,
                        old_dom,
                        new_dom
                    );
                    parent.replace_child(new_dom.as_ref(), old_dom.as_ref())
                        .unwrap_throw();
                }
            }

            // take out all the self view replacement rxs and put them into the new view's stored rxs
            //{
            //    let old_rxs: Vec<StoredRx> = {
            //        let mut old_rxs_ref: RefMut<Vec<_>> =
            //            RefMut::map(internals.borrow_mut(), |i| &mut i.rxs);
            //        std::mem::replace(&mut old_rxs_ref, vec![])
            //    };
            //    let mut old_rxs_ref: RefMut<Vec<_>> =
            //        RefMut::map(internals.borrow_mut(), |i| &mut i.rxs);
            //    let mut new_rxs_ref: RefMut<Vec<_>> =
            //        RefMut::map(new_view.internals.borrow_mut(), |i| &mut i.rxs);
            //    for srx in old_rxs.into_iter() {
            //        match srx {
            //            StoredRx::View(rx) => {
            //                new_rxs_ref.push(StoredRx::View(rx));
            //            }
            //            _ => old_rxs_ref.push(srx),
            //        }
            //    }
            //}

            // swap the internals
            {
                let old_internals: &RefCell<ViewInternals> = internals.as_ref();
                let new_internals: &RefCell<ViewInternals> = new_view.internals.as_ref();
                old_internals.swap(new_internals);
            }

            log::trace!(
                "view is now: {}",
                internals
                    .borrow()
                    .element
                    .unchecked_ref::<HtmlElement>()
                    .outer_html()
            );
        });
    }
}


/// # PatchView


impl<T: IsDomNode + AsRef<Node>, C: IsDomNode + AsRef<Node>> PatchView<View<C>> for View<T> {
    fn patch(&mut self, rx: Receiver<Patch<View<C>>>) {
        let rx = rx.branch_map(|patch| match patch {
            Patch::Insert { index, value } => Patch::Insert {
                index: *index,
                value: value.clone().upcast::<Node>(),
            },
            Patch::Remove { index } => Patch::Remove { index: *index },
            Patch::PushFront { value } => Patch::PushFront {
                value: value.clone().upcast::<Node>(),
            },
            Patch::PushBack { value } => Patch::PushBack {
                value: value.clone().upcast::<Node>(),
            },
            Patch::PopFront => Patch::PopFront,
            Patch::PopBack => Patch::PopBack,
        });

        {
            let mut internals = self.internals.borrow_mut();
            internals
                .rxs
                .push(StoredRx::Patch(crate::txrx::hand_clone(&rx)));
        }

        let internals = self.internals.clone();
        rx.respond(move |patch| match patch {
            Patch::Insert { index, value } => {
                internals.borrow_mut().add_child_at(*index, value.clone());
            }
            Patch::Remove { index } => {
                internals.borrow_mut().remove_child_at(*index);
            }
            Patch::PushFront { value } => {
                internals.borrow_mut().add_child_at(0, value.clone());
            }
            Patch::PushBack { value } => {
                internals.borrow_mut().add_child(value.clone());
            }
            Patch::PopFront => {
                let _ = internals.borrow_mut().remove_child_at(0);
            }
            Patch::PopBack => {
                let mut i = internals.borrow_mut();
                let len = i.slots.len();
                let _ = i.remove_child_at(len - 1);
            }
        });
    }
}
