//! Server-side rendering demo.
use demo::button_clicks::{ButtonClicks, ButtonClicksView};
use mogwai_futura::ssr::prelude::*;

fn main() {
    let mut button_clicks = ButtonClicks { clicks: 0 };
    let view = ButtonClicksView::<Ssr>::default();
    let click = view.button_click.clone();

    std::thread::spawn({
        let view = view.clone();
        move || futures_lite::future::block_on(button_clicks.run(view))
    });

    let init_html_string = view.wrapper.html_string();
    futures_lite::future::block_on(async {
        click.fire().await;
        click.fire().await;
        click.fire().await;
    });
    let final_html_string = view.wrapper.html_string();
    println!("init: {init_html_string}");
    println!();
    println!("final: {final_html_string}");
}
