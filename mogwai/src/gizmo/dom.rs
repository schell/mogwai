//! Widgets for the browser.
use std::{cell::RefCell, collections::HashMap, marker::PhantomData, ops::Deref, rc::Rc};
use wasm_bindgen::closure::Closure;
pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
pub use web_sys::{Element, Event, EventTarget, HtmlElement, HtmlInputElement};
use web_sys::{Node, Text};

pub use super::utils;
use super::{
    super::{
        component::Component,
        ssr::Node as SsrNode,
        txrx::{hand_clone, Receiver, Transmitter},
    },
    view::*,
    Gizmo,
};


#[derive(Clone)]
pub enum NameOrText {
    Name(String),
    Text(String),
}


/// A place to store Closures and Receivers.
#[derive(Default)]
pub struct DomStorage {
    pub(crate) callbacks: HashMap<String, Rc<Closure<dyn FnMut(JsValue)>>>,
    pub(crate) window_callbacks: HashMap<String, Rc<Closure<dyn FnMut(JsValue)>>>,
    pub(crate) document_callbacks: HashMap<String, Rc<Closure<dyn FnMut(JsValue)>>>,
    pub(crate) string_rxs: Vec<Receiver<String>>,
    pub(crate) bool_rxs: Vec<Receiver<bool>>,
}


#[derive(Clone)]
pub struct ServerNode {
    pub(crate) name_or_text: NameOrText,
    pub(crate) attributes: Vec<(String, Option<String>)>,
    pub(crate) styles: Vec<String>,
}


/// A widget that may contain a bundled network of html elements, callback
/// closures and receivers. This wraps a Javascript DOM node and maintains lists
/// and maps needed to orchestrate user interaction.
pub struct DomWrapper<T: JsCast> {
    pub(crate) phantom: PhantomData<T>,
    pub(crate) element: Rc<JsValue>,
    pub(crate) children: Vec<DomWrapper<Node>>,

    #[cfg(target_arch = "wasm32")]
    pub(crate) dom_storage: DomStorage,

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) server_node: ServerNode,
}


impl<T: JsCast> DomWrapper<T> {
    #[cfg(target_arch = "wasm32")]
    fn with_storage<F>(&mut self, f: F)
    where
        F: FnOnce(&mut DomStorage),
    {
        f(&mut self.dom_storage);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn with_storage<F>(&mut self, _: F)
    where
        F: FnOnce(&mut DomStorage),
    {
        // Do nothing!
    }

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
        panic!("DomWrapper::to_ssr_node is only available outside of wasm32")
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn to_ssr_node(self) -> SsrNode {
        let DomWrapper {
            server_node,
            children,
            ..
        } = self;
        match server_node.name_or_text {
            NameOrText::Name(name) => SsrNode::Container {
                name,
                attributes: server_node.attributes,
                children: children.into_iter().map(|g| g.to_ssr_node()).collect(),
            },
            NameOrText::Text(text) => SsrNode::Text(text),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn element(name: &str) -> Self {
        DomWrapper {
            element: Rc::new(JsValue::NULL),
            phantom: std::marker::PhantomData,
            server_node: ServerNode {
                name_or_text: NameOrText::Name(name.into()),
                attributes: vec![],
                styles: vec![],
            },
            children: vec![],
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub fn element(name: &str) -> Self {
        let name = name.into();
        let el = utils::document()
            .create_element(name)
            .expect(&format!("cannot create element {:?}", name))
            .unchecked_into();
        DomWrapper::wrapping(el)
    }

    /// Adds a DomWrapper as a child node.
    pub fn add_child<E: JsCast + AsRef<Node> + Clone>(&mut self, child: DomWrapper<E>) {
        if cfg!(target_arch = "wasm32") {
            let node: &Node = self.as_ref().unchecked_ref();
            node.append_child(&child.element.as_ref().unchecked_ref())
                .expect("Could not add text node to DomWrapper");
        }
        self.children.push(child.upcast());
    }
}


impl DomWrapper<Text> {
    #[cfg(target_arch = "wasm32")]
    pub fn text(text: &str) -> DomWrapper<Text> {
        DomWrapper::wrapping(Text::new_with_data(text).expect("could not create text"))
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn text(text: &str) -> DomWrapper<Text> {
        DomWrapper {
            element: Rc::new(JsValue::NULL),
            phantom: std::marker::PhantomData,
            server_node: ServerNode {
                name_or_text: NameOrText::Text(
                    text.replace("&", "&amp;")
                        .replace("<", "&lt;")
                        .replace(">", "&gt;")
                        .into(),
                ),
                attributes: vec![],
                styles: vec![],
            },
            children: vec![],
        }
    }
}


impl<T: JsCast + Clone> Clone for DomWrapper<T> {
    fn clone(&self) -> Self {
        self.clone_as()
    }
}


impl<T, S> AsRef<S> for DomWrapper<T>
where
    T: JsCast + AsRef<S>,
    S: JsCast,
{
    fn as_ref(&self) -> &S {
        self.element.unchecked_ref::<S>()
    }
}


impl<T: JsCast> Deref for DomWrapper<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.element.unchecked_ref::<T>()
    }
}


impl<T: JsCast> View for DomWrapper<T> {
    type DomNode = T;

    fn into_dom(self) -> DomWrapper<T> {
        self
    }

    fn into_html_string(self) -> String {
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


impl<T: JsCast + AsRef<EventTarget>> EventTargetView for DomWrapper<T> {
    fn on(mut self, ev_name: &str, tx: Transmitter<Event>) -> DomWrapper<T> {
        if cfg!(target_arch = "wasm32") {
            let target: &EventTarget = self.as_ref();
            let cb = utils::add_event(ev_name, target, tx);
            self.with_storage(|dom| {
                dom.callbacks.insert(ev_name.to_string(), cb);
            });
            self
        } else {
            self
        }
    }

    fn window_on(mut self, ev_name: &str, tx: Transmitter<Event>) -> DomWrapper<T> {
        if cfg!(target_arch = "wasm32") {
            let window = utils::window();
            let target: &EventTarget = window.as_ref();
            let cb = utils::add_event(ev_name, &target, tx);
            self.with_storage(|dom| {
                dom.window_callbacks.insert(ev_name.to_string(), cb);
            });
            self
        } else {
            self
        }
    }

    fn document_on(mut self, ev_name: &str, tx: Transmitter<Event>) -> DomWrapper<T> {
        if cfg!(target_arch = "wasm32") {
            let doc = utils::document();
            let target: &EventTarget = doc.as_ref();
            let cb = utils::add_event(ev_name, target, tx);
            self.with_storage(|dom| {
                dom.document_callbacks.insert(ev_name.to_string(), cb);
            });
            self
        } else {
            self
        }
    }
}


/// # AttributeView

impl<T: JsCast + AsRef<Element>> AttributeView for DomWrapper<T> {
    fn attribute<E: Into<Effect<String>>>(mut self, name: &str, eff: E) -> DomWrapper<T> {
        let once = |wrapper: &mut DomWrapper<T>, now: &str| {
            if cfg!(target_arch = "wasm32") {
                (wrapper.as_ref() as &Element)
                    .set_attribute(name, now)
                    .unwrap_throw();
            } else {
                wrapper.with_node(|node| {
                    node.attributes.push((name.into(), Some(now.to_string())));
                });
            }
        };
        let many = |wrapper: &mut DomWrapper<T>, rx: Receiver<String>| {
            // Save a clone so we can drop_responder if this gizmo goes out of scope
            wrapper.with_storage(|dom| {
                dom.string_rxs.push(hand_clone(&rx));
            });

            let element: &Element = wrapper.as_ref();
            let element = element.clone();
            let name = name.to_string();

            rx.respond(move |s| {
                element
                    .set_attribute(&name, s)
                    .expect("Could not set attribute");
            });
        };

        match eff.into() {
            Effect::OnceNow { now } => once(&mut self, now.as_ref()),
            Effect::ManyLater { later } => {
                if cfg!(target_arch = "wasm32") {
                    many(&mut self, later)
                }
            }
            Effect::OnceNowAndManyLater { now, later } => {
                once(&mut self, now.as_ref());
                if cfg!(target_arch = "wasm32") {
                    many(&mut self, later);
                }
            }
        }
        self
    }

    fn boolean_attribute<E: Into<Effect<bool>>>(mut self, name: &str, eff: E) -> DomWrapper<T> {
        let once = |wrapper: &mut DomWrapper<T>, is_present: bool| {
            if is_present {
                if cfg!(target_arch = "wasm32") {
                    (wrapper.as_ref() as &Element)
                        .set_attribute(name, "")
                        .unwrap_throw();
                } else {
                    wrapper.with_node(|node| {
                        node.attributes.push((name.into(), None));
                    });
                }
            }
        };
        let many = |wrapper: &mut DomWrapper<T>, rx: &Receiver<bool>| {
            if cfg!(target_arch = "wasm32") {
                let rx = rx.branch();
                // Save a clone so we can drop_responder if this gizmo goes out of scope
                wrapper.with_storage(|dom| {
                    dom.bool_rxs.push(hand_clone(&rx));
                });

                let element: &Element = wrapper.as_ref();
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
        };
        match eff.into() {
            Effect::OnceNow { now } => once(&mut self, now),
            Effect::ManyLater { later } => many(&mut self, &later),
            Effect::OnceNowAndManyLater { now, later } => {
                once(&mut self, now);
                many(&mut self, &later);
            }
        }

        self
    }
}


/// # ParentView

impl<T: JsCast + AsRef<Node>> ParentView<(&str, Receiver<String>)> for DomWrapper<T> {
    fn with(self, tuple: (&str, Receiver<String>)) -> Self {
        let eff:Effect<String> = tuple.into();
        self.with(eff)
    }
}


impl<T: JsCast + AsRef<Node>> ParentView<(String, Receiver<String>)> for DomWrapper<T> {
    fn with(self, tuple: (String, Receiver<String>)) -> Self {
        let eff:Effect<String> = tuple.into();
        self.with(eff)
    }
}


impl<T: JsCast + AsRef<Node>> ParentView<(&String, Receiver<String>)> for DomWrapper<T> {
    fn with(self, (now, later): (&String, Receiver<String>)) -> Self {
        let tuple = (now.clone(), later);
        let eff:Effect<String> = tuple.into();
        self.with(eff)
    }
}


impl<T: JsCast + AsRef<Node>> ParentView<Effect<String>> for DomWrapper<T> {
    fn with(mut self, eff: Effect<String>) -> Self {
        let (may_now, may_later) = eff.into_some();
        let now = may_now.unwrap_or("".into());
        let mut text = DomWrapper::text(&now);
        if let Some(rx) = may_later {
            text.with_storage(|store| {
                store.string_rxs.push(hand_clone(&rx));
            });
            let text_node: Text = (text.as_ref() as &Text).clone();
            rx.respond(move |s| {
                text_node.set_data(s.as_str());
            });
        }
        self.add_child(text);

        self
    }
}


impl<T: JsCast + AsRef<Node>> ParentView<&Node> for DomWrapper<T> {
    fn with(self, child: &Node) -> Self {
        let this: &Node = self.as_ref();
        this.append_child(child).unwrap_throw();
        self
    }
}


impl<T: JsCast + AsRef<Node>> ParentView<&str> for DomWrapper<T> {
    fn with(mut self, text: &str) -> Self {
        self.add_child(DomWrapper::text(text));
        self
    }
}


impl<T: JsCast + AsRef<Node>> ParentView<&String> for DomWrapper<T> {
    fn with(mut self, text: &String) -> Self {
        self.add_child(DomWrapper::text(text));
        self
    }
}


impl<T> ParentView<Gizmo<T>> for DomWrapper<<T as Component>::DomNode>
where
    T: Component,
    <T as Component>::DomNode: JsCast + AsRef<Node>,
{
    fn with(self, gizmo: Gizmo<T>) -> Self {
        self.with(gizmo.view)
    }
}


impl<P, C> ParentView<DomWrapper<C>> for DomWrapper<P>
where
    P: JsCast + AsRef<Node>,
    C: JsCast + AsRef<Node> + Clone,
{
    fn with(mut self, dom: DomWrapper<C>) -> Self {
        self.add_child(dom);
        self
    }
}


/// # StyleView

impl<T: JsCast + AsRef<HtmlElement>> StyleView for DomWrapper<T> {
    fn style<E: Into<Effect<String>>>(mut self, name: &str, eff: E) -> Self {
        let (may_now, may_later) = eff.into().into_some();
        if let Some(now) = may_now {
            if cfg!(target_arch = "wasm32") {
                (self.as_ref() as &HtmlElement)
                    .style()
                    .set_property(name, now.as_str())
                    .unwrap_throw();
            } else {
                self.with_node(|node| {
                    node.styles.push(format!("{}:{};", name, now));
                });
            }
        }
        if cfg!(target_arch = "wasm32") {
            if let Some(later) = may_later {
                let rx = later;
                // Save a clone so we can drop_responder if this gizmo goes out of scope
                self.with_storage(|store| {
                    store.string_rxs.push(hand_clone(&rx));
                });

                let element: &HtmlElement = self.as_ref();
                let style = element.style();
                let name = name.to_string();

                rx.respond(move |s| {
                    style.set_property(&name, s).expect("Could not set style");
                });
            }
        }

        self
    }
}


/// # PostBuildView

impl<T: JsCast + Clone + 'static> PostBuildView for DomWrapper<T> {
    type DomNode = T;

    fn post_build(self, tx: Transmitter<T>) -> Self {
        let t: &T = self.element.unchecked_ref();
        let t: T = t.clone();
        tx.send_async(async move { t });
        self
    }
}


impl<T: JsCast> DomWrapper<T> {
    /// Create a new `DomWrapper` wrapping a `T` that can be dereferenced to a `Node`.
    ///
    /// # Panics
    /// Panics if used outside of a wasm32 target.
    #[cfg(target_arch = "wasm32")]
    pub fn wrapping(element: T) -> DomWrapper<T> {
        DomWrapper {
            element: Rc::new(element.into()),
            phantom: PhantomData,
            dom_storage: DomStorage {
                callbacks: HashMap::new(),
                window_callbacks: HashMap::new(),
                document_callbacks: HashMap::new(),
                string_rxs: vec![],
                bool_rxs: vec![],
            },
            children: vec![],
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn wrapping(_element: T) -> DomWrapper<T> {
        panic!("DomWrapper::wrapping is only available on wasm32")
    }
}


impl<T: JsCast + Clone> DomWrapper<T> {
    /// Creates a new gizmo with data cloned from the first.
    #[cfg(target_arch = "wasm32")]
    fn clone_as<D>(&self) -> DomWrapper<D>
    where
        D: JsCast,
    {
        DomWrapper {
            phantom: PhantomData,
            element: self.element.clone(),
            dom_storage: DomStorage {
                callbacks: self.dom_storage.callbacks.clone(),
                window_callbacks: self.dom_storage.window_callbacks.clone(),
                document_callbacks: self.dom_storage.document_callbacks.clone(),
                string_rxs: self
                    .dom_storage
                    .string_rxs
                    .iter()
                    .map(|rx| hand_clone(rx))
                    .collect(),
                bool_rxs: self
                    .dom_storage
                    .bool_rxs
                    .iter()
                    .map(|rx| hand_clone(rx))
                    .collect(),
            },
            children: self.children.clone(),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn clone_as<D>(&self) -> DomWrapper<D>
    where
        D: JsCast,
    {
        DomWrapper {
            phantom: PhantomData,
            element: self.element.clone(),
            server_node: self.server_node.clone(),
            children: self.children.clone(),
        }
    }

    /// Cast the given DomWrapper to contain the inner DOM node of another type.
    /// That type must be dereferencable from the given DomWrapper.
    pub fn upcast<D>(self) -> DomWrapper<D>
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
    pub fn downcast<To: JsCast + AsRef<Node>>(self) -> Result<DomWrapper<To>, DomWrapper<T>> {
        if self.element.has_type::<To>() {
            Ok(self.clone_as())
        } else {
            Err(self)
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn downcast<To: JsCast + AsRef<Node>>(self) -> Result<DomWrapper<To>, DomWrapper<T>> {
        Ok(self.clone_as())
    }
}


impl<T: JsCast + 'static> DomWrapper<T> {
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


impl<T: JsCast + AsRef<Node> + Clone + 'static> DomWrapper<T> {
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


/// DomWrapper's Drop implementation insures that responders no longer attempt to
/// update the gizmo. It also removes its element from the DOM.
#[cfg(target_arch = "wasm32")]
impl<T: JsCast> Drop for DomWrapper<T> {
    fn drop(&mut self) {
        let count = Rc::strong_count(&self.element);
        let node = self.element.unchecked_ref::<Node>().clone();
        if count <= 1 {
            self.with_storage(|dom| {
                if let Some(parent) = node.parent_node() {
                    let _ = parent.remove_child(&node);
                }
                dom.string_rxs.iter_mut().for_each(|rx| rx.drop_responder());
                dom.bool_rxs.iter_mut().for_each(|rx| rx.drop_responder());
            });
        }
    }
}


impl<'a> From<&'a str> for DomWrapper<Text> {
    fn from(s: &'a str) -> Self {
        DomWrapper::text(s)
    }
}


impl From<&String> for DomWrapper<Text> {
    fn from(s: &String) -> Self {
        DomWrapper::text(s)
    }
}


#[cfg(test)]
#[allow(unused_braces)]
mod gizmo_tests {
    #[allow(unused_braces)]

    use super::{super::super::prelude::*, *};
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
            let pre = dom! { <pre>"this has text"</pre> };
            let div = dom! { <div id="parent">{pre.as_ref() as &Node}</div> };
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
            let div = dom! {
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
        let root = dom! {
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
        let div = dom! {
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
        let div = dom! {
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
        let div =
            (DomWrapper::element("div") as DomWrapper<HtmlElement>).style("display", ("block", rx));
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
        let div = dom!{ <div style:display=("block", rx) /> };
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
        let div = (DomWrapper::element("div") as DomWrapper<HtmlElement>)
            .with(("initial", rx));
        let el:&HtmlElement = div.as_ref();
        assert_eq!(el.inner_text().as_str(), "initial");
        tx.send(&"after".into());
        assert_eq!(el.inner_text(), "after");
    }

    #[wasm_bindgen_test]
    fn tx_on_click_plain() {
        let (tx, rx) =
            txrx_fold(
                0,
                |n:&mut i32, _:&Event| -> String {
                    *n += 1;
                    if *n == 1 {
                        "Clicked 1 time".to_string()
                    } else {
                        format!("Clicked {} times", *n)
                    }
                }
            );

        let button = (DomWrapper::element("button") as DomWrapper<HtmlElement>)
            .with(("Clicked 0 times", rx))
            .on("click", tx);
        let el:&HtmlElement = button.as_ref();

        assert_eq!(el.inner_html(), "Clicked 0 times");
        el.click();
        assert_eq!(el.inner_html(), "Clicked 1 time");
    }

    #[wasm_bindgen_test]
    fn tx_on_click_jsx() {
        let (tx, rx) =
            txrx_fold(
                0,
                |n:&mut i32, _:&Event| -> String {
                    *n += 1;
                    if *n == 1 {
                        "Clicked 1 time".to_string()
                    } else {
                        format!("Clicked {} times", *n)
                    }
                }
            );

        let button = dom! { <button on:click=tx>{("Clicked 0 times", rx)}</button> };
        let el:&HtmlElement = button.as_ref();

        assert_eq!(el.inner_html(), "Clicked 0 times");
        el.click();
        assert_eq!(el.inner_html(), "Clicked 1 time");
    }


    #[wasm_bindgen_test]
    fn tx_window_on_click_jsx() {
        let (tx, rx) = txrx();
        let _button = dom! {
            <button window:load=tx>
            {(
                "Waiting...",
                rx.branch_map(|_| "Loaded!".into())
            )}
            </button>
        };
    }

    //fn nice_compiler_error() {
    //    let _div = dom! {
    //        <div unknown:colon:thing="not ok" />
    //    };
    //}
}
