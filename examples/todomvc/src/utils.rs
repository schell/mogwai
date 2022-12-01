use mogwai::dom::event::DomEvent;
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, HtmlInputElement};

pub fn input_value(input: &HtmlElement) -> Option<String> {
    let input: &HtmlInputElement = input.unchecked_ref();
    Some(input.value().trim().to_string())
}

pub fn event_input_value(ev: JsDomEvent) -> Option<String> {
    let ev = ev.browser_event()?;
    let target = ev.target()?;
    let input: &HtmlInputElement = target.unchecked_ref();
    Some(input.value().trim().to_string())
}
