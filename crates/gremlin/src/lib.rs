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
pub mod channel;
pub mod event;
pub mod model;
pub mod patch;
pub mod sleep;
pub mod var;
pub mod view;

use crate::{
    builder::ViewBuilder,
    channel::{bounded, SinkExt, StreamExt},
    patch::{HashPatch, ListPatch},
    view::View,
};

#[derive(Clone)]
pub enum FromButtonMsg {
    Click,
    MouseOver,
    MouseOut,
}

pub async fn run() -> Result<(), JsValue> {
    info!("hello");

    let (mut btn_text_tx, btn_text_rx) = bounded::<String>(1);
    let (mut btn_color_tx, btn_color_rx) = bounded::<String>(1);
    let (btn_out_tx, mut btn_out_rx) = bounded::<FromButtonMsg>(1);

    info!("button text created");

    //builder! {
    //    <button
    //     style:background-color = btn_color_rx
    //     on:click = btn_out_tx.contra_map(|_| FromButtonMsg::Click)
    //     on:mouseover = btn_out_tx.contra_map(|_| FromButtonMsg::MouseOver)
    //     on:mouseout = btn_out_tx.contra_map(|_| FromButtonMsg::MouseOut)>
    //        {("0 clicks", btn_text_rx)}
    //    </button>
    //}
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
        .with_event(
            "click",
            btn_out_tx
                .clone()
                .with(|_| async { Ok(FromButtonMsg::Click) }),
        )
        .with_event(
            "mouseover",
            btn_out_tx
                .clone()
                .with(|_| async { Ok(FromButtonMsg::MouseOver) }),
        )
        .with_event(
            "mouseout",
            btn_out_tx.with(|_| async { Ok(FromButtonMsg::MouseOut) }),
        );

    let button: View<HtmlElement> = button_builder.try_into().unwrap();
    info!("button created");

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
        info!("waiting for user input");
        match btn_out_rx.next().await {
            Some(FromButtonMsg::Click) => {
                clicks += 1;
                match clicks {
                    30 => break,
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
