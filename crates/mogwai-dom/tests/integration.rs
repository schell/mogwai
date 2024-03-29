use std::convert::TryFrom;

use mogwai::model::Model;
use mogwai_dom::{core::channel::broadcast, prelude::*};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn expand_this_builder() {
    let _ = html! {
        <div
         post:build = move |_:&mut JsDom| Ok(println!("post build"))
         style:background_color = "red"
         data_thing = "a string"
         checked >
            "Hello"
            <ul>
                <li>"Uno"</li>
                <li>"Dos"</li>
            </ul>
        </div>
    };
}

#[test]
fn node_self_closing() {
    futures_lite::future::block_on(async {
        // not all nodes are void nodes
        let div = SsrDom::try_from(html! {
            <a href="http://zyghost.com" />
        })
        .unwrap();
        let div: String = div.html_string().await;
        assert_eq!(&div, r#"<a href="http://zyghost.com"></a>"#);

        let div = SsrDom::try_from(html! {
            <img src="http://zyghost.com/favicon.ico" />
        })
        .unwrap();
        let div = div.html_string().await;
        assert_eq!(&div, r#"<img src="http://zyghost.com/favicon.ico" />"#);
    });
}

#[test]
fn node_self_closing_gt_1_att() {
    futures_lite::future::block_on(async {
        let bldr: ViewBuilder = html! {<a href="http://zyghost.com" class="blah"/>};
        match &bldr.initial_values[0] {
            Update::Attribute(HashPatch::Insert(k, v)) => {
                assert_eq!("href", k.as_str());
                assert_eq!("http://zyghost.com", v.as_str());
            }
            att => panic!("unmatched attribute: {:?}", att),
        }

        // not all nodes are void nodes
        let div = SsrDom::try_from(html! {<a href="http://zyghost.com" class="blah"/>}).unwrap();
        let div: String = div.html_string().await;
        assert_eq!(&div, r#"<a href="http://zyghost.com" class="blah"></a>"#);

        let div =
            SsrDom::try_from(html! {<img src="http://zyghost.com/favicon.ico" class="blah"/>})
                .unwrap();
        let div: String = div.html_string().await;
        assert_eq!(
            &div,
            r#"<img src="http://zyghost.com/favicon.ico" class="blah" />"#
        );
    });
}

#[test]
fn by_hand() {
    futures_lite::future::block_on(async {
        let builder: ViewBuilder = ViewBuilder::element("a")
            .with_single_attrib_stream("href", "http://zyghost.com")
            .with_single_attrib_stream("class", "a_link")
            .append(ViewBuilder::text("a text node"));
        assert_eq!(
            r#"<a href="http://zyghost.com" class="a_link">a text node</a>"#,
            SsrDom::try_from(builder).unwrap().html_string().await
        );
    });
}

#[test]
fn node() {
    futures_lite::future::block_on(async {
        let div: String = SsrDom::try_from(html! {
            <a href="http://zyghost.com" class = "a_link">"a text node"</a>
        })
        .unwrap()
        .html_string()
        .await;
        assert_eq!(
            &div,
            r#"<a href="http://zyghost.com" class="a_link">a text node</a>"#
        );
    });
}

#[test]
fn block_in_text() {
    futures_lite::future::block_on(async {
        let x: u32 = 66;
        let s: String = SsrDom::try_from(html! {
            <pre>"just a string with the number" {format!("{}", x)} "<- blah blah"</pre>
        })
        .unwrap()
        .html_string()
        .await;

        assert_eq!(
            s,
            format!("<pre>just a string with the number 66 &lt;- blah blah</pre>")
        );
    });
}

#[test]
fn block_at_end_of_text() {
    futures_lite::future::block_on(async {
        let x: u32 = 66;
        let s: String = SsrDom::try_from(html! {
            <pre>"just a string with the number" {format!("{}", x)}</pre>
        })
        .unwrap()
        .html_string()
        .await;

        assert_eq!(&s, "<pre>just a string with the number 66</pre>");
    });
}

#[test]
fn lt_in_text() {
    futures_lite::future::block_on(async {
        let s: String = SsrDom::try_from(html! {
            <pre>"this is text <- text"</pre>
        })
        .unwrap()
        .html_string()
        .await;

        assert_eq!(s, "<pre>this is text &lt;- text</pre>");
    });
}

#[test]
fn allow_attributes_on_next_line() {
    futures_lite::future::block_on(async {
        let _: String = SsrDom::try_from(html! {
            <div
                id="my_div"
                style="float: left;">
                "A string"
            </div>
        })
        .unwrap()
        .html_string()
        .await;
    });
}

#[test]
fn rsx_cookbook() {
    let (_tx, rx) = broadcast::bounded::<String>(1.try_into().unwrap());
    let _ = signed_in_view_builder(
        &User {
            username: "oona".to_string(),
            o_image: None,
        },
        rx.clone(),
        rx.clone(),
        rx.clone(),
        rx,
    );
}

struct User {
    username: String,
    o_image: Option<String>,
}

fn signed_in_view_builder(
    user: &User,
    home_class: impl Stream<Item = String> + Send + 'static,
    editor_class: impl Stream<Item = String> + Send + 'static,
    settings_class: impl Stream<Item = String> + Send + 'static,
    profile_class: impl Stream<Item = String> + Send + 'static,
) -> ViewBuilder {
    let o_image: Option<ViewBuilder> = user
        .o_image
        .as_ref()
        .map(|image| {
            if image.is_empty() {
                None
            } else {
                Some(html! { <img class="user-pic" src=image /> })
            }
        })
        .flatten();

    html! {
        <ul class="nav navbar-nav pull-xs-right">
            <li class="nav-item">
                <a class=home_class href="#/">" Home"</a>
            </li>
            <li class="nav-item">
                <a class=editor_class href="#/editor">
                    <i class="ion-compose"></i>
                    " New Post"
                </a>
            </li>
            <li class="nav-item">
                <a class=settings_class href="#/settings">
                    <i class="ion-gear-a"></i>
                    " Settings"
                </a>
            </li>
            <li class="nav-item">
                <a class=profile_class href=format!("#/profile/{}", user.username)>
                    {o_image}
                    {format!(" {}", user.username)}
                </a>
            </li>
        </ul>
    }
}

#[test]
pub fn function_style_rsx() {
    fn _signed_in_view_builder(
        user: &User,
        home_class: impl Stream<Item = String> + Send + 'static,
        editor_class: impl Stream<Item = String> + Send + 'static,
        settings_class: impl Stream<Item = String> + Send + 'static,
        profile_class: impl Stream<Item = String> + Send + 'static,
    ) -> ViewBuilder {
        // ANCHOR: rsx_conditional_dom
        let o_image: Option<ViewBuilder> = user
            .o_image
            .as_ref()
            .map(|image| {
                if image.is_empty() {
                    None
                } else {
                    Some(html! { <img class="user-pic" src=image /> })
                }
            })
            .flatten();

        rsx! {
            ul(class="nav navbar-nav pull-xs-right") {
                li(class="nav-item") {
                    a(class=home_class, href="#/"){ " Home" }
                }
                li(class="nav-item") {
                    a(class=editor_class, href="#/editor") {
                        i(class="ion-compose") {
                            " New Post"
                        }
                    }
                }
                li(class="nav-item") {
                    a(class=settings_class, href="#/settings") {
                        i(class="ion-gear-a"){
                            " Settings"
                        }
                    }
                }
                li(class="nav-item") {
                    a(class=profile_class, href=format!("#/profile/{}", user.username)) {
                        {o_image}
                        {format!(" {}", user.username)}
                    }
                }
            }
        }
        // ANCHOR_END: rsx_conditional_dom
    }
}

#[test]
fn rsx_same_as_html() {
    futures_lite::future::block_on(async {
        let html = SsrDom::try_from(html! {
            <p><div class="my_class">"Hello"</div></p>
        })
        .unwrap();
        let html_string = html.html_string().await;

        let rsx = SsrDom::try_from(rsx! {
            p{ div(class="my_class"){ "Hello" }}
        })
        .unwrap();
        let rsx_string = rsx.html_string().await;

        assert_eq!(html_string, rsx_string, "rsx and html to not agree");
    });
}

#[derive(Clone)]
struct Row {
    id: Model<usize>,
    label: Model<String>,
    selected: Model<bool>,
}

impl Row {
    fn new(id: usize, label: impl Into<String>) -> Self {
        Row {
            id: Model::new(id),
            label: Model::new(label),
            selected: Model::new(false),
        }
    }

    fn viewbuilder(self) -> ViewBuilder {
        rsx! {
            tr(
                key = self.id.clone().map(|id| id.to_string()),
                class = self.selected.map(|is_selected| if is_selected {
                        "danger"
                    } else {
                        ""
                    }.to_string())
            ) {
                td(class="col-md-1"){{ self.id.clone().map(|id| id.to_string()) }}
                td(class="col-md-4"){
                    a() {{ self.label.clone() }}
                }
                td(class="col-md-1"){
                    a() {
                        span(class="glyphicon glyphicon-remove", aria_hidden="true") {}
                    }
                }
                td(class="col-md-6"){ }
            }
        }
    }
}

impl TryFrom<Row> for JsDom {
    type Error = anyhow::Error;

    fn try_from(value: Row) -> Result<Self, Self::Error> {
        value.viewbuilder().try_into()
    }
}

#[wasm_bindgen_test]
async fn benchmark_row_clone() {
    let row = Row::new(0, "hello");
    let proto_node = JsDom::try_from(row.clone())
        .unwrap()
        .ossify();

    fn compare_str(id: usize, label: &str) -> String {
        format!(
            r#"<tr key="{id}" class=""><td class="col-md-1">{id}</td><td class="col-md-4"><a>{label}</a></td><td class="col-md-1"><a><span class="glyphicon glyphicon-remove" aria-hidden="true"></span></a></td><td class="col-md-6"></td></tr>"#
        )
    }

    let html_string = proto_node.html_string().await;
    assert_eq!(compare_str(0, "hello"), html_string);

    let hydrated_node = proto_node.hydrate(row.clone().viewbuilder()).unwrap();
    row.id.replace(1usize).await;
    row.label.replace("kia ora").await;
    mogwai::time::wait_millis(10).await;
    let html_string = hydrated_node.html_string().await;
    assert_eq!(compare_str(1, "kia ora"), html_string);
}
