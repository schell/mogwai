use mogwai_dom::view::JsDom;
use mogwai_js_framework_benchmark::{App, Msg};

async fn mdl_create(count: usize, mdl: &mut App, doc: &JsDom) -> f64 {
    mdl.update(Msg::Create(count)).await;
    let found = mogwai_dom::core::time::wait_for(20.0, || {
        doc.clone_as::<web_sys::Document>()?
            .query_selector(&format!(
                "tbody>tr:nth-of-type({count})>td:nth-of-type(2)>a"
            ))
            .ok()
            .flatten()
    })
    .await
    .expect("cannot create");
    found.elapsed_seconds
}

async fn mdl_clear(mdl: &mut App, doc: &JsDom) -> f64 {
    mdl.update(Msg::Clear).await;
    let found = mogwai_dom::core::time::wait_for(3.0, || {
        let trs = doc
            .clone_as::<web_sys::Document>()?
            .query_selector_all("tbody>tr")
            .ok()?;
        if trs.length() == 0 {
            Some(())
        } else {
            None
        }
    })
    .await
    .expect("cannot clear");
    found.elapsed_seconds
}

pub async fn create(mdl: &mut App, doc: &JsDom, count: usize) -> f64 {
    mdl_create(count, mdl, doc).await + mdl_clear(mdl, doc).await
}
