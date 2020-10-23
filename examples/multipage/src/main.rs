#[macro_use]
extern crate lazy_static;

use std::convert::Infallible;
use std::net::SocketAddr;
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use hyper::service::{make_service_fn, service_fn};
use multipage::Route;

lazy_static! {
    /// Define the [`tera::Tera`] templates which can be used as the shell of the single page
    /// application. The templates are expected to have output for a `"contents"` key. The
    /// `"contents"` key (as defined in the [`html_view`] function) will be the output of rendering
    /// the [`multipage::Route`].
    pub static ref TEMPLATES: tera::Tera = {
        let mut tera = match tera::Tera::new("examples/multipage/src/templates/*.html") {
            Ok(t) => t,
            Err(e) => {
                println!("Parsing error(s): {}", e);
                ::std::process::exit(1);
            }
        };
        // disable autoescaping completely
        tera.autoescape_on(vec![]);
        tera
    };
}

#[tokio::main]
async fn main() {
    // We'll bind to 127.0.0.1:4001
    let addr = SocketAddr::from(([127, 0, 0, 1], 4001));

    // A `Service` is needed for every connection, so this
    // creates one from our `hello_world` function.
    let make_svc = make_service_fn(|_conn| async {
        // service_fn converts our function into a `Service`
        Ok::<_, Infallible>(service_fn(ssr_service))
    });

    let server = Server::bind(&addr).serve(make_svc);
    // And now add a graceful shutdown signal...
    let graceful = server.with_graceful_shutdown(shutdown_signal());

    // Run this server for... forever!
    if let Err(e) = graceful.await {
        eprintln!("server error: {}", e);
    }
}

fn html_view(route: &Route) -> Body {
    let html = multipage::view(&route.to_string());
    let mut context = tera::Context::new();
    context.insert("title", "Home");
    context.insert("contents", &html);
    match TEMPLATES.render("index.html", &context) {
        Ok(page) => Body::from(page),
        Err(_) => Body::from(html),
    }
}

async fn ssr_service(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let mut response = Response::new(Body::empty());
    let route: Route = req.uri().path().into();
    let static_root = std::path::Path::new("examples/multipage");

    match (req.method(), route) {
        (&Method::GET, Route::NotFound) => {
            // Check if it is a static asset
            let static_result = hyper_staticfile::resolve(&static_root, &req).await;
            match static_result {
                Ok(hyper_staticfile::ResolveResult::NotFound) => {
                    // if static asset not found the not found page
                    *response.body_mut() = html_view(&route);
                }
                Ok(asset) => {
                    // if static asset exists, serve the asset
                    response = hyper_staticfile::ResponseBuilder::new()
                        .request(&req)
                        .build(asset)
                        .unwrap();
                }
                Err(_) => {
                    // otherwise serve the not found content
                    *response.body_mut() = html_view(&route);
                }
            }
        }
        (&Method::GET, _) => {
            *response.body_mut() = html_view(&route);
        }
        _ => {
            *response.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
        }
    }

    Ok(response)
}

async fn shutdown_signal() {
    // Wait for the CTRL+C signal
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
}
