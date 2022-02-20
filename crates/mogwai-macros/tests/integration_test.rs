use mogwai::{
    builder::{MogwaiStream, ViewBuilder},
    channel::broadcast,
    dom::view::{Dom, DomBuilderExt},
    html,
    patch::HashPatch,
    rsx,
};

#[test]
fn expand_this_builder() {
    let _ = html! {
        <div
         cast:type = mogwai::dom::view::Dom
         post:build = move |_:&mut Dom| println!("post build")
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
    smol::block_on(async {
        // not all nodes are void nodes
        let div: String = html! {
            <a href="http://zyghost.com" />
        }
        .build()
        .unwrap()
        .html_string()
        .await;
        assert_eq!(&div, r#"<a href="http://zyghost.com"></a>"#);

        let div: String = html! {
            <img src="http://zyghost.com/favicon.ico" />
        }
        .build()
        .unwrap()
        .html_string()
        .await;
        assert_eq!(&div, r#"<img src="http://zyghost.com/favicon.ico" />"#);
    });
}

#[smol_potat::test]
async fn node_self_closing_gt_1_att() {
    let decomp: ViewBuilder<Dom> = html! {<a href="http://zyghost.com" class="blah"/>};
    let (_, attribs) = mogwai::builder::exhaust::<HashPatch<_, _>, _>(
        futures::stream::select_all(decomp.attribs),
    );
    assert_eq!(
        attribs[0],
        HashPatch::Insert("href".to_string(), "http://zyghost.com".to_string())
    );

    // not all nodes are void nodes
    let div: String = html! {<a href="http://zyghost.com" class="blah"/>}
        .build()
        .unwrap()
        .html_string()
        .await;
    assert_eq!(&div, r#"<a href="http://zyghost.com" class="blah"></a>"#);

    let div: String = html! {<img src="http://zyghost.com/favicon.ico" class="blah"/>}
        .build()
        .unwrap()
        .html_string()
        .await;
    assert_eq!(
        &div,
        r#"<img src="http://zyghost.com/favicon.ico" class="blah" />"#
    );
}

#[smol_potat::test]
async fn by_hand() {
    let builder: ViewBuilder<Dom> = ViewBuilder::element("a")
        .with_single_attrib_stream("href", "http://zyghost.com")
        .with_single_attrib_stream("class", "a_link")
        .append(ViewBuilder::text("a text node"));
    assert_eq!(
        r#"<a href="http://zyghost.com" class="a_link">a text node</a>"#,
        builder.build().unwrap().html_string().await
    );
}

#[smol_potat::test]
async fn node() {
    let div: String = html! {
        <a href="http://zyghost.com" class = "a_link">"a text node"</a>
    }
    .build()
    .unwrap()
    .html_string()
    .await;
    assert_eq!(
        &div,
        r#"<a href="http://zyghost.com" class="a_link">a text node</a>"#
    );
}

#[smol_potat::test]
async fn block_in_text() {
    let x: u32 = 66;
    let s: String = html! {
        <pre>"just a string with the number" {format!("{}", x)} "<- blah blah"</pre>
    }
    .build()
    .unwrap()
    .html_string()
    .await;

    assert_eq!(
        s,
        format!("<pre>just a string with the number 66 &lt;- blah blah</pre>")
    );
}

#[smol_potat::test]
async fn block_at_end_of_text() {
    let x: u32 = 66;
    let s: String = html! {
        <pre>"just a string with the number" {format!("{}", x)}</pre>
    }
    .build()
    .unwrap()
    .html_string()
    .await;

    assert_eq!(&s, "<pre>just a string with the number 66</pre>");
}

#[smol_potat::test]
async fn lt_in_text() {
    let s: String = html! {
        <pre>"this is text <- text"</pre>
    }
    .build()
    .unwrap()
    .html_string()
    .await;

    assert_eq!(s, "<pre>this is text &lt;- text</pre>");
}

#[smol_potat::test]
async fn allow_attributes_on_next_line() {
    let _: String = html! {
        <div
            id="my_div"
            style="float: left;">
            "A string"
        </div>
    }
    .build()
    .unwrap()
    .html_string()
    .await;
}

#[test]
fn rsx_cookbook() {
    let (_tx, rx) = broadcast::bounded::<String>(1);
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
    home_class: impl MogwaiStream<String>,
    editor_class: impl MogwaiStream<String>,
    settings_class: impl MogwaiStream<String>,
    profile_class: impl MogwaiStream<String>,
) -> ViewBuilder<Dom> {
    let o_image: Option<ViewBuilder<Dom>> = user
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

#[cfg(feature = "never")]
#[test]
pub fn struct_view_macro_source() {
    struct_view! {
        <Facade>
            <div
             style:background_color = set_bg_color
             on:click = get_click >
                "Hello"
            </div>
        </Facade>
    }

    let (facade, builder): (Facade<Dom>, _) = Facade::new();
    let view = builder.build().unwrap();
    view.run().unwrap();

    let mut remote_facade: Facade<Dom> = facade.clone();
    mogwai::spawn(async move {
        remote_facade.set_bg_color("red").await.unwrap();
        let _event: DomEvent = remote_facade.get_click().await.unwrap();
    });
}

#[test]
pub fn function_style_rsx() {
    fn _signed_in_view_builder(
        user: &User,
        home_class: impl MogwaiStream<String>,
        editor_class: impl MogwaiStream<String>,
        settings_class: impl MogwaiStream<String>,
        profile_class: impl MogwaiStream<String>,
    ) -> ViewBuilder<Dom> {
        let o_image: Option<ViewBuilder<Dom>> = user
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
    }
}

#[smol_potat::test]
async fn rsx_same_as_html() {
    let html = html! {
        <p><div class="my_class">"Hello"</div></p>
    }
    .build()
    .unwrap();
    let html_string = html.html_string().await;

    let rsx = rsx! {
        p{ div(class="my_class"){ "Hello" }}
    }
    .build()
    .unwrap();
    let rsx_string = rsx.html_string().await;

    assert_eq!(html_string, rsx_string, "rsx and html to not agree");
}
