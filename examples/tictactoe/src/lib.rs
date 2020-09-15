use mogwai::prelude::*;

use wasm_bindgen::prelude::*;

use log::Level;
use std::panic;

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap_throw();

    let root = game();
    root.run()
}

fn game() -> View<HtmlElement> {
    view! {
        <div class="game">
           <div class="game-board">
               { board() }
           </div>
           <div class="game-info">
              <div> </div>
              <ol> </ol>
           </div>
        </div>
    }
}

fn board() -> View<HtmlElement> {
    view! {
        <div>
            <div class="status"> "Next player: X" </div>
            <div class="board-row">
                { square(0) }
                { square(1) }
                { square(2) }
            </div>
            <div class="board-row">
                { square(3) }
                { square(4) }
                { square(5) }
            </div>
            <div class="board-row">
                { square(6) }
                { square(7) }
                { square(8) }
            </div>
        </div>
    }
}

fn square(_i: u8) -> View<HtmlElement> {
    let (tx, rx) = txrx_map(|_| "X");

    view! {
        <button class="square" on:click=tx>
            { ("", rx) }
        </button>
    }
}