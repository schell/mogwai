use std::convert::TryFrom;
/// [`TryFrom`] instances that can 're-animate' views or portions of views using the DOM.
use std::{cell::RefCell, collections::HashMap, marker::PhantomData, ops::Deref, rc::Rc};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
pub use web_sys::{Element, Event, EventTarget, HtmlElement, HtmlInputElement};
use web_sys::{Node, Text};

use super::super::{
    super::{
        component::Component,
        ssr::Node as SsrNode,
        txrx::{hand_clone, Receiver, Transmitter},
    },
    view::*,
    Gizmo,
};
pub use super::utils;


use super::View;


pub struct HydrateView<T:JsCast> {
    tag: String,
    id: Option<String>,
    child_of: Option<(Node, u32)>,
    effect: Box<dyn FnOnce(View<T>) -> View<T>>
}


impl<T:JsCast> HydrateView<T> {
    pub fn new(tag: &str) -> Self {
        HydrateView {
            tag: tag.into(),
            id: None,
            child_of: None,
            effect: Box::new(|v| v)
        }
    }
}


impl<T: JsCast> ElementView for HydrateView<T> {
    fn element(tag: &str) -> Self {
        HydrateView::new(tag)
    }

    fn element_ns(tag: &str, _ns: &str) -> Self {
        HydrateView::new(tag)
    }

    fn from_element_by_id(id: &str) -> Option<Self> {
        Some(
            HydrateView {
                tag: "...".into(),
                id: Some(id.into()),
                child_of: None,
                effect: Box::new(|v| v)
            }
        )
    }
}


impl<T: JsCast + AsRef<Element> + 'static> AttributeView for HydrateView<T> {
    fn attribute<E: Into<Effect<String>>>(mut self, name: &str, eff: E) -> Self {
        let (may_now, may_later) = eff.into().into_some();
        if let Some(now) = may_now {
            if name == "id" {
                self.id = Some(now);
            }
        }

        if let Some(later) = may_later {
            let name = name.to_string();
            let prev_effect = self.effect;
            self.effect = Box::new(move |v| {
                prev_effect(v)
                    .attribute(&name, later)
            });
        }
        self
    }


    fn boolean_attribute<E: Into<Effect<bool>>>(mut self, name: &str, eff: E) -> Self {
        let (_may_now, may_later) = eff.into().into_some();
        if let Some(later) = may_later {
            let name = name.to_string();
            let prev_effect = self.effect;
            self.effect = Box::new(move |v| {
                prev_effect(v)
                    .boolean_attribute(&name, later)
            });
        }
        self
    }
}


impl<T: JsCast> TryFrom<HydrateView<T>> for View<T> {
    type Error = String;

    fn try_from(HydrateView { tag, id, child_of, effect }: HydrateView<T>) -> Result<View<T>, Self::Error> {
        let view =
            if let Some((parent, index)) = child_of {
                let children = parent.child_nodes();
                let child = children.get(index).ok_or_else(|| format!("Could not find child {}", index))?;
                let el: T = child.dyn_into::<T>().map_err(|_| {
                    format!(
                        "Could not cast child at '{}' '{}'",
                        index,
                        std::any::type_name::<T>()
                    )
                })?;
                Ok(View::wrapping(el))
            } else if let Some(id) = id {
                let el: Element = utils::document()
                    .get_element_by_id(&id)
                    .ok_or_else(|| format!("Could not find any element by id '{}'", id))?;
                let el: T = el.dyn_into::<T>().map_err(|_| {
                    format!(
                        "Could not cast element by id '{}' into '{}'",
                        id,
                        std::any::type_name::<T>()
                    )
                })?;
                Ok(View::wrapping(el))
            } else {
                Err(format!("Not enough information to hydrate tag '{}' - needs an id or a parent element", tag))
            }?;
        Ok(effect(view))
    }
}
