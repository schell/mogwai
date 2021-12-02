#![allow(unused_braces)]
use log::Level;
use mogwai::prelude::*;
use std::panic;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

struct TextOps {}

impl TextOps {
    fn component() -> Component<Dom> {
        Component::from(builder! {
            <div class="frow width-100" id="textops-immutable">
                <button>"B"</button>
                <button>"I"</button>
                <button>"U"</button>
                <button>"S"</button>
            </div>
        })
    }
}

#[derive(Clone)]
struct FocusedOn(Dom);

impl FocusedOn {
    fn from_event(dom_ev: DomEvent) -> Option<FocusedOn> {
        let ev:web_sys::Event = dom_ev.browser_event()?;
        if let Some(target) = ev.target() {
            // here we're using the javascript API provided by web-sys
            // see https://rustwasm.github.io/wasm-bindgen/api/web_sys/index.html
            let focused_el = target.dyn_ref::<HtmlElement>()?;
            let focused_el: HtmlElement = focused_el.clone();
            let focused_dom = Dom::try_from(JsValue::from(focused_el)).ok()?;
            Some(FocusedOn(focused_dom))
        } else {
            None
        }
    }
}

fn editor_component() -> Component<Dom> {
    let text_ops = TextOps::component().build().unwrap();
    let (tx_logic, mut rx_logic) = broadcast::bounded::<FocusedOn>(1);

    Component::from(
        builder! {
            <section class="frow direction-column">
                <div
                 id="editor"
                 on:focusin=tx_logic.sink().contra_filter_map(|ev: DomEvent| FocusedOn::from_event(ev))
                 class="frow direction-column width-100" data-block-editor="browser-wasm">
                    <div contenteditable="true" class="frow direction-column width-100 row-center" data-block="heading1">
                        <div>"This is heading 1"</div>
                    </div>
                    <div contenteditable="true" class="frow direction-column width-100 row-center" data-block="heading1">
                        <div>"This is heading 2"</div>
                    </div>
                </div>
            </section>
    })
    .with_logic(async move {
        loop {
            match rx_logic.next().await {
                Some(FocusedOn(dom)) => {
                    if let Some(focused_node) = dom.clone_as::<HtmlElement>() {
                        if let Some(text_ops_node) = text_ops.inner.clone_as::<Node>() {
                            focused_node.prepend_with_node_1(&text_ops_node).unwrap();
                        }
                    }
                }
                None => break,
            }
        }
    })
}

#[wasm_bindgen]
pub fn main(parent_id: Option<String>) -> Result<(), JsValue> {
panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    let editor_view = editor_component().build().unwrap();
    if let Some(id) = parent_id {
        let parent = utils::document().get_element_by_id(&id).unwrap();
        editor_view.run_in_container(&parent)
    } else {
        editor_view.run()
    }
}
