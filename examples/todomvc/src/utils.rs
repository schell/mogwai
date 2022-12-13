use mogwai_dom::event::JsDomEvent;
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, HtmlInputElement};

pub fn input_value(input: &HtmlElement) -> Option<String> {
    let input: &HtmlInputElement = input.unchecked_ref();
    Some(input.value().trim().to_string())
}

pub fn event_input(ev: JsDomEvent) -> Option<HtmlInputElement> {
    let ev = ev.browser_event()?;
    let target = ev.target()?;
    let input: &HtmlInputElement = target.unchecked_ref();
    Some(input.clone())
}

pub fn event_input_value(ev: JsDomEvent) -> Option<String> {
    let input = event_input(ev)?;
    Some(input.value().trim().to_string())
}
