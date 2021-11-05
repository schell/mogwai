use mogwai::prelude::*;

/// Defines a button that changes its text every time it is clicked.
/// Once built, the button will also transmit clicks into the given transmitter.
fn new_button_view(click_chan: broadcast::Channel<()>) -> Component<Dom> {
    // Get a receiver from the click channel
    let mut rx_click = click_chan.receiver();
    // Create a receiver for our button to get its text from.
    let (tx_text, rx_text) = broadcast::bounded(1);

    // Create the button that gets its text from our receiver.
    //
    // The button text will start out as "Click me" and then change to whatever
    // comes in on the receiver.
    Component::from(builder! {
        // The button has a style and transmits its clicks
        <button style="cursor: pointer;" on:click=click_chan.sender().sink().contra_map(|_| ())>
            // The text starts with "Click me" and receives updates
            {("Click me", rx_text)}
        </button>
    }).with_logic(async move {
        let mut is_red = true;
        loop {
            match rx_click.next().await {
                Some(()) => {
                    let out = if is_red {
                        "Turn me blue"
                    } else {
                        "Turn me red"
                    }.into();
                    is_red = !is_red;
                    tx_text.broadcast(out).await.unwrap();
                }
                None => break,
            }
        }
    })
}

fn stars() -> ViewBuilder<Dom> {
    builder! {
        <div className="three-stars">
            <span>"★"</span>
            <span>"★"</span>
            <span>"★"</span>
        </div>
    }
}

fn star_title() -> ViewBuilder<Dom> {
    builder! {
        <div class="title-component uppercase">
            {stars()}
            <div class="title-component__description">
                <span class="strike-preamble">"Did contributions come"</span>
                <span class="strike-out">"from you"</span>
            </div>
        </div>
    }
}

pub fn home() -> ViewBuilder<Dom> {
    // Create a channels to send button clicks into.
    let click_chan = broadcast::Channel::new(1);
    builder! {
        <main class="container">
            <div class="overlay">
                "This site is only supported in portrait mode."
            </div>
            <div class="page-one">
                <div class="section-block">
                    {star_title()}
                    {new_button_view(click_chan)}
                </div>
            </div>
        </main>
    }
}

pub fn not_found() -> ViewBuilder<Dom> {
    builder! {
        <h1>"Not Found"</h1>
    }
}
