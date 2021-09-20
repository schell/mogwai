#[allow(unused_braces)]
#[cfg(test)]
mod gremlin_tests {
    use wasm_bindgen_test::*;
    use mogwai_chan::{model::*, *};
    use web_sys::HtmlElement;

    struct Counter {
        model: Model<u32>,
        view: View<HtmlElement>,
    }

    enum MsgFromView {
        CountInc
    }

    enum MsgToView {
        CountChanged(u32)
    }

    fn view_builder(model: &Model<u32>) -> ViewBuilder<HtmlElement> {
        builder! {
            <fieldset>
                <legend>"Counter"</legend>
                <div class="title">
                    {model.receiver().branch_map(|count| format!("{} clicks", count))}
                </div>
                <button on:click=tx.contra_map(|_| MsgFromView::CountInc)>"+1"</button>
            </fieldset>
        }
    }

    impl Counter {
        /// Wait for an increment from the user and return the new count.
        pub async fn get_inc(&self) -> u32 {
            self.model.receiver().recv().await
        }
    }

    #[tokio::test]
    async fn main_test() {
        let model = Model::new(0);
        let tx_model = model.clone();
        let tx: Transmitter<MsgFromView> = Transmitter::new();
        let view = View::from(view_builder(&tx, &model.receiver().branch_map(|count| MsgToView::CountChanged(*count))));
        body().append_child(&view.dom_ref()).unwrap();
        let counter = Counter {
            model,
            view
        };

        let inc = counter.get_inc().await;
        assert_eq!(inc, 1);
    }
}
