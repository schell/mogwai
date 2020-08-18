#![allow(unused_braces)]
use mogwai::prelude::*;


#[test]
fn node_self_closing() {
    let div: String = view! {
        <a href="http://zyghost.com" />
    }
    .into_html_string();
    assert_eq!(&div, r#"<a href="http://zyghost.com" />"#);
}


#[test]
fn node_self_closing_gt_1_att() {
    let div: String = view! {<a href="http://zyghost.com" class="blah"/>}.into_html_string();
    assert_eq!(&div, r#"<a href="http://zyghost.com" class="blah" />"#);
}


#[test]
fn by_hand() {
    let _div: String = (View::element("a") as View<web_sys::HtmlElement>)
        .attribute("href", "http://zyghost.com")
        .attribute("class", "a_link")
        .with(View::from("a text node"))
        .into_html_string();
}


#[test]
fn node() {
    let div: String = view! {
        <a href="http://zyghost.com" class = "a_link">"a text node"</a>
    }
    .into_html_string();
    assert_eq!(
        &div,
        r#"<a href="http://zyghost.com" class="a_link">a text node</a>"#
    );
}


#[test]
fn block_in_text() {
    let x: u32 = 66;
    let s: String = view! {
        <pre>"just a string with the number" {&format!("{}", x)} "<- blah blah"</pre>
    }
    .into_html_string();

    assert_eq!(
        s,
        format!("<pre>just a string with the number 66 &lt;- blah blah</pre>")
    );
}


#[test]
fn block_at_end_of_text() {
    let x: u32 = 66;
    let s: String = view! {
        <pre>"just a string with the number" {&format!("{}", x)}</pre>
    }
    .into_html_string();

    assert_eq!(&s, "<pre>just a string with the number 66</pre>");
}


#[test]
fn lt_in_text() {
    let s: String = view! {
        <pre>"this is text <- text"</pre>
    }
    .into_html_string();

    assert_eq!(s, "<pre>this is text &lt;- text</pre>");
}


#[test]
fn allow_attributes_on_next_line() {
    let _: String = view! {
        <div
            id="my_div"
            style="float: left;"
            >
            "A string"
        </div>
    }
    .into_html_string();
}
