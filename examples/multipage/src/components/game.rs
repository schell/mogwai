use mogwai::prelude::*;
use web_sys::MouseEvent;

pub enum CellUpdate {
    All {
        cells: Vec<Vec<String>>,
    },
    Single {
        row: usize,
        column: usize,
        value: String,
    },
}

#[derive(Copy, Clone, Debug)]
pub struct CellInteract {
    pub row: usize,
    pub column: usize,
    pub flag: bool,
}

impl CellInteract {
    fn new(coords: (usize, usize), event: &Event) -> Self {
        let (column, row) = coords;
        let event: Option<&MouseEvent> = event.dyn_ref();
        CellInteract {
            row,
            column,
            flag: event.map(|e| e.alt_key() || e.ctrl_key()).unwrap_or(false),
        }
    }
}

/// Create a `<td>` representing a single game cell. `coords` are expected to be `(column, row)` or
/// `(x, y)` in the grid. Interactions are transmitted to `tx` and new text to display is received
/// by `rx`.
#[allow(unused_braces)]
fn board_cell<'a>(
    coords: (usize, usize, String),
    tx: &Transmitter<CellInteract>,
    rx: &Receiver<CellUpdate>,
) -> ViewBuilder<HtmlElement> {
    let (col, row, initial_value) = coords;
    let rx_text = rx.branch_filter_map(move |update| match update {
        CellUpdate::All { cells } => cells
            .get(row)
            .map(|r| r.get(col))
            .flatten()
            .map(|s| s.to_owned()),
        CellUpdate::Single {
            row: y,
            column: x,
            value,
        } if *x == col && *y == row => Some(value.to_owned()),
        _ => None,
    });
    builder! {
        <td
            valign="middle"
            align="center"
            style="height: 50px; width: 50px; border: 1px solid black;"
            on:click=tx.contra_map(move |event: &Event| CellInteract::new((col, row), event))
        >
            // Cells initialize to empty but may update if revealed or clicked
            {(initial_value, rx_text)}
        </td>
    }
}

/// Create a `<tr>` representing a row of game cells. Interactions are transmitted to `tx` and new
/// text to display is received by `rx`.
fn board_row<'a>(
    row: usize,
    initial_cells: Vec<String>,
    tx: &Transmitter<CellInteract>,
    rx: &Receiver<CellUpdate>,
) -> ViewBuilder<HtmlElement> {
    let children = initial_cells
        .into_iter()
        .enumerate()
        .map(|(col, value)| board_cell((col, row, value), tx, rx));
    let mut tr = builder! { <tr /> };
    for child in children {
        tr.with(child);
    }
    tr
}

pub fn board<'a>(
    cells: Vec<Vec<String>>,
    tx: &Transmitter<CellInteract>,
    rx: &Receiver<CellUpdate>,
) -> ViewBuilder<HtmlElement> {
    let children = cells
        .into_iter()
        .enumerate()
        .map(|(row, cells)| board_row(row, cells, tx, rx));
    let mut tbody: ViewBuilder<HtmlElement> = builder! { <tbody /> };
    for child in children {
        tbody.with(child);
    }
    tbody
}
