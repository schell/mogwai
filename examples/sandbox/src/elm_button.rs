use web_sys::HtmlElement;
use mogwai::component::{subscriber::Subscriber, Component};
use mogwai::gizmo::Gizmo;
use mogwai::gizmo::html::button;
use mogwai::txrx::{Receiver, Transmitter};

pub struct Button {
  pub clicks: i32,
}

#[derive(Clone)]
pub enum ButtonIn {
  Click,
}

#[derive(Clone)]
pub enum ButtonOut {
  Clicks(i32),
}

impl Component for Button {
  type ModelMsg = ButtonIn;
  type ViewMsg = ButtonOut;
  type DomNode = HtmlElement;

  fn update(
    &mut self,
    msg: &ButtonIn,
    tx_view: &Transmitter<ButtonOut>,
    _subscriber: &Subscriber<ButtonIn>,
  ) {
    match msg {
      ButtonIn::Click => {
        self.clicks += 1;
        tx_view.send(&ButtonOut::Clicks(self.clicks))
      }
    }
  }

  fn view(
    &self,
    tx: Transmitter<ButtonIn>,
    rx: Receiver<ButtonOut>,
  ) -> Gizmo<HtmlElement> {
    button()
      .rx_text(
        "Clicked 0 times",
        rx.branch_map(|msg| match msg {
          ButtonOut::Clicks(n) => format!("Clicked {} times", n),
        }),
      )
      .tx_on("click", tx.contra_map(|_| ButtonIn::Click))
  }
}
