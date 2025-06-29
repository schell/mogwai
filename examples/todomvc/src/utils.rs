use mogwai::web::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Event, HtmlElement, HtmlInputElement};

pub fn input_value(input: &HtmlElement) -> Option<String> {
    let input: &HtmlInputElement = input.unchecked_ref();
    Some(input.value().trim().to_string())
}

pub fn event_input(ev: &Event) -> Option<HtmlInputElement> {
    let target = ev.target()?;
    let input: &HtmlInputElement = target.unchecked_ref();
    Some(input.clone())
}

pub fn event_input_value(ev: &Event) -> Option<String> {
    let input = event_input(ev)?;
    Some(input.value().trim().to_string())
}
