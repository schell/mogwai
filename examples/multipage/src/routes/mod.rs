use mogwai::prelude::*;

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
