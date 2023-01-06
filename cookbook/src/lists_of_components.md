# A List of Components
A list of components is a common interface pattern. In `mogwai` we can express this using two component
definitions. In this example we'll use a `list` component and an `item` component.

### Contents
- [Explanation](#explanation)
- [Notes](#notes)
- [Code](#code)
- [A running example](#play-with-it)

## Explanation

### Views
The `list` defines its view to include a `button` to create new items and an `ol` to hold them.
Messages will flow from the view into two async tasks, which will patch the items using the `patch:children`
RSX attribute (which we can be set to anything that implements `Stream<Item = ListPatch<ViewBuilder<JsDom>>`).

```rust, ignore
{{#include ../../examples/list-of-gizmos/src/lib.rs:cookbook_list_view}}
```

Each item has a unique `id` that helps us determine which items to remove.

Each item contains a button to remove the item from the list as well as a button to increment a counter.
Click events on these buttons will be as an output into an async task that determines what to do next.

The view receives a count of the current number of clicks from a stream and uses that to display a nice message to the user.

```rust, ignore
{{#include ../../examples/list-of-gizmos/src/lib.rs:cookbook_list_item_view}}
```

### Logic and Communication
Just like most other components, both `list` and `item` have their own async tasks that loop over incoming messages.
These messages encode all the events that occur in our views.

The difference between this list of items and other less complex widgets is the way items communicate
up to the parent list and how view updates are made to the parent list.

As you can see below, `item` takes the item ID _and a `remove_item_clicked: Output<ItemId>`, which it plugs into
the "remove" button's `on:click` attribute.
This sends any click events straight to the the caller of `item`.

It is this output that `list` uses to receiver remove events from its items.

Also notice that we're using [`Model`][structmodel] to keep the state of the number of clicks.
[`Model`][structmodel] is convenient here because it tracks the state and automatically streams any
updates to downstream observers.

```rust, ignore
{{#include ../../examples/list-of-gizmos/src/lib.rs:cookbook_list_item}}
```

The `list` uses [`ListPatchModel`][structlistpatchmodel] to maintain our list if items.
[`ListPatchModel`][structlistpatchmodel] is much like `Model` but is special in that every time itself is patched
it sends a clone of that patch to downstream listeners.
So while `Model` sends a clone of the full updated inner value, `ListPatchModel` sends the diff.
This makes it easy to map the patch from `ItemId` to `ViewBuilder` and use the resulting stream to patch the list's view:

```rust, ignore
{{#include ../../examples/list-of-gizmos/src/lib.rs:cookbook_list}}
```

This is a good example of how `mogwai` separates component state from component views. The list logic doesn't own the
view and doesn't maintain the list of DOM nodes. Instead, the view has a patching mechanism that is set with a stream and then the logic maintains a collection of `ItemId`s that it patches locally - triggering downstream patches to the view automatically.

## Notes

In this example you can see that unlike other vdom based libraries, mogwai's state is completely separate from its views.
Indeed they even require separate handling to "keep them in sync".
This is by design.
In general a mogwai app tends to be more explicit and less magical than its vdom counterparts.

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
