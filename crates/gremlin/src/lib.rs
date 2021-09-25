use async_channel::{Sender, TrySendError, bounded};
use futures::stream::StreamExt;
use log::{info, Level};
use std::panic;
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{Document, HtmlElement, Window};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

pub mod var;
pub mod sleep;
pub mod dom;
pub mod patch;

/// Add an event listener of the given name to the given target. When the event happens, the
/// event will be sent on the given sender. If the sender is closed, the listener will be removed.
pub fn add_event<T, F>(
    ev_name: &str,
    target: &web_sys::EventTarget,
    tx: Sender<T>,
    mut f:F,
) -> Closure<dyn FnMut(JsValue)>
where
    T: 'static,
    F: FnMut(web_sys::Event) -> T + 'static
{
    let closure = Closure::wrap(Box::new(move |val: JsValue| {
        let ev = val.unchecked_into();
        match tx.try_send(f(ev)) {
            Ok(_) => {}
            Err(err) => match err {
                TrySendError::Full(_) => panic!("event handler Sender is full"),
                TrySendError::Closed(_) => todo!("remove the event listener"),
            },
        }
    }) as Box<dyn FnMut(JsValue)>);

    target
        .add_event_listener_with_callback(ev_name, closure.as_ref().unchecked_ref())
        .unwrap_throw();

    closure
}

pub enum FromButtonMsg {
    Click,
    MouseOver,
    MouseOut,
}

pub struct View<T> {
    pub raw: T,
}

impl<T: Clone + 'static> View<T> {
    pub fn set_stream<S, F, A>(&self, mut setter: S, f:F)
    where
        S: futures::Stream<Item = A> + Unpin + 'static,
        F: Fn(&T, A) + 'static,
    {
        let t = self.raw.clone();
        wasm_bindgen_futures::spawn_local(async move {
            loop {
                match setter.next().await {
                    Some(msg) => {
                        f(&t, msg);
                    }
                    None => {
                        break;
                    }
                }
            }
        });
    }
}

pub async fn run() -> Result<(), JsValue> {
    info!("hello");
    let window: Window = web_sys::window().unwrap();
    let document: Document = window.document().unwrap();
    let el: HtmlElement = document
        .create_element("button")
        .unwrap()
        .dyn_into::<HtmlElement>()
        .unwrap();
    el.set_inner_text("0 clicks");
    let (click_tx, click_rx) = bounded(1);
    let _click_cb = add_event("click", &el, click_tx, |_| FromButtonMsg::Click);
    let (over_tx, over_rx) = bounded(1);
    let _over_cb = add_event("mouseover", &el, over_tx, |_| FromButtonMsg::MouseOver);
    let (out_tx, out_rx) = bounded(1);
    let _out_cb = add_event("mouseout", &el, out_tx, |_| FromButtonMsg::MouseOut);

    let mut from_btn_msgs = futures::stream::select_all(vec![
        click_rx,
        over_rx,
        out_rx,
    ]);

    let view:View<HtmlElement> = View {
        raw: el.clone()
    };
    info!("view created");
    let (btn_text_tx, btn_text_rx) = bounded::<String>(1);
    view.set_stream(btn_text_rx, |v, s| v.set_inner_text(s.as_str()));
    let (btn_color_tx, btn_color_rx) = bounded::<String>(1);
    view.set_stream(btn_color_rx, |v, s| v.style().set_property("background-color", s.as_str()).unwrap());
    info!("updates bound to view");

    document.body().unwrap().append_child(&el)?;
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
                btn_text_tx.send(format!("{} clicks", clicks)).await.unwrap();
            }
            Some(FromButtonMsg::MouseOver) => {
                btn_text_tx.send(format!("click me")).await.unwrap();
            }
            Some(FromButtonMsg::MouseOut) => {
                btn_text_tx.send(format!("{} clicks", clicks)).await.unwrap();
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
    use wasm_bindgen_test::*;
    use super::*;

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
