//! Widgets for the browser.
use std::{
    cell::{Ref, RefCell},
    convert::TryFrom,
    marker::PhantomData,
    rc::Rc,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
pub use web_sys::{Element, Event, EventTarget, HtmlElement};
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
    /// Create a new callback from a rust FnMut closure.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new<F>(f: F) -> Self
    where
        F: FnMut(JsValue) + 'static,
    {
        MogwaiCallback {
            callback: Rc::new(Box::new(f)),
        }
    }
    /// Placeholder docs to negate warnings.
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
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) server_node: ServerNode,
}

/// ViewInternal's Drop implementation removes any javascript callbacks being managed by this
/// view and also removes its element from the DOM if it is not referenced by any other view.
#[cfg(target_arch = "wasm32")]
impl Drop for ViewInternals {
    fn drop(&mut self) {
        let count = Rc::strong_count(&self.element);
        if count <= 1 {
            if let Some(node) = self.element.dyn_ref::<Node>() {
                if let Some(parent) = node.parent_node() {
                    let _ = parent.remove_child(&node);
                }
            }
        }

        let may_targets = if cfg!(target_arch = "wasm32") {
            self.element
                .dyn_ref::<EventTarget>()
                .cloned()
                .map(|target| (target, utils::window(), utils::document()))
        } else {
            None
        };

        if let Some((this, window, document)) = may_targets {
            for callback in self.callbacks.iter() {
                match callback {
                    StoredCb::This(event, mcb) => utils::remove_event(event, &this, mcb),
                    StoredCb::Window(event, mcb) => utils::remove_event(event, &window, mcb),
                    StoredCb::Document(event, mcb) => utils::remove_event(event, &document, mcb),
                }
            }
        }
    }
}

impl Default for ViewInternals {
    #[cfg(target_arch = "wasm32")]
    fn default() -> Self {
        ViewInternals {
            element: Rc::new(JsValue::NULL),
            callbacks: vec![],
            slots: vec![],
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn default() -> Self {
        ViewInternals {
            element: Rc::new(JsValue::NULL),
            callbacks: vec![],
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

    pub fn remove_all_children(&mut self) -> Vec<View<Node>> {
        let children: Vec<View<Node>> = std::mem::replace(&mut self.slots, vec![]);
        if cfg!(target_arch = "wasm32") {
            let node: &Node = self.element.unchecked_ref();
            for child in children.iter() {
                node.remove_child(&child.dom_ref()).unwrap_throw();
            }
        }
        children
    }

    /// Removes a child View node from a certain index and replaces it with another. Returns
    /// the child view that was replaced.
    /// If the index given is greater than the number of children the result is `None`.
    pub fn replace_child_at(&mut self, index: usize, new_child: View<Node>) -> Option<View<Node>> {
        if index >= self.slots.len() {
            return None;
        }

        let old_child: &View<Node> = self.slots.get(index).unwrap_throw();
        if cfg!(target_arch = "wasm32") {
            let node: &Node = self.element.unchecked_ref();
            node.replace_child(&new_child.dom_ref(), &old_child.dom_ref())
                .unwrap_throw();
        }
        old_child.internals.swap(&new_child.internals);
        // Hand back the new_child that now contains the internals of the old child.
        Some(new_child)
    }

    /// Adds an event on this view's element that will be stored by this view.
    pub fn add_event_on_this(&mut self, ev_name: &str, tx: Transmitter<Event>) {
        let cb = utils::add_event(ev_name, self.element.as_ref().unchecked_ref(), tx);
        self.callbacks.push(StoredCb::This(ev_name.to_string(), cb));
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
                    let style = element.style();
                    let name = name.to_string();
                    later.respond(move |s| {
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
                later.respond(move |s| {
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
                    let element = element.clone();
                    let name = name.to_string();
                    later.respond(move |s| {
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
                    let element = element.clone();
                    let name = name.to_string();
                    later.respond(move |b| {
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
                later.respond(move |is_present| {
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
#[derive(Clone)]
pub struct View<T: IsDomNode> {
    pub(crate) phantom: PhantomData<*const T>,
    pub(crate) internals: Rc<RefCell<ViewInternals>>,
}

impl<T: IsDomNode> Default for View<T> {
    fn default() -> Self {
        View {
            phantom: PhantomData,
            internals: Rc::new(RefCell::new(ViewInternals::default())),
        }
    }
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

    /// Create a server side rendering node from this View.
    /// Instead of using this, consider [`View::html_string`].
    #[cfg(not(target_arch = "wasm32"))]
    pub fn as_ssr_node(&self) -> SsrNode {
        let internals = self.internals.borrow();
        match &internals.server_node.name_or_text {
            NameOrText::Name(name) => SsrNode::Container {
                name: name.borrow().clone(),
                attributes: {
                    // TODO: Merge attribute style with explicit styles
                    let mut attributes = internals
                        .server_node
                        .attributes
                        .iter()
                        .map(|(k, var)| (k.clone(), var.borrow().clone()))
                        .collect::<Vec<_>>();
                    if !internals.server_node.styles.is_empty() {
                        attributes.push((
                            "style".to_string(),
                            Some(
                                internals
                                    .server_node
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
                children: internals
                    .slots
                    .iter()
                    .map(|rc_ref_view| rc_ref_view.as_ssr_node())
                    .collect(),
            },
            NameOrText::Text(text) => SsrNode::Text(text.borrow().clone()),
        }
    }

    /// A string value of this View.
    /// This is equivalent to `Element.outerHtml` in JS.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn html_string(&self) -> String {
        String::from(self.as_ssr_node())
    }
    /// Placeholder docs to negate warnings.
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

    /// Create a new `View` wrapping a `T` that can be dereferenced to a `Node`.
    ///
    /// # Panics
    /// Panics if used outside of a wasm32 target.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn wrapping(_element: T) -> View<T> {
        panic!("View::wrapping is only available on wasm32")
    }
    /// Placeholder docs to negate warnings.
    #[cfg(target_arch = "wasm32")]
    pub fn wrapping(element: T) -> View<T> {
        let mut internals = ViewInternals::default();
        internals.element = Rc::new(element.unchecked_into());

        View {
            phantom: PhantomData,
            internals: Rc::new(RefCell::new(internals)),
        }
    }

    /// Attempt to cast the view.
    /// # Note
    /// On wasm32 this performs a check on the inner element to determine if the
    /// element can be downcast to the desired type. On other compilation targets
    /// this function always returns Ok.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn try_cast<To: IsDomNode>(self) -> Result<View<To>, Self> {
        let mut view: View<To> = View::default();
        view.internals = self.internals;
        Ok(view)
    }
    /// Placeholder docs to negate warnings.
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
    /// Create a new text node View.
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
    /// Placeholder docs to negate warnings.
    #[cfg(target_arch = "wasm32")]
    pub fn text(text: &str) -> View<Text> {
        View::wrapping(Text::new_with_data(text).expect("could not create text"))
    }

    /// Use the given receiver to update this text node View's text.
    pub fn rx_text(&mut self, rx: Receiver<String>) {
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

impl<T: IsDomNode + AsRef<Node>> TryFrom<Option<View<T>>> for View<T> {
    type Error = ();

    fn try_from(o_view: Option<View<T>>) -> Result<View<T>, ()> {
        o_view.ok_or_else(|| ())
    }
}

/// # From instances for [`View`]
///
/// * String, str etc get converted into [`View<Text>`] - ie text nodes,
///   with their initial inner text set to the input string.
/// * Receiver<String> get converted into [`View<Text>`] with their
///   inner text initially empty and then later set by the messages sent to the
///   receiver.
/// * Effect<String> gets converted into [`View<Text>`] with possibly
///   an initial string and updates through the receiver.
/// * Any raw DOM element `T` gets wrapped by a view to make [`View<T>`]
/// * [`Gizmo<C>`] returns its view, a `View<<C::as Component>::DomNode>`.

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
        View::from(gizmo.view_builder())
    }
}

/// # ElementView

impl<T: IsDomNode> ElementView for View<T> {
    #[cfg(not(target_arch = "wasm32"))]
    fn element(name: &str) -> Self {
        let view = View::default();
        view.internals.borrow_mut().server_node = ServerNode {
            name_or_text: NameOrText::Name(Rc::new(RefCell::new(name.into()))),
            attributes: vec![],
            styles: vec![],
        };
        view
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
        let view = View::element(tag);
        view.internals.borrow_mut().server_node.attributes =
            vec![("xmlns".into(), Rc::new(RefCell::new(Some(ns.into()))))];
        view
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

impl<S: IsDomNode + AsRef<Node>, T: IsDomNode + AsRef<Node>> ParentView<Option<View<S>>>
    for View<T>
{
    fn with(&mut self, o_view: Option<View<S>>) {
        if let Some(view) = o_view {
            self.internals.borrow_mut().add_child(view);
        }
    }
}

impl<P, C> ParentView<Vec<View<C>>> for View<P>
where
    P: IsDomNode + AsRef<Node>,
    C: IsDomNode + AsRef<Node>,
{
    fn with(&mut self, children: Vec<View<C>>) {
        children.into_iter().for_each(|c| self.with(c));
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

    fn post_build(&mut self, _tx: Transmitter<T>) {
        #[cfg(target_arch = "wasm32")]
        {
            let t: Ref<T> = Ref::map(self.internals.borrow(), |internals| {
                internals.element.unchecked_ref()
            });
            let t: T = t.clone();
            _tx.send_async(async move { t });
        }
    }
}

fn to_child_node<T: IsDomNode + AsRef<Node>, S: Clone + Into<View<T>>>(value: &S) -> View<Node> {
    let s: S = value.clone();
    let v: View<T> = s.into();
    v.upcast::<Node>()
}

/// # PatchView

impl<T, C> PatchView<View<C>> for View<T>
where
    T: IsDomNode + AsRef<Node>,
    C: IsDomNode + AsRef<Node>,
{
    fn patch<S: Clone + Into<View<C>>>(&mut self, rx: Receiver<Patch<S>>) {
        let rx = rx.branch_map(|patch| patch.branch_map(to_child_node));

        let internals = self.internals.clone();
        rx.respond(move |patch| match patch {
            Patch::Insert { index, value } => {
                internals.borrow_mut().add_child_at(*index, value.clone());
            }
            Patch::Replace { index, value } => {
                internals
                    .borrow_mut()
                    .replace_child_at(*index, value.clone());
            }
            Patch::Remove { index } => {
                internals.borrow_mut().remove_child_at(*index);
            }
            Patch::RemoveAll => {
                internals.borrow_mut().remove_all_children();
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
