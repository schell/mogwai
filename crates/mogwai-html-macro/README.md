# mogwai-html-macro
Provides procedural macro `view!`, which allows the use of RSX
to declare mogwai views.

Example - this RSX:

```html
    view!(
        <footer class="info">
            <p>"Double click to edit a todo"</p>
            <p>
                "Written by "
                <a href="https://github.com/schell">"Schell Scivally"</a>
            </p>
            <p>
                "Part of "
                <a href="http://todomvc.com">"TodoMVC"</a>
            </p>
        </footer>
    ).run()
```

will generate this rust code:

```rust
    (mogwai::gizmo::dom::View::element("footer") as View<web_sys::HtmlElement>)
        .attribute("class", "info")
        .with(
            (mogwai::gizmo::dom::View::element("p") as View<web_sys::HtmlElement>)
                .with("Double click to edit a todo"),
        )
        .with(
            (mogwai::gizmo::dom::View::element("p") as View<web_sys::HtmlElement>)
                .with("Written by ")
                .with(
                    (mogwai::gizmo::dom::View::element("a")
                        as View<web_sys::HtmlElement>)
                        .attribute("href", "https://github.com/schell")
                        .with("Schell Scivally"),
                ),
        )
        .with(
            (mogwai::gizmo::dom::View::element("p") as View<web_sys::HtmlElement>)
                .with("Part of ")
                .with(
                    (mogwai::gizmo::dom::View::element("a")
                        as View<web_sys::HtmlElement>)
                        .attribute("href", "http://todomvc.com")
                        .with("TodoMVC"),
                ),
        )
        .run()
```
