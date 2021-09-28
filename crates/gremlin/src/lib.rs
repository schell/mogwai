use async_channel::bounded;
use futures::stream::StreamExt;
use log::{info, Level};
use std::{convert::TryInto, panic};
use wasm_bindgen::prelude::*;
use web_sys::{Event, HtmlElement, Node, Text};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

pub mod builder;
pub mod event;
pub mod patch;
pub mod sleep;
pub mod var;
pub mod view;

use crate::{builder::ViewBuilder, patch::{HashPatch, ListPatch}, view::View};

pub enum FromButtonMsg {
    Click,
    MouseOver,
    MouseOut,
}

pub async fn run() -> Result<(), JsValue> {
    info!("hello");

    let (btn_text_tx, btn_text_rx) = bounded::<String>(1);
    let (btn_color_tx, btn_color_rx) = bounded::<String>(1);
    let (click_tx, click_rx) = bounded(1);
    let (over_tx, over_rx) = bounded(1);
    let (out_tx, out_rx) = bounded(1);

    info!("button text created");

    let button_builder: ViewBuilder<HtmlElement, Node, Event> = ViewBuilder::element("button")
        .with_child_stream(futures::stream::once(async move {
            let text: ViewBuilder<Text, Node, Event> = ViewBuilder::text(
                futures::stream::once(async { "0 clicks".to_string() })
                    .chain(btn_text_rx)
                    .boxed_local(),
            );

            ListPatch::Push(text.with_type::<Node>())
        }))
        .with_style_stream(
            btn_color_rx.map(|color| HashPatch::Insert("background-color".to_string(), color)),
        )
        .with_event("click", click_tx)
        .with_event("mouseover", over_tx)
        .with_event("mouseout", out_tx);

    let button: View<HtmlElement> = button_builder.try_into().unwrap();
    info!("button created");

    let mut from_btn_msgs = futures::stream::select_all(vec![
        click_rx.map(|_| FromButtonMsg::Click).boxed_local(),
        over_rx.map(|_| FromButtonMsg::MouseOver).boxed_local(),
        out_rx.map(|_| FromButtonMsg::MouseOut).boxed_local(),
    ]);

    web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .body()
        .unwrap()
        .append_child(&button.inner)?;
    info!("raw view added to document.body");

    let mut clicks = 0u32;
    loop {
        info!("waiting for click");
        match from_btn_msgs.next().await {
            Some(FromButtonMsg::Click) => {
                clicks += 1;
                match clicks {
                    20 => btn_color_tx.send("green".to_string()).await.unwrap(),
                    10 => btn_color_tx.send("blue".to_string()).await.unwrap(),
                    1 => btn_color_tx.send("yellow".to_string()).await.unwrap(),
                    _ => {}
                }
                btn_text_tx
                    .send(format!("{} clicks", clicks))
                    .await
                    .unwrap();
            }
            Some(FromButtonMsg::MouseOver) => {
                btn_text_tx.send(format!("click me")).await.unwrap();
            }
            Some(FromButtonMsg::MouseOut) => {
                btn_text_tx
                    .send(format!("{} clicks", clicks))
                    .await
                    .unwrap();
            }
            None => {
                break;
            }
        }
    }

    Ok(())
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(Level::Trace).unwrap();

    wasm_bindgen_futures::spawn_local(async {
        run().await.unwrap();
    });

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn channel_streams() {
        let (f32tx, f32rx) = bounded::<f32>(3);
        let f32stream = f32rx.map(|f| format!("{:.2}", f)).boxed();
        let (u32tx, u32rx) = bounded::<u32>(3);
        let u32stream = u32rx.map(|u| format!("{}", u)).boxed();

        let formatted = futures::stream::select_all(vec![u32stream, f32stream]);

        f32tx.send(1.5).await.unwrap();
        f32tx.send(2.3).await.unwrap();
        u32tx.send(666).await.unwrap();

        let mut strings: Vec<String> = formatted.take(3).collect::<Vec<_>>().await;
        strings.sort();

        assert_eq!(
            strings,
            vec!["1.50".to_string(), "2.30".to_string(), "666".to_string()]
        );
    }
}
