# Single Page App Routing
SPA routing is often needed to represent different resources within a single page application.
Here we define a method of routing that relies on conversions of types and mogwai's `patch:children`
RSX attribute.

### Contents
- [Explanation](#explanation)
- [Code](#code)
- [A running example](#play-with-it)

## Explanation
We first define a `Route` type which will hold all our available routes.
Then we create conversion implementations to convert from a window hash string into a `Route` and back again.
For this routing example (and in order to keep it simple) we also implement `From<Route> for ViewBuilder`,
which we'll use to create views from our route.

```rust, ignore
{{#include ../../examples/spa-routing/src/lib.rs:11:124}}
```

The view will wait for the window's `hashchange` event.
When it receives a `hashchange` event it will extract the new URL and send a
message to the async `logic` loop.
The view will contain a `pre` element to hold any potential error messages.
We will use some convenience functions on `Route` to help display data in the view.

```rust, ignore
{{#include ../../examples/spa-routing/src/lib.rs:195:232}}
```

`logic` receives the `hashchange`, attempting to convert it into a `Route` and either patches the DOM with a new page or sends an error message to our error element.

```rust, ignore
{{#include ../../examples/spa-routing/src/lib.rs:161:193}}
```

That's the bulk of the work.

## Code
[Here's the whole mogwai single page routing example on github](https://github.com/schell/mogwai/blob/master/examples/spa-routing/src/lib.rs).

Notice that the `main` of this example takes an optional string. This allows us to pass the id
of an element that we'd like to append our routing component to. This allows us to load the example on
the page right here. Click on the links below to see the route and our page change. Click on the links in
the table of contents to see our errors propagate into the view.

## Play with it

<div id="app_example"></div>
<script type="module">
  import init, { main } from '{{cookbookroot}}/examples/spa-routing/pkg/spa_routing.js';
  window.addEventListener('load', async () => {
      await init();
      await main("app_example");
  });
</script>
