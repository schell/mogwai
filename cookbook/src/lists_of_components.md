# A List of Components
A list of components is a common interface pattern. In `mogwai` we can express this using two component
definitions. In this example we'll use a `List` component and an `Item` component.

### Contents
- [Explanation](#explanation)
- [Notes](#notes)
- [Code](#code)
- [A running example](#play-with-it)

## Explanation

The `List` defines its view to include a `button` to create new items and an `ol` to hold each item. Logic messages will be sent into a channel and child items are patched in using the `patch:children` RSX attribute, which we can fill in with anything that implements `Stream<Item = ListPatch<ViewBuilder<JsDom>>`.

```rust, ignore
{{#include ../../examples/list-of-gizmos/src/lib.rs:list_view}}
```

Each item will have a unique `id` that will help us determine which items to remove. This `id` isn't
strictly neccessary but it my experience it's a foolproof way to maintain a list of items with frequent splices.

Each item contains a button to remove the item from the list as well as a button to increment a counter. Click events on these buttons will be sent on a channel.

The view receives a count of the current number of clicks from a stream and uses that to display a nice message to the user.

```rust, ignore
{{#include ../../examples/list-of-gizmos/src/lib.rs:item_view}}
```

Just like most other componens both `List` and `Item` have their own logic loops with enum messages -
`ListMsg` and `ItemMsg` respectively, respectively. These messages encode all the interaction between
the user/operator, the `List` and each `Item`.

The real difference between this list of items and other less complex widgets is the way items communicate
up to the parent list and how view updates are made to the parent list.

Upstream communication from items to the list is set up in the list's logic loop, along with a patching mechanism for
keeping the list in sync with its view:

```rust, ignore
{{#include ../../examples/list-of-gizmos/src/lib.rs:list_logic_coms}}
```

Above we use [`ListPatchModel`][structlistpatchmodel] to maintain our list if items. [`ListPatchModel`][structlistpatchmodel]
is special in that every time it is patched it sends a clone of that patch to downstream listeners. This makes it easy to
map the patch from `Item` to `ViewBuilder<JsDom>` and use the resulting stream to patch the list's view.

This is a good example of how `mogwai` separates component state from component views. The list logic doesn't own the
view and doesn't maintain the list of DOM nodes. Instead, the list logic sets up a patching mechanism and then
maintains a collection of `Item`s that it patches locally - triggering downstream patches to the view automatically.

The rest of the logic loop is business as usual with the exception of where we get our logic messages. The special bit here
is that we are waiting for messages on both a receiver from the logic's view _and_ all receivers given to the items:

```rust, ignore
{{#include ../../examples/list-of-gizmos/src/lib.rs:list_logic_loop}}
```

## Notes

In this example you can see that unlike other vdom based libraries, mogwai's state is completely separate from its views.
Indeed they even require separate handling to "keep them in sync". This is by design. In general a mogwai app tends to be
more explicit and less magical than its vdom counterparts.

## Code

You can see the code in its entirety at
[mogwai's list of components example](https://github.com/schell/mogwai/blob/master/examples/list-of-gizmos/src/lib.rs).

## Play with it

<div id="app_example"></div>
<script type="module">
  import init, { main } from '{{cookbookroot}}/examples/list-of-gizmos/pkg/list_of_gizmos.js';
  window.addEventListener('load', async () => {
      await init();
      await main("app_example");
  });
</script>

{{#include reflinks.md}}
