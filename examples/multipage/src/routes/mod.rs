mod game_list;

use crate::api;
use mogwai::prelude::*;

pub use game_list::GameList;

/// Defines a button that changes its text every time it is clicked.
/// Once built, the button will also transmit clicks into the given transmitter.
#[allow(unused_braces)]
fn new_button_view(tx_click: Transmitter<Event>) -> ViewBuilder<HtmlElement> {
    // Create a receiver for our button to get its text from.
    let rx_text = Receiver::<String>::new();

    // Create the button that gets its text from our receiver.
    //
    // The button text will start out as "Click me" and then change to whatever
    // comes in on the receiver.
    let button = builder! {
        // The button has a style and transmits its clicks
        <button style="cursor: pointer;" on:click=tx_click.clone()>
            // The text starts with "Click me" and receives updates
            {("Click me", rx_text.branch())}
        </button>
    };

    // Now that the routing is done, we can define how the signal changes from
    // transmitter to receiver over each occurance.
    // We do this by wiring the two together, along with some internal state in the
    // form of a fold function.
    tx_click.wire_fold(
        &rx_text,
        true, // our initial folding state
        |is_red, _| {
            let out = if *is_red {
                "Turn me blue".into()
            } else {
                "Turn me red".into()
            };

            *is_red = !*is_red;
            out
        },
    );

    button
}

fn stars() -> ViewBuilder<HtmlElement> {
    builder! {
        <div className="three-stars">
            <span>"★"</span>
            <span>"★"</span>
            <span>"★"</span>
        </div>
    }
}

#[allow(unused_braces)]
fn star_title(rx_org: Receiver<String>) -> ViewBuilder<HtmlElement> {
    let org_name = rx_org.branch_map(|org| format!("from {}?", org));
    builder! {
        <div class="title-component uppercase">
            {stars()}
            <div class="title-component__description">
                <span class="strike-preamble">"Did contributions come"</span>
                <span class="strike-out">{("from you?", org_name)}</span>
            </div>
        </div>
    }
}

#[allow(unused_braces)]
pub fn game(game_id: api::GameId) -> ViewBuilder<HtmlElement> {
    use crate::components::game::{self, CellInteract, CellUpdate};
    // Create a transmitter to send button clicks into.
    let tx_game: Transmitter<api::GameState> = Transmitter::new();
    let tx_cells: Transmitter<CellInteract> = Transmitter::new();
    let rx_cell_updates = Receiver::new();
    let rx_game_board = Receiver::new();
    tx_cells.wire_map(&rx_cell_updates, |interaction| CellUpdate::Single {
        column: interaction.column,
        row: interaction.row,
        value: match interaction.flag {
            true => "F".into(),
            false => "*".into(),
        },
    });
    // Fetch the initial board state
    let game_state = api::get_game(game_id.clone());
    let tx =
        tx_game.contra_filter_map(|r: &Result<api::GameState, api::FetchError>| r.clone().ok());
    tx_game.wire_map(&rx_game_board, |game_state| CellUpdate::All {
        cells: game_state.board.clone(),
    });
    tx.send_async(game_state);
    // Set up to receive later board states
    tx_cells.spawn_recv().respond(move |interaction| {
        let input = api::GameMoveInput {
            game_id,
            column: interaction.column,
            row: interaction.row,
            move_type: match interaction.flag {
                true => api::GameMoveType::FLAG,
                false => api::GameMoveType::OPEN,
            },
        };
        tx.send_async(api::patch_game(input));
    });
    // Patch the initial board state into the game board slot
    let rx_patch_game = rx_game_board.branch_filter_map(move |update| match update {
        CellUpdate::All { cells } => Some(Patch::Replace {
            index: 0,
            value: game::board(cells.clone(), &tx_cells, &rx_cell_updates),
        }),
        _ => None,
    });
    builder! {
        <main class="container">
            <div class="overlay">
                "This site is only supported in portrait mode."
            </div>
            <div class="game-board" data-game-id=&game_id.to_hyphenated().to_string()>
                <table>
                    <slot name="game-board" patch:children=rx_patch_game>
                        <tbody />
                    </slot>
                </table>
            </div>
        </main>
    }
}

#[allow(unused_braces)]
pub fn home() -> ViewBuilder<HtmlElement> {
    // Create a transmitter to send button clicks into.
    let tx_click = Transmitter::new();
    let rx_org = Receiver::new();
    builder! {
        <main class="container">
            <div class="overlay">
                "This site is only supported in portrait mode."
            </div>
            <div class="page-one">
                <div class="section-block">
                    {star_title(rx_org)}
                    {new_button_view(tx_click)}
                </div>
            </div>
        </main>
    }
}

pub fn not_found() -> ViewBuilder<HtmlElement> {
    builder! {
        <h1>"Not Found"</h1>
    }
}
