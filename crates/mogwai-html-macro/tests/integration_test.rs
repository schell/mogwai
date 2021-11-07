use std::convert::TryFrom;

use mogwai_html_macro::{builder, view};
use mogwai::{builder::{DecomposedViewBuilder, ViewBuilder}, channel::broadcast, patch::HashPatch, target::Streamable, view::{View, Dom}};

#[test]
fn node_self_closing() {
    // not all nodes are void nodes
    let div: String = view! {
        <a href="http://zyghost.com" />
    }
    .into();
    assert_eq!(&div, r#"<a href="http://zyghost.com"></a>"#);

    let div: String = view! {
        <img src="http://zyghost.com/favicon.ico" />
    }
    .into();
    assert_eq!(&div, r#"<img src="http://zyghost.com/favicon.ico" />"#);

}

#[test]
fn node_self_closing_gt_1_att() {
    let decomp: DecomposedViewBuilder<Dom>  = builder! {<a href="http://zyghost.com" class="blah"/>}.into();
    assert_eq!(decomp.attribs[0], HashPatch::Insert("href".to_string(), "http://zyghost.com".to_string()));

    // not all nodes are void nodes
    let div: String = view! {<a href="http://zyghost.com" class="blah"/>}.into();
    assert_eq!(&div, r#"<a href="http://zyghost.com" class="blah"></a>"#);

    let div: String = view! {<img src="http://zyghost.com/favicon.ico" class="blah"/>}.into();
    assert_eq!(&div, r#"<img src="http://zyghost.com/favicon.ico" class="blah" />"#);
}

#[test]
fn by_hand() {
    let builder: ViewBuilder<Dom> = ViewBuilder::element("a")
        .with_single_attrib_stream("href", "http://zyghost.com")
        .with_single_attrib_stream("class", "a_link")
        .with_child(ViewBuilder::text("a text node"));
    assert_eq!(
        r#"<a href="http://zyghost.com" class="a_link">a text node</a>"#,
        String::from(View::try_from(builder).unwrap())
    );
}

#[test]
fn node() {
    let div: String = view! {
        <a href="http://zyghost.com" class = "a_link">"a text node"</a>
    }
    .into();
    assert_eq!(
        &div,
        r#"<a href="http://zyghost.com" class="a_link">a text node</a>"#
    );
}

#[test]
fn block_in_text() {
    let x: u32 = 66;
    let s: String = view! {
        <pre>"just a string with the number" {format!("{}", x)} "<- blah blah"</pre>
    }
    .into();

    assert_eq!(
        s,
        format!("<pre>just a string with the number 66 &lt;- blah blah</pre>")
    );
}

#[test]
fn block_at_end_of_text() {
    let x: u32 = 66;
    let s: String = view! {
        <pre>"just a string with the number" {format!("{}", x)}</pre>
    }
    .into();

    assert_eq!(&s, "<pre>just a string with the number 66</pre>");
}

#[test]
fn lt_in_text() {
    let s: String = view! {
        <pre>"this is text <- text"</pre>
    }
    .into();

    assert_eq!(s, "<pre>this is text &lt;- text</pre>");
}

#[test]
fn allow_attributes_on_next_line() {
    let _: String = view! {
        <div
            id="my_div"
            style="float: left;">
            "A string"
        </div>
    }
    .into();
}

#[test]
fn rsx_cookbook() {
    let (_tx, rx) = broadcast::bounded::<String>(1);
    let _ = signed_in_view_builder(&User{username: "oona".to_string(), o_image: None}, rx.clone(), rx.clone(), rx.clone(), rx);
}

struct User {
    username: String,
    o_image: Option<String>
}

fn signed_in_view_builder(
    user: &User,
    home_class: impl Streamable<String>,
    editor_class: impl Streamable<String>,
    settings_class: impl Streamable<String>,
    profile_class: impl Streamable<String>,
) -> ViewBuilder<Dom> {
    let o_image: Option<ViewBuilder<Dom>> = user
        .o_image
        .as_ref()
        .map(|image| {
            if image.is_empty() {
                None
            } else {
                Some(builder! { <img class="user-pic" src=image /> })
            }
        })
        .flatten();

    builder! {
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
