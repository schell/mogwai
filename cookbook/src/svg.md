# Creating SVGs with mogwai
*S*calable *V*ector *G*raphics are images that are specified with XML. An SVG's
markup looks a lot like HTML markup and allows developers and artists to quickly
create images that can be resized without degrading the quality of the image.

### Contents
- [Explanation](#explanation)
- [Notes](#notes)
- [Code](#code)
- [Example](#example)

## Explanation

In `mogwai` we create SVG images using the same RSX we use to create any other
[`View`][structviewbuilder]. There's just one extra attribute we need to specify
that lets the browser know that we're drawing an SVG image instead of HTML -
the `xmlns` attribute.

## Notes

Unfortunately we must supply this namespace for each SVG node. It ends up not
being too much of a burden.

## Code

```rust, ignore
{{#include ../../examples/svg/src/lib.rs}}
```

Notice that the `main` of this example takes an optional string. This allows us
to pass the id of an element that we'd like to append our list/parent component
to. This allows us to load the example on the page right here.

## Example

<div id="app_example"></div>
<script type="module">
  import init, { main } from '{{cookbookroot}}/examples/svg/pkg/svg.js';
  window.addEventListener('load', async () => {
      await init();
      await main("app_example");
  });
</script>


{{#include reflinks.md}}
