use mogwai::prelude::*;

#[derive(Default)]
pub struct Button {
    clicks: usize,
    click: Output<()>,
    text: Input<String>,
}

impl Button {
    fn click_text(&self) -> String {
        match self.clicks {
            0 => "Clicked zero times.".to_string(),
            1 => "Clicked once.".to_string(),
            n => format!("Clicked {} times.", n),
        }
    }

    fn view(mut self) -> ViewBuilder<Dom> {
        html! (
            <button style="cursor: pointer;" on:click=self.click.sink().contra_map(|_| ())>
            {(self.click_text(), self.text.stream().unwrap())}
            </button>
        ).with_task(async move {
            while let Some(()) = self.click.get().await {
                self.clicks += 1;
                self.text
                    .set(self.click_text())
                    .await
                    .unwrap_or_else(|_| panic!("could not set text"));
            }
        })
    }
}

impl From<Button> for ViewBuilder<Dom> {
    fn from(btn: Button) -> Self {
        btn.view()
    }
}
