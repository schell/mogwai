# A List of Components
A list of components is a common interface pattern. In `mogwai` we can express this using two component
definitions. In this example we'll use a `List` component and an `Item` component.

### Contents
- [Explanation](#explanation)
- [Notes](#notes)
- [Code](#code)
- [A running example](#play-with-it)

## Explanation

The `List` defines its view to include a `button` to create new items and a `ul` to hold each item.
Each item will have a unique `id` that will help us determine which items to remove. This `id` isn't
strictly neccessary but it my experience it's a foolproof way to maintain a list of items with frequent splices.

Each item contains a button to remove the item from the list. Click events on this button must be bound to
the parent since it is the job of the parent to patch its child views.
Each item will also maintain a count of clicks - just for fun.

Both `List` and `Item` must define their own model and view messages - `ListIn`, `ListOut`, `ItemIn` and `ItemOut`,
respectively. These messages encode all the interaction between the user/operator, the `List` and each `Item`.

When the operator clicks an item's remove button the item's view sends an `ItemIn::Remove` message. The item's
`Component::update` function then sends an `ItemOut::Remove(item.id)` message - which is bound to its parent
`List` and mapped to `ListIn::Remove(id)`. This triggers the parent's `Component::update` function,
which will search for the item that triggered the event, remove it from its items and also send an `ItemOut::PatchItem(...)`
patch message to remove the item's view from the list's view.

This is a good example of how `mogwai` separates component state from component views. The item gizmos don't own the
view - the window does! Using the item gizmos we can communicate to the item views. Conversly the item views will
communicate with our item gizmos, which will trickle up into the parent.

## Notes

In this example you can see that unlike other vdom based libraries, mogwai's state is completely separate from its views.
Indeed they even require separate handling to "keep them in sync". This is by design. In general a mogwai app tends to be
more explicit and less magical than its vdom counterparts.

## Code

```rust, ignore
{{#include ../../examples/list-of-gizmos/src/lib.rs}}
```

Notice that the `main` of this example is a little different than the others. This allows us to pass the id
of an element that we'd like to append our list/parent component to. This allows us to load the example on
the page right here.

## Play with it

<div id="app_example"></div>
<script type="module">
  import init, { main } from '{{cookbookroot}}/examples/list-of-gizmos/pkg/list_of_gizmos.js';
  window.addEventListener('load', async () => {
      await init();
      await main("app_example");
  });
</script>
