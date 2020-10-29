#![allow(unused_braces)]

mod elm_button;

use log::Level;
use mogwai::prelude::*;
use mogwai_hydrator::Hydrator;
use std::panic;
use wasm_bindgen::prelude::*;
use web_sys::{Request, RequestInit, RequestMode, Response};

/// Defines a button that changes its text every time it is clicked.
/// Once built, the button will also transmit clicks into the given transmitter.
pub fn new_button_view(tx_click: Transmitter<Event>) -> View<HtmlElement> {
    // Create a receiver for our button to get its text from.
    let rx_text = Receiver::<String>::new();

    // Create the button that gets its text from our receiver.
    //
    // The button text will start out as "Click me" and then change to whatever
    // comes in on the receiver.
    let button = view! {
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

/// Creates a h1 heading that changes its color.
pub fn new_h1_view(tx_click: Transmitter<Event>) -> View<HtmlElement> {
    // Create a receiver for our heading to get its color from.
    let rx_color = Receiver::<String>::new();

    // Create the view for our heading, giving it the receiver.
    let h1 = view! {
        <h1 id="header" class="my-header"
            // set style.color with an initial value and then update it whenever
            // we get a message on rx_color
            style:color=("green", rx_color.branch())
            >
            "Hello from mogwai!"
        </h1>
    };

    // Now that the view is done, let's define the logic
    // The h1's color will change every click back and forth between blue and red
    // after the initial green.
    tx_click.wire_fold(
        &rx_color,
        false, // the intial value for is_red
        |is_red, _| {
            let out = if *is_red { "blue".into() } else { "red".into() };
            *is_red = !*is_red;
            out
        },
    );

    h1
}

async fn request_to_text(req: Request) -> Result<String, String> {
    let resp: Response = JsFuture::from(window().fetch_with_request(&req))
        .await
        .map_err(|_| "request failed".to_string())?
        .dyn_into()
        .map_err(|_| "response is malformed")?;
    let text: String = JsFuture::from(resp.text().map_err(|_| "could not get response text")?)
        .await
        .map_err(|_| "getting text failed")?
        .as_string()
        .ok_or_else(|| "couldn't get text as string".to_string())?;
    Ok(text)
}

async fn click_to_text() -> Option<String> {
    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::Cors);

    let req = Request::new_with_str_and_init(
        "https://worldtimeapi.org/api/timezone/Europe/London.txt",
        &opts,
    )
    .unwrap_throw();

    let result = match request_to_text(req).await {
        Ok(s) => s,
        Err(s) => s,
    };
    Some(result)
}

/// Creates a button that when clicked requests the time in london and sends
/// it down a receiver.
pub fn time_req_button_and_pre() -> View<HtmlElement> {
    let (req_tx, req_rx) = txrx::<Event>();
    let (resp_tx, resp_rx) = txrx::<String>();

    req_rx.forward_filter_fold_async(
        &resp_tx,
        false,
        |is_in_flight: &mut bool, _| {
            // When we receive a click event from the button and we're not already
            // sending a request, we'll set one up and send it.
            if !*is_in_flight {
                // Change the state to tell later invocations that a request is in
                // flight
                *is_in_flight = true;
                // Return a future to be excuted which possibly produces a value to
                // send downstream to resp_tx
                wrap_future(async { click_to_text().await })
            } else {
                // Don't change the state and don't send anything downstream to
                // resp_tx
                None
            }
        },
        |is_in_flight, _| {
            // the cleanup function reports that the request is no longer in flight
            *is_in_flight = false;
        },
    );

    view! {
        <div>
            <button
                style="cursor: pointer;"
                on:click=req_tx >

                "Get the time (london)"
            </button>
            <pre>{("(waiting)", resp_rx)}</pre>
        </div>
    }
}

/// Creates a view that ticks a count upward every second.
pub fn counter() -> View<HtmlElement> {
    // Create a transmitter to send ticks every second
    let tx = Transmitter::<()>::new();

    // Create a receiver for a string to accept our counter's text
    let rx = Receiver::<String>::new();

    let timeout_tx = tx.clone();
    timeout(1000, move || {
        // Once a second send a unit down the pipe
        timeout_tx.send(&());
        // Always reschedule this timeout
        true
    });

    // Wire the tx to the rx using a fold function
    tx.wire_fold(&rx, 0, |n: &mut i32, &()| {
        *n += 1;
        format!("Count: {}", *n)
    });

    view! { <h3>{("Awaiting first count", rx)}</h3> }
}

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap_throw();

    // Create a transmitter to send button clicks into.
    let tx_click = Transmitter::new();
    let h1 = new_h1_view(tx_click.clone());
    let btn = new_button_view(tx_click);
    let req = time_req_button_and_pre();
    let counter = counter();

    // Put it all in a parent view and run it right now
    let root = view! {
        <div>
            {h1}
            {btn}
            {Gizmo::from(elm_button::Button { clicks: 0 })}
            <br />
            <br />
            {req}
            {counter}
        </div>
    };
    root.run().unwrap_throw();

    // Here we'll start a hydration-by-hand experiment.
    let body: HtmlElement = utils::document().body().unwrap_throw();
    {
        // First we'll create some non-mogwai managed DOM using web_sys:
        let section = utils::document()
            .create_element("section")
            .unwrap_throw()
            .dyn_into::<HtmlElement>()
            .unwrap_throw();
        section.set_inner_html(r#"<div id="my_div"><p class="my_p">This is pre-existing text that will be hydrated</p></div>"#);

        body.append_child(&section).unwrap_throw();
    }

    // Now we'll attempt to hydrate a view from the pre-existing DOM and then
    // update the inner text of the child `p` node.
    let (tx, rx) = txrx_fold(0, |count: &mut u32, _: &()| -> String {
        *count += 1;
        if *count == 1 {
            "Sent 1 message".into()
        } else {
            format!("Sent {} messages", *count)
        }
    });
    {
        let builder = builder! {
            <div id="my_div">
                <p class="my_p">{("blah", rx)}</p>
            </div>
        };
        let hydrator = Hydrator::from(builder);
        let view = View::try_from(hydrator).unwrap();
        view.forget().unwrap_throw();
    };

    tx.send(&());
    tx.send(&());
    tx.send(&());
    utils::timeout(3000, move || {
        tx.send(&());
        true
    });

    Ok(())
}
