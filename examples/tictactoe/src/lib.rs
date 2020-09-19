use mogwai::prelude::*;

use wasm_bindgen::prelude::*;

use log::*;
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

type Board = [&'static str; 9];

fn board() -> View<HtmlElement> {

    let board = [""; 9];
    let shared_board = new_shared(board);

    let (tx, rx) = txrx();
    rx.respond_shared(shared_board.clone(), |b: &mut Board, s: &u8| {
        info!("mark {}", s);
        b[*s as usize] = "X";
        info!("{:?}", b)
    });

    view! {
        <div>
            <div class="status"> "Next player: X" </div>
            <div class="board-row">
                { square(0, tx.clone()) }
                { square(1, tx.clone()) }
                { square(2, tx.clone()) }
            </div>
            <div class="board-row">
                { square(3, tx.clone()) }
                { square(4, tx.clone()) }
                { square(5, tx.clone()) }
            </div>
            <div class="board-row">
                { square(6, tx.clone()) }
                { square(7, tx.clone()) }
                { square(8, tx.clone()) }
            </div>
        </div>
    }
}

fn square(i: u8, board_tx: Transmitter<u8>) -> View<HtmlElement> {
    let (tx, rx) = txrx_map(move |_| "X".to_string());
    rx.branch().forward_map(&board_tx, move |_| i);

    view! {
        <button class="square" on:click=tx>
            { ("", rx) }
        </button>
    }
}