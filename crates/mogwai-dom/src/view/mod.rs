//! Wrapped views.
use mogwai::view::{ViewBuilder, AnyView};
mod js_dom;

pub use js_dom::*;

mod ssr;
pub use ssr::*;

#[derive(Clone)]
pub struct Dom(mogwai::view::AnyView);

impl From<JsDom> for Dom {
    fn from(v: JsDom) -> Self {
        Dom(AnyView::new(v))
    }
}

impl From<SsrDom> for Dom {
    fn from(v: SsrDom) -> Self {
        Dom(AnyView::new(v))
    }
}

impl Dom {
    pub fn run(self) -> anyhow::Result<()> {
        if cfg!(target_arch = "wasm32") {
            let js_dom: JsDom = self.0.downcast()?;
            js_dom.run()
        } else {
            Ok(())
        }
    }
}

pub struct DomEvent(mogwai::view::AnyEvent);

pub trait DomBuilder<T> {
    fn build(self) -> anyhow::Result<T>;
}

impl DomBuilder<JsDom> for mogwai::view::ViewBuilder {
    fn build(self) -> anyhow::Result<JsDom> {
        self.try_into()
    }
}

impl DomBuilder<SsrDom> for mogwai::view::ViewBuilder {
    fn build(self) -> anyhow::Result<SsrDom> {
        self.try_into()
    }
}

impl DomBuilder<Dom> for mogwai::view::ViewBuilder {
    fn build(self) -> anyhow::Result<Dom> {
        if cfg!(target_arch = "wasm32") {
            let js_dom: JsDom = self.build()?;
            Ok(Dom(AnyView::new(js_dom)))
        } else {
            let ssr_dom: SsrDom = self.build()?;
            Ok(Dom(AnyView::new(ssr_dom)))
        }
    }
}
