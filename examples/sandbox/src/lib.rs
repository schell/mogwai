#![allow(unused_braces)]

mod relay_button;

use log::Level;
use mogwai::dom::utils;
use mogwai::prelude::*;
use mogwai_hydrator::Hydrator;
use std::{convert::TryFrom, panic};
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlElement;
#[cfg(target_arch = "wasm32")]
use web_sys::{Request, RequestInit, RequestMode, Response};

/// Defines a button that changes its text every time it is clicked.
/// Once built, the button will also transmit clicks into the given transmitter.
pub fn new_button(click_chan: &broadcast::Channel<()>) -> ViewBuilder<JsDom> {
    // Create a channel for our button to get its text from.
    let (tx_text, rx_text) = broadcast::bounded::<String>(1);

    // Create the button view builder that gets its text from our receiver.
    //
    // The button text will start out as "Click me" and then change to whatever
    // comes in on the receiver.
    let view = html! {
        // The button has a style and transmits its clicks
        <button
         style="cursor: pointer;"
         on:click=click_chan.sender().contra_map(|_| ())>
            // The text starts with "Click me" and receives updates
            {("Click me", rx_text)}
        </button>
    };

    // Now that the view routing is done, we can define how the signal changes from
    // transmitter to receiver over each occurance.
    // We do this by wiring the two channels together, along with some internal state in the
    // form of a logic loop which we spawn into the target's runtime.
    let mut rx_click = click_chan.receiver();
    let logic = async move {
        let mut is_red = true;
        loop {
            match rx_click.next().await {
                Some(()) => {
                    tx_text
                        .broadcast(
                            if is_red {
                                "Turn me blue"
                            } else {
                                "Turn me red"
                            }
                            .to_string(),
                        )
                        .await
                        .unwrap();
                    is_red = !is_red;
                }
                None => break,
            }
        }
    };

    // Bundle them together to use later in a tree of widgets
    view.with_task(logic)
}

/// Creates a h1 heading that changes its color.
pub fn new_h1(click_chan: &broadcast::Channel<()>) -> ViewBuilder<JsDom> {
    // Create a receiver for our heading to get its color from.
    let (tx_color, rx_color) = broadcast::bounded::<String>(1);

    // Create the builder for our view, giving it the receiver.
    let builder = html! {
        <h1 id="header" class="my-header"
         // set style.color with an initial value and then update it whenever
         // we get a message on rx_color
         style:color=("green", rx_color.clone())>
            "Hello from mogwai!"
        </h1>
    };

    // Now that the view builder is done, let's define the logic
    // The h1's color will change every click back and forth between blue and red
    // after the initial green.
    let mut rx_click = click_chan.receiver();
    let logic = async move {
        let mut is_red = false;
        loop {
            match rx_click.next().await {
                Some(()) => {
                    let msg = if is_red { "blue" } else { "red" }.to_string();
                    tx_color.broadcast(msg).await.unwrap();
                    is_red = !is_red;
                }
                None => break,
            }
        }
    };

    // Wrap it up into a component
    builder.with_task(logic)
}

#[cfg(target_arch = "wasm32")]
async fn request_to_text(req: Request) -> Result<String, String> {
    let resp: Response = JsFuture::from(utils::window().fetch_with_request(&req))
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

#[cfg(target_arch = "wasm32")]
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
#[cfg(not(target_arch = "wasm32"))]
async fn click_to_text() -> Option<String> {
    None
}

/// Creates a button that when clicked requests the time in london and sends
/// it down a receiver.
pub fn time_req_button_and_pre() -> ViewBuilder<JsDom> {
    // In the time it takes to send a request to the time server and get a response
    // back, the user can click the button again and again, so we'll let the channel
    // handle more than one click at a time by increasing our bounds a bit.
    let (req_tx, req_rx) = broadcast::bounded::<()>(5);
    let (resp_tx, resp_rx) = broadcast::bounded::<String>(1);

    html! (
        <div>
            <button
            style="cursor: pointer;"
            on:click=req_tx.contra_map(|_| ()) >

                "Get the time (london)"
            </button>
            <pre>{("(waiting)", resp_rx)}</pre>
        </div>
    )
    .with_task(async move {
        // Here we use `ready_chunks`, which batches all the messages that are waiting
        // in the channel (max capacity).
        let mut rx = req_rx.ready_chunks(5);
        loop {
            match rx.next().await {
                Some(_) => {
                    if let Some(london_time_text) = click_to_text().await {
                        resp_tx.broadcast(london_time_text).await.unwrap();
                    }
                }
                None => break,
            }
        }
    })
}

/// Creates a view that ticks a count upward every second.
pub fn counter() -> ViewBuilder<JsDom> {
    // Create a channel for a string to accept our counter's text
    let (tx_txt, rx_txt) = broadcast::bounded::<String>(1);

    rsx! (
        h3() {
            {("Awaiting first count", rx_txt)}
        }
    )
    // The logic loop waits a second and then increments a counter,
    // then send a message.
    .with_task(async move {
        let mut n = 0;
        loop {
            let _ = wait_millis(1000).await;
            n += 1;
            let msg = format!("Count: {}", n);
            tx_txt.broadcast(msg).await.unwrap();
        }
    })
}

#[wasm_bindgen(start)]
pub fn main() {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap_throw();

    mogwai::spawn(async {
        // Create a channel to send button clicks into.
        let click_chan = broadcast::Channel::new(1);
        let h1 = new_h1(&click_chan);
        let btn = new_button(&click_chan);
        let req = time_req_button_and_pre();
        let counter = counter();

        // Put it all in a parent view and run it right now
        let root = html! {
            <div>
                {h1}
                {btn}
                // Since Button can be converted into ViewBuilder<JsDom>, we can plug
                // it right into the DOM tree
                {relay_button::Button::default()}
                <br />
                    <br />
                {req}
                {counter}
            </div>
        }
        .build()
        .unwrap();
        root.run().unwrap_throw();

        // Here we'll start a hydration-by-hand experiment.
        let body: JsDom = utils::body();
        {
            // First we'll create some non-mogwai managed DOM using web_sys:
            let body: web_sys::HtmlElement = body.clone_as().unwrap_throw();
            let document: web_sys::Document = utils::document().unwrap_js();
            let section = document
                .create_element("section")
                .unwrap_throw()
                .dyn_into::<HtmlElement>()
                .unwrap_throw();
            section.set_inner_html(r#"<div id="my_div"><p data-count="42" class="my_p">This is pre-existing text that will be hydrated</p></div>"#);

            body.append_child(&section).unwrap_throw();
        }

        // Now we'll attempt to hydrate a view from the pre-existing DOM and then
        // update the view.

        // We will need a channel to send view messages.
        let (tx_view, rx_view) = broadcast::bounded::<u32>(1);
        // Create a channel for getting the count state from the DOM after hydration.
        let (tx_p, mut rx_p) = mpsc::bounded(1);
        // Create a builder that matches the pre-existing DOM (this builder would be how we create it server-side).
        let builder = html! {
            <div id="my_div">
                <p
                data-count=rx_view.clone().map(|n| format!("{}", n))
                capture:view=tx_p
                class="my_p">
            {("Waiting",
              rx_view.map(|n| if n == 1 {
                  "Sent 1 message".to_string()
              } else {
                  format!("Sent {} messages", n)
              })
            )}
            </p>
                </div>
        };
        // Create a channel for driving updates.
        let (tx, mut rx) = broadcast::bounded::<()>(1);
        let logic = async move {
            // we can get the stored count from the DOM
            let mut count = {
                // first get the p tag
                let dom: JsDom = rx_p.next().await.unwrap();
                let s = dom.get_attribute("data-count").unwrap().unwrap();
                s.parse::<u32>().unwrap()
            };
            loop {
                match rx.next().await {
                    Some(()) => {
                        count += 1;
                        tx_view.broadcast(count).await.unwrap();
                    }
                    None => break,
                }
            }
        };
        let component = builder.with_task(logic);

        {
            let hydrator = Hydrator::try_from(component).unwrap();
            // Since this view is already attached, we don't have to `run` it.
            // Instead we can forget about it.
            let _ = JsDom::from(hydrator);
        };

        // Pump the hydrated widget a few times.
        let mut times = 3;
        while times > 0 {
            tx.broadcast(()).await.unwrap();
            times -= 1;
        }

        // Since we hydrated the count as 42, then sent 3 more messages,
        // the count should now be 45.
    });
}
