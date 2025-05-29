use mogwai_futura::web::Web;
use mogwai_js_framework_benchmark::{App, AppView};

async fn app_create(
    count: usize,
    app: &mut App,
    view: &AppView<Web>,
    doc: &web_sys::Document,
) -> f64 {
    app.create(view, count);
    let found = mogwai_futura::time::wait_for(20.0, || {
        doc.query_selector(&format!(
            "tbody>tr:nth-of-type({count})>td:nth-of-type(2)>a"
        ))
        .ok()
        .flatten()
    })
    .await
    .expect("cannot create");
    found.elapsed_seconds
}

async fn app_clear(app: &mut App, view: &AppView<Web>, doc: &web_sys::Document) -> f64 {
    app.clear(view);
    let found = mogwai_futura::time::wait_for(3.0, || {
        let trs = doc.query_selector_all("tbody>tr").ok()?;
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

pub async fn create(
    app: &mut App,
    view: &AppView<Web>,
    doc: &web_sys::Document,
    count: usize,
) -> f64 {
    app_create(count, app, view, doc).await + app_clear(app, view, doc).await
}
