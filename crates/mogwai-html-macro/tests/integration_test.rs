#![allow(unused_braces)]
use::mogwai_html_macro::builder;

#[test]
fn node_self_closing() {
    // not all nodes are void nodes
    let div: String = builder! {
        <a href="http://zyghost.com" />
    }
    .html_string();
    assert_eq!(&div, r#"<a href="http://zyghost.com"></a>"#);

    let div: String = builder! {
        <img src="http://zyghost.com/favicon.ico" />
    }
    .html_string();
    assert_eq!(&div, r#"<img src="http://zyghost.com/favicon.ico" />"#);

}

#[test]
fn node_self_closing_gt_1_att() {
    // not all nodes are void nodes
    let div: String = view! {<a href="http://zyghost.com" class="blah"/>}.html_string();
    assert_eq!(&div, r#"<a href="http://zyghost.com" class="blah"></a>"#);

    let div: String = view! {<img src="http://zyghost.com/favicon.ico" class="blah"/>}.html_string();
    assert_eq!(&div, r#"<img src="http://zyghost.com/favicon.ico" class="blah" />"#);
}

#[test]
fn by_hand() {
    let mut div: View<HtmlElement> = View::element("a");
    div.attribute("href", "http://zyghost.com");
    div.attribute("class", "a_link");
    div.with(View::from("a text node"));
    assert_eq!(
        r#"<a href="http://zyghost.com" class="a_link">a text node</a>"#,
        &div.html_string()
    );
}

#[test]
fn node() {
    let div: String = view! {
        <a href="http://zyghost.com" class = "a_link">"a text node"</a>
    }
    .html_string();
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
    .html_string();

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
    .html_string();

    assert_eq!(&s, "<pre>just a string with the number 66</pre>");
}

#[test]
fn lt_in_text() {
    let s: String = view! {
        <pre>"this is text <- text"</pre>
    }
    .html_string();

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
    .html_string();
}
