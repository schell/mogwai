//! A low cost intermediate structure for creating views either by
//! hydration from the DOM or by creating a fresh view from scratch.
//!
//! Here we attempt to have our cake and eat it too.
pub use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
use web_sys::Node;
pub use web_sys::{Element, Event, EventTarget, HtmlInputElement, Text};

use crate::{
    prelude::{Effect, Error, HydrateView, Receiver, Transmitter, TryFrom, View},
    view::{interface::*, hydration::ViewOnly},
};


pub struct ViewBuilder<T: JsCast> {
    view_fn: Box<dyn FnOnce() -> View<T>>,
    hydrate_fn: Box<dyn FnOnce() -> HydrateView<T>>,
}


impl<T: JsCast + 'static> ViewBuilder<T> {
    pub fn new<VF, HF>(view_fn: VF, hydrate_fn: HF) -> Self
    where
        VF: FnOnce() -> View<T> + 'static,
        HF: FnOnce() -> HydrateView<T> + 'static,
    {
        let view_fn = Box::new(view_fn);
        let hydrate_fn = Box::new(hydrate_fn);
        ViewBuilder {
            view_fn,
            hydrate_fn,
        }
    }

    pub fn append_update<VF, HF>(&mut self, view_fn: VF, hydrate_fn: HF)
    where
        VF: FnOnce(View<T>) -> View<T> + 'static,
        HF: FnOnce(HydrateView<T>) -> HydrateView<T> + 'static,
    {
        let f = std::mem::replace(&mut self.view_fn, Box::new(|| panic!()));
        self.view_fn = Box::new(move || view_fn(f()));

        let f = std::mem::replace(&mut self.hydrate_fn, Box::new(|| panic!()));
        self.hydrate_fn = Box::new(move || hydrate_fn(f()));
    }

    /// Convert this builder into a fresh [`View`].
    pub fn fresh_view(self) -> View<T> {
        (self.view_fn)()
    }

    /// Attempt to convert this builder into a [`View`] hydrated from
    /// the existing DOM.
    pub fn hydrate_view(self) -> Result<View<T>, Error> {
        let hydrate = (self.hydrate_fn)();
        let val = View::try_from(hydrate)?;
        Ok(val)
    }

    /// Attempt to convert this build into a [`View`] hydrated from
    /// the existing DOM - if that fails, create a fresh view.
    pub fn hydrate_or_else_fresh_view(self) -> View<T> {
        let hydrate = self.hydrate_fn;
        let fresh = self.view_fn;
        View::try_from(hydrate()).unwrap_or_else(|_| fresh())
    }
}


/// # [`From`] instances for [`HydrateView`]
///
/// Most of these mimic the corresponding [`From`] instances for [`View`],
/// the rest are here for the operation of this module.


impl From<Effect<String>> for ViewBuilder<Text> {
    fn from(eff: Effect<String>) -> Self {
        let view_eff = eff.clone();
        ViewBuilder::new(|| View::from(view_eff), || HydrateView::from(eff))
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
        let vtext = text.clone();
        ViewBuilder::new(|| View::from(vtext), || HydrateView::from(text))
    }
}


impl<T: JsCast + 'static> From<View<T>> for ViewBuilder<T> {
    fn from(view: View<T>) -> Self {
        ViewBuilder::new(|| view, || HydrateView::from_create_fn(|| ViewOnly.fail()))
    }
}


impl<T: JsCast + 'static> From<HydrateView<T>> for ViewBuilder<T> {
    fn from(hview: HydrateView<T>) -> Self {
        ViewBuilder::new(|| panic!("could not create a fresh view - hydrate only"), || hview)
    }
}


/// # ElementView


impl<T: JsCast + AsRef<Node> + 'static> ElementView for ViewBuilder<T> {
    fn element(tag: &str) -> Self {
        let vtag = tag.to_owned();
        let htag = tag.to_owned();
        ViewBuilder::new(
            move || View::element(&vtag),
            move || HydrateView::element(&htag),
        )
    }

    fn element_ns(tag: &str, ns: &str) -> Self {
        let vtag = tag.to_owned();
        let vns = ns.to_owned();
        let htag = tag.to_owned();
        let hns = ns.to_owned();
        ViewBuilder::new(
            move || View::element_ns(&vtag, &vns),
            move || HydrateView::element_ns(&htag, &hns),
        )
    }
}


/// # AttributeView


impl<T: JsCast + AsRef<Node> + AsRef<Element> + 'static> AttributeView for ViewBuilder<T> {
    fn attribute<E: Into<Effect<String>>>(mut self, name: &str, eff: E) -> Self {
        let vname = name.to_owned();
        let veff = eff.into();
        let hname = name.to_owned();
        let heff = veff.clone();
        self.append_update(
            move |v| v.attribute(&vname, veff),
            move |v| v.attribute(&hname, heff),
        );
        self
    }


    fn boolean_attribute<E: Into<Effect<bool>>>(mut self, name: &str, eff: E) -> Self {
        let vname = name.to_owned();
        let veff = eff.into();
        let hname = name.to_owned();
        let heff = veff.clone();
        self.append_update(
            move |v| v.boolean_attribute(&vname, veff),
            move |v| v.boolean_attribute(&hname, heff),
        );
        self
    }
}


/// # StyleView


impl<T: JsCast + AsRef<HtmlElement> + 'static> StyleView for ViewBuilder<T> {
    fn style<E: Into<Effect<String>>>(mut self, name: &str, eff: E) -> Self {
        let vname = name.to_owned();
        let veff = eff.into();
        let hname = name.to_owned();
        let heff = veff.clone();
        self.append_update(
            move |v| v.style(&vname, veff),
            move |v| v.style(&hname, heff),
        );
        self
    }
}


/// # EventTargetView


impl<T: JsCast + AsRef<EventTarget> + 'static> EventTargetView for ViewBuilder<T> {
    fn on(mut self, ev_name: &str, tx: Transmitter<Event>) -> Self {
        let vev_name = ev_name.to_owned();
        let vtx = tx.clone();
        let hev_name = vev_name.clone();
        self.append_update(move |v| v.on(&vev_name, vtx), move |v| v.on(&hev_name, tx));
        self
    }

    fn window_on(mut self, ev_name: &str, tx: Transmitter<Event>) -> Self {
        let vev_name = ev_name.to_owned();
        let vtx = tx.clone();
        let hev_name = vev_name.clone();
        self.append_update(
            move |v| v.window_on(&vev_name, vtx),
            move |v| v.window_on(&hev_name, tx),
        );
        self
    }

    fn document_on(mut self, ev_name: &str, tx: Transmitter<Event>) -> Self {
        let vev_name = ev_name.to_owned();
        let vtx = tx.clone();
        let hev_name = vev_name.clone();
        self.append_update(
            move |v| v.document_on(&vev_name, vtx),
            move |v| v.document_on(&hev_name, tx),
        );
        self
    }
}


/// # ParentView


impl<P, C> ParentView<ViewBuilder<C>> for ViewBuilder<P>
where
    P: JsCast + AsRef<Node> + 'static,
    C: JsCast + Clone + AsRef<Node> + 'static,
{
    fn with(mut self, child: ViewBuilder<C>) -> Self {
        let ViewBuilder {
            view_fn,
            hydrate_fn,
        } = child;
        self.append_update(move |v| v.with(view_fn()), move |v| v.with(hydrate_fn()));
        self
    }
}


/// # PostBuildView


impl<T: JsCast + Clone + 'static> PostBuildView for ViewBuilder<T> {
    type DomNode = T;

    fn post_build(mut self, tx: Transmitter<T>) -> Self {
        let vtx = tx.clone();
        self.append_update(move |v| v.post_build(vtx), move |v| v.post_build(tx));
        self
    }
}
