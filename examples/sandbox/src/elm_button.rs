use mogwai::prelude::*;

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

impl IsElmComponent for Button {
    type LogicMsg = ButtonIn;
    type ViewMsg = ButtonOut;
    type ViewNode = Dom;

    fn update(&mut self, msg: ButtonIn, tx_view: broadcast::Sender<ButtonOut>) {
        match msg {
            ButtonIn::Click => {
                self.clicks += 1;
                let out = ButtonOut::Clicks(self.clicks);
                mogwai::spawn(async move {
                    tx_view
                        .broadcast(out)
                        .await
                        .unwrap();
                });
            }
        }
    }

    fn view(
        &self,
        tx: broadcast::Sender<ButtonIn>,
        rx: broadcast::Receiver<ButtonOut>,
    ) -> ViewBuilder<Dom> {
        builder! {
            <button style="cursor: pointer;" on:click=tx.sink().with(|_| async {Ok(ButtonIn::Click)})>
                {(
                    format!("Clicked {} times", self.clicks),
                    rx.clone().map(|msg| match msg {
                        ButtonOut::Clicks(n) => format!("Clicked {} times", n),
                    })
                )}
            </button>
        }
    }
}
