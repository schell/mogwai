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

struct TextOps{}

impl Component for TextOps {
    type ModelMsg = ();
    type ViewMsg = ();
    type DomNode = HtmlElement;

    fn update(
        &mut self,
        _msg: &Self::ModelMsg,
        _tx_view: &Transmitter<Self::ViewMsg>,
        _sub: &Subscriber<Self::ModelMsg>,
    ) {}

    fn view(
        &self,
        _tx: &Transmitter<Self::ModelMsg>,
        _rx: &Receiver<Self::ViewMsg>,
    ) -> ViewBuilder<Self::DomNode> {
        builder! {
            <div class="frow width-100" id="textops-immutable">
                <button>"B"</button>
                <button>"I"</button>
                <button>"U"</button>
                <button>"S"</button>
            </div>
        }
    }
}

struct Editor {
    text_ops: View<HtmlElement>
}

#[derive(Clone)]
enum ToEditor {
    Focused(Event)
}

impl Component for Editor {
    type ModelMsg = ToEditor;
    type ViewMsg = ();
    type DomNode = HtmlElement;

    fn update(
        &mut self,
        msg: &Self::ModelMsg,
        _tx_view: &Transmitter<Self::ViewMsg>,
        _sub: &Subscriber<Self::ModelMsg>,
    ) {
        match msg {
            ToEditor::Focused(ev) => if let Some(target) = ev.target() {
                // here we're using the javascript API provided by web-sys
                // see https://rustwasm.github.io/wasm-bindgen/api/web_sys/index.html
                let old_el: &HtmlElement = target.dyn_ref().unwrap();
                let dom_ref = self.text_ops.dom_ref();
                let text_ops_node: &Node = dom_ref.as_ref();
                old_el.prepend_with_node_1(text_ops_node).unwrap();
            }
        }
    }

    fn view(
        &self,
        tx: &Transmitter<Self::ModelMsg>,
        rx: &Receiver<Self::ViewMsg>,
    ) -> ViewBuilder<Self::DomNode> {
        builder! {
            <section class="frow direction-column">
                <div
                    id="editor"
                    on:focusin=tx.contra_map(|ev: &Event| ToEditor::Focused(ev.clone()))
                    class="frow direction-column width-100" data-block-editor="browser-wasm">
                    <div contenteditable="true" class="frow direction-column width-100 row-center" data-block="heading1">
                        <div>"This is heading 1"</div>
                    </div>
                    <div contenteditable="true" class="frow direction-column width-100 row-center" data-block="heading1">
                        <div>"This is heading 2"</div>
                    </div>
                </div>
            </section>
        }
    }
}

#[wasm_bindgen]
pub fn main(parent_id: Option<String>) -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    let text_ops = View::from(Gizmo::from(TextOps{}));
    let editor = Gizmo::from(Editor{ text_ops });
    let view = View::from(editor.view_builder());
    if let Some(id) = parent_id {
        let parent = utils::document()
            .get_element_by_id(&id)
            .unwrap();
        view.run_in_container(&parent)
    } else {
        view.run()
    }
}
