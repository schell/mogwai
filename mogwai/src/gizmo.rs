//! A widget.
use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Deref;
use std::rc::Rc;
use wasm_bindgen::closure::Closure; 
use web_sys::{HtmlElement, Node, Text};

//use super::builder::GizmoBuilder;
use super::txrx::{hand_clone, Receiver, Transmitter};
pub use super::utils;
pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
pub use web_sys::{Element, Event, EventTarget, HtmlInputElement};

pub mod html;


/// A widget that may contain a bundled network of html elements, callback
/// closures and receivers.
pub struct Gizmo<T: JsCast> {
  pub(crate) element: Rc<JsValue>,
  pub(crate) phantom: PhantomData<T>, 
  pub(crate) callbacks: HashMap<String, Rc<Closure<dyn FnMut(JsValue)>>>,
  pub(crate) window_callbacks:
    HashMap<String, Rc<Closure<dyn FnMut(JsValue)>>>,
  pub(crate) document_callbacks:
    HashMap<String, Rc<Closure<dyn FnMut(JsValue)>>>,
  pub(crate) opt_string_rxs: Vec<Receiver<Option<String>>>,
  pub(crate) string_rxs: Vec<Receiver<String>>,
  pub(crate) bool_rxs: Vec<Receiver<bool>>,
  //pub(crate) gizmo_rxs: Vec<Receiver<Vec<Gizmo<Node>>>>,
  pub(crate) static_gizmos: Vec<Gizmo<Node>>,
}


impl<T: JsCast + Clone> Clone for Gizmo<T> {
  fn clone(&self) -> Self {
    self.clone_as()
  }
}


impl<T, S> AsRef<S> for Gizmo<T>
where
  T: JsCast + AsRef<S>,
  S: JsCast,
{
  fn as_ref(&self) -> &S {
    self.element.unchecked_ref::<S>()
  }
}


impl<T: JsCast> Deref for Gizmo<T> {
  type Target = T;

  fn deref(&self) -> &T {
    self.element.unchecked_ref::<T>()
  }
}


impl<T: JsCast + AsRef<EventTarget>> Gizmo<T> {
  /// Sends an event into the given transmitter when the given dom event happens.
  pub fn tx_on(mut self, ev_name: &str, tx: Transmitter<Event>) -> Gizmo<T> {
    let target: &EventTarget = self.as_ref();
    let cb = utils::add_event(ev_name, target, tx);
    self.callbacks.insert(ev_name.to_string(), cb); 
    self
  }


  /// Sends an event into the given transmitter when the given dom event happens
  /// on `window`.
  pub fn tx_on_window(
    mut self,
    ev_name: &str,
    tx: Transmitter<Event>,
  ) -> Gizmo<T> {
    let window = utils::window();
    let target: &EventTarget = window.as_ref();
    let cb = utils::add_event(ev_name, &target, tx);
    self.window_callbacks.insert(ev_name.to_string(), cb);
    self
  }

  /// Sends an event into the given transmitter when the given dom event happens.
  pub fn tx_on_document(
    mut self,
    ev_name: &str,
    tx: Transmitter<Event>,
  ) -> Gizmo<T> {
    let doc = utils::document();
    let target: &EventTarget = doc.as_ref();
    let cb = utils::add_event(ev_name, target, tx);
    self.document_callbacks.insert(ev_name.to_string(), cb);
    self
  }
}


/// Anything that can be nested within a Gizmo.
pub trait SubGizmo
where
  Self: Sized,
{
  /// Attempts to turn the Gizmo into a child gizmo who's inner element is a
  /// Node, if possible. Otherwise this will return a Node.
  fn into_sub_gizmo(self) -> Result<Gizmo<Node>, Node>;
}


impl<T: JsCast + AsRef<Node> + Clone> SubGizmo for Gizmo<T> {
  fn into_sub_gizmo(self) -> Result<Gizmo<Node>, Node> {
    Ok(self.upcast())
  }
}


impl<T: JsCast + AsRef<Node> + Clone> SubGizmo for &Gizmo<T> {
  fn into_sub_gizmo(self) -> Result<Gizmo<Node>, Node> {
    let node: &Node = self.as_ref();
    Err(node.clone())
  }
}


impl<T: JsCast + AsRef<Node>> Gizmo<T> {
  /// Create a text node and insert it into the Gizmo being built.
  pub fn text(self, s: &str) -> Gizmo<T> {
    let text: Text = Text::new_with_data(s).unwrap_throw();
    (self.as_ref() as &Node)
      .append_child(text.as_ref())
      .expect("Could not add text node to gizmo element");
    self
  }

  /// Create a text node that changes its text to anything that is sent on the
  /// given receiver and insert it into the Gizmo being built.
  pub fn rx_text(mut self, init: &str, rx: Receiver<String>) -> Gizmo<T> {
    // Save a clone so we can drop_responder if this gizmo goes out of scope
    self.string_rxs.push(hand_clone(&rx));

    let text: Text = Text::new_with_data(init).unwrap_throw();

    (self.as_ref() as &Node)
      .append_child(text.as_ref())
      .expect("Could not add text node to gizmo");
    rx.respond(move |s| {
      text.set_data(s);
    });
    self
  }

  /// Append a child that implements SubGizmo to this Gizmo.
  pub fn with<Child>(mut self, child: Child) -> Gizmo<T>
  where
    Child: SubGizmo,
  {
    // If this thing is a gizmo then store it and append it, otherwise just
    // append it
    match child.into_sub_gizmo() {
      Ok(gizmo) => {
        (self.as_ref() as &Node)
          .append_child(&gizmo)
          .expect("Gizmo::with could not append gizmo");
        self.static_gizmos.push(gizmo);
      }
      Err(node) => {
        (self.as_ref() as &Node)
          .append_child(&node)
          .expect("Gizmo::with could not append node");
      }
    }

    self
  }
}


impl<T: JsCast + AsRef<Element>> Gizmo<T> {
  /// Create a static attribute on the Gizmo being built.
  pub fn attribute(self, name: &str, value: &str) -> Gizmo<T> {
    (self.as_ref() as &Element)
      .set_attribute(name, value)
      .unwrap_throw();
    self
  }

  /// If `condition` is `true`, create a static boolean attribute on the Gizmo
  /// being built.
  ///
  /// For background on `condition` See
  /// https://github.com/schell/mogwai/issues/19
  pub fn boolean_attribute(self, name: &str, condition: bool) -> Gizmo<T> {
    if condition {
      (self.as_ref() as &Element)
        .set_attribute(name, "")
        .unwrap_throw();
      self
    } else {
      self
    }
  }

  /// Create a class attribute on the Gizmo being built.
  ///
  /// This represents all the classes for this gizmo. If you'd like to specify
  /// more than one class call this as:
  /// ```rust
  /// extern crate mogwai;
  /// use mogwai::prelude::*;
  ///
  /// let gizmo =
  ///   Gizmo::element("div");
  ///   .class("class1 class2 class3 etc");
  /// ```
  pub fn class(self, value: &str) -> Gizmo<T> {
    self.attribute("class", value)
  }

  /// Create an id attribute on the Gizmo being built.
  pub fn id(self, value: &str) -> Gizmo<T> {
    self.attribute("id", value)
  }

  /// Create an attribute on the Gizmo being built that changes its value every
  /// time the given receiver receives a message. If the receiver receives `None`
  /// it will respond by removing the attribute until it receives `Some(...)`.
  pub fn rx_attribute(
    mut self,
    name: &str,
    init: Option<&str>,
    rx: Receiver<Option<String>>,
  ) -> Gizmo<T> {
    // Save a clone so we can drop_responder if this gizmo goes out of scope
    self.opt_string_rxs.push(hand_clone(&rx));

    let element: &Element = self.as_ref();
    if let Some(init) = init {
      element
        .set_attribute(name, &init)
        .expect("Could not set attribute");
    }

    let element = element.clone();
    let name = name.to_string();

    rx.respond(move |s| {
      if let Some(s) = s {
        element
          .set_attribute(&name, s)
          .expect("Could not set attribute");
      } else {
        element
          .remove_attribute(&name)
          .expect("Could not remove attribute");
      }
    });

    self
  }

  /// Create a boolean attribute on the Gizmo being built that changes its value every
  /// time the given receiver receives a message. If the receiver receives `None`
  /// it will respond by removing the attribute until it receives `Some(...)`.
  pub fn rx_boolean_attribute(
    mut self,
    name: &str,
    init: bool,
    rx: Receiver<bool>,
  ) -> Gizmo<T> {
    // Save a clone so we can drop_responder if this gizmo goes out of scope
    self.bool_rxs.push(hand_clone(&rx));

    let element: &Element = self.as_ref();

    if init {
      element
        .set_attribute(name, "")
        .expect("Could not set attribute");
    }

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

    self
  }


  /// Create a class attribute on the Gizmo being built that changes its value
  /// every time the given receiver receives a message.
  pub fn rx_class(self, init: &str, rx: Receiver<String>) -> Gizmo<T> {
    self.rx_attribute("class", Some(init), rx.branch_map(|s| Some(s.clone())))
  }
}


impl<T: JsCast + AsRef<HtmlElement>> Gizmo<T> {
  /// Set a CSS property in the style attribute of the Gizmo being built.
  pub fn style(self, name: &str, value: &str) -> Gizmo<T> {
    (self.as_ref() as &HtmlElement)
      .style()
      .set_property(name, value)
      .unwrap_throw();
    self
  }

  /// Set a CSS property in the style attribute of the Gizmo being built that
  /// updates its value every time a message is received on the given `Receiver`.
  pub fn rx_style(
    mut self,
    s: &str,
    init: &str,
    rx: Receiver<String>,
  ) -> Gizmo<T> {
    // Save a clone so we can drop_responder if this gizmo goes out of scope
    self.string_rxs.push(hand_clone(&rx));

    let element: &HtmlElement = self.as_ref();
    let style = element.style();
    let name = s.to_string();

    style
      .set_property(&name, init)
      .expect("Could not set initial style property");

    rx.respond(move |s| {
      style.set_property(&name, s).expect("Could not set style");
    });

    self
  }
}


impl<T: JsCast + AsRef<HtmlInputElement>> Gizmo<T> {
  /// Set the value of the Gizmo being built.
  pub fn value(self, s: &str) -> Gizmo<T> {
    let input: &HtmlInputElement = self.as_ref();
    input.set_value(s);
    self
  }

  /// Set the value of the Gizmo being built that updates every time a message is
  /// received on the given Receiver.
  pub fn rx_value(mut self, init: &str, rx: Receiver<String>) -> Gizmo<T> {
    // Save a clone so we can drop_responder if this gizmo goes out of scope
    self.string_rxs.push(hand_clone(&rx));
    let input: &HtmlInputElement = self.as_ref();
    input.set_value(init);

    let input = input.clone();
    rx.respond(move |val: &String| {
      input.set_value(val);
    });

    self
  }
}


impl<T: JsCast> Gizmo<T> {
  /// Create a new `Gizmo` wrapping a `T` that can be dereferenced to a `Node`.
  pub fn wrapping(element: T) -> Gizmo<T> {
    Gizmo {
      element: Rc::new(element.into()),
      phantom: PhantomData,
      callbacks: HashMap::new(),
      window_callbacks: HashMap::new(),
      document_callbacks: HashMap::new(),
      opt_string_rxs: vec![],
      string_rxs: vec![],
      //gizmo_rxs: vec![],
      bool_rxs: vec![],
      static_gizmos: vec![],
    }
  }
}


impl<T: JsCast + Clone> Gizmo<T> {
  /// Creates a new gizmo with data cloned from the first, but with a null
  /// element.
  fn clone_as<D>(&self) -> Gizmo<D>
  where
    D: JsCast,
  {
    Gizmo {
      element: self.element.clone(),
      phantom: PhantomData,
      callbacks: self.callbacks.clone(),
      window_callbacks: self.window_callbacks.clone(),
      document_callbacks: self.document_callbacks.clone(),
      opt_string_rxs: self
        .opt_string_rxs
        .iter()
        .map(|rx| hand_clone(rx))
        .collect(),
      string_rxs: self.string_rxs.iter().map(|rx| hand_clone(rx)).collect(),
      bool_rxs: self.bool_rxs.iter().map(|rx| hand_clone(rx)).collect(),
      //gizmo_rxs: self.gizmo_rxs.iter().map(|rx| hand_clone(rx)).collect(),
      static_gizmos: self.static_gizmos.clone(),
    }
  }

  /// Cast the given Gizmo to contain the inner DOM node of another type.
  /// That type must be dereferencable from the given Gizmo.
  pub fn upcast<D>(self) -> Gizmo<D>
  where
    T: AsRef<D>,
    D: JsCast,
  {
    self.clone_as()
  }

  /// Attempt to downcast the inner element.
  pub fn downcast<To: JsCast + AsRef<Node>>(
    self,
  ) -> Result<Gizmo<To>, Gizmo<T>> {
    if self.element.has_type::<To>() {
      Ok(self.clone_as())
    } else {
      Err(self)
    }
  }
}


impl<T: JsCast + 'static> Gizmo<T> {
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


impl<T: JsCast + AsRef<Node> + Clone + 'static> Gizmo<T> {
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

  /// After the gizmo is built, send a clone of T on the given transmitter.
  /// This allows you to construct component behaviors that operate on the T
  /// directly, while still keeping the Gizmo in its place within your view
  /// function. For example, you may want to use `input.focus()` within the
  /// `update` function of your component. This method allows you to store the
  /// input's `HtmlInputElement` once it is built.
  pub fn tx_post_build(self, tx: Transmitter<T>) -> Gizmo<T> {
    let t: &T = self.element.unchecked_ref();
    let t: T = t.clone();
    tx.send_async(async move { t });
    self
  }
}


impl Gizmo<Element> {
  /// Create a new gizmo with the given element tag.
  /// ```rust,ignore
  /// Gizmo::element("div")
  /// ```
  pub fn element(tag: &str) -> Self {
    let element: Element = utils::document().create_element(tag).unwrap_throw();
    Gizmo::wrapping(element)
  }


  /// Create a new gizmo with the given element tag.
  /// ```rust,ignore
  /// Gizmo::element("div")
  /// ```
  pub fn element_ns(tag: &str, ns: &str) -> Self {
    let element: Element =
      utils::document()
      .create_element_ns(Some(ns), tag)
      .unwrap_throw();
    Gizmo::wrapping(element)
  }


  /// Create a new Gizmo from an existing Element with the given id.
  /// Returns None if it cannot be found.
  pub fn from_element_by_id(id: &str) -> Option<Gizmo<Element>> {
    let el = utils::document().get_element_by_id(id)?;
    Some(Gizmo::wrapping(el))
  }
}


/// Gizmo's Drop implementation insures that responders no longer attempt to
/// update the gizmo. It also removes its element from the DOM.
impl<T: JsCast> Drop for Gizmo<T> {
  fn drop(&mut self) {
    let count = Rc::strong_count(&self.element);
    if count <= 1 {
      let node:&Node = self.element.unchecked_ref();
      if let Some(parent) = node.parent_node() {
        let _ = parent.remove_child(&node);
      }

      self
        .opt_string_rxs
        .iter_mut()
        .for_each(|rx| rx.drop_responder());

      self
        .string_rxs
        .iter_mut()
        .for_each(|rx| rx.drop_responder());

      self.bool_rxs.iter_mut().for_each(|rx| rx.drop_responder());
    }
  }
}


#[cfg(test)]
mod gizmo_tests {
  use super::html::{div, pre};
  use super::SubGizmo;
  use wasm_bindgen::JsCast;
  use wasm_bindgen_test::*;
  use web_sys::{console, Element};

  wasm_bindgen_test_configure!(run_in_browser);

  #[wasm_bindgen_test]
  fn can_into_sub_gizmo() {
    let tag = div().id("sub-gizmo");
    let ref_res = (&tag).into_sub_gizmo();
    assert!(
      ref_res.is_err(),
      "gizmo reference does not sub-gizmo into Err(Node)"
    );

    let self_res = tag.into_sub_gizmo();
    assert!(
      self_res.is_ok(),
      "gizmo does not sub-gizmo into Ok(Gizmo<Node>)"
    );
    console::log_1(&"dropping sub-gizmo".into());
  }

  #[wasm_bindgen_test]
  fn gizmo_ref_as_child() {
    // Since the pre tag is dropped after the scope block the last assert should
    // show that the div tag has no children.
    let div =
      {
      let pre = pre().text("this has text");
      let div = div().id("parent").with(&pre);
      assert!(
        div.first_child().is_some(),
        "parent does not contain in-scope child"
      );
      console::log_1(&"dropping pre".into());
      div
    };
    assert!(
      div.first_child().is_none(),
      "parent does not maintain out-of-scope child"
    );
    console::log_1(&"dropping parent".into());
  }

  #[wasm_bindgen_test]
  fn gizmo_as_child() {
    // Since the pre tag is *not* dropped after the scope block the last assert
    // should show that the div tag has a child.
    let div =
      {
      let pre = pre().text("some text");
      let div = div().id("parent-div").with(pre);
      assert!(div.first_child().is_some(), "could not add child gizmo");
      div
    };
    assert!(
      div.first_child().is_some(),
      "could not keep hold of child gizmo"
    );
    assert_eq!(div.static_gizmos.len(), 1, "parent is missing static_gizmo");
    console::log_1(&"dropping div and pre".into());
  }

  #[wasm_bindgen_test]
  fn gizmo_tree() {
    let root =
      {
      let leaf = pre().id("leaf").text("leaf");
      let branch = div().id("branch").with(leaf);
      let root = div().id("root").with(branch);
      root
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
}
