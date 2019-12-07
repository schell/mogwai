# mogwai
> minimalist, obvious, graphical web application interface

`mogwai` is a frontend DOM library for creating web applications.
It is written in Rust and runs in your browser. It is an alternative
to React, Backbone, Ember, Elm, Purescript, etc.

## goals

* provide a declarative approach to creating and managing DOM nodes
* encapsulate component state and compose components easily
* explicate DOM updates
* be small and fast (snappy)

If mogwai achieves these goals, which I think it does, then maintaining
application state, composing widgets and reasoning about your program will be
easy. Furthermore, your users will be happy because their UI is snappy!

## example
Here is an example of a button that counts its own clicks.

```rust
extern crate mogwai;
use mogwai::prelude::*;

let (tx, rx) =
  txrx_fold(
    0,
    |n:&mut i32, _:&Event| -> String {
      *n += 1;
      if *n == 1 {
        "Clicked 1 time".to_string()
      } else {
        format!("Clicked {} times", *n)
      }
    }
  );

button()
  .rx_text("Clicked 0 times", rx)
  .tx_on("click", tx)
  .build().unwrap()
  .run().unwrap()
```

## introduction
If you're interested in learning more - please read the [introduction and
documentation](https://docs.rs/mogwai/).

## why
Rust is beginning to have a good number of frontend libraries. Most however,
encorporate a virtual DOM with a magical update phase. Even in a languague that
has performance to spare this step can cause unwanted slowness.

`mogwai` lives in a happy space just above "bare metal". It does this by
providing the tools needed to declare exactly which parts of the DOM change and
when.

These same tools encourage functional progamming patterns like encapsulation over
inheritance (or traits, in this case).

Channel-like primitives and a declarative html builder are used to define
components and then wire them together. Once the interface is defined and built,
the channels are effectively erased and it's functions all the way down. There's
no performance overhead from vdom, shadow dom, polling or patching. So if you
prefer a functional style of programming with lots of maps and folds - or if
you're looking to go _vroom!_ then maybe `mogwai` is right for you and your
team :)

### made for rustaceans, by a rustacean
Another benefit of `mogwai` is that it is Rust-first. There is no requirement
that you have `npm` or `node`. Getting your project up and running without
writing any javascript is easy enough.

### performance
`mogwai` is snappy! Here is a very handwavey and sketchy todomvc benchmark:

![mogwai performance benchmarking](img/perf.png)

## ok - where do i start?
First you'll need new(ish) version of the rust toolchain. For that you can visit
https://rustup.rs/ and follow the installation instructions.

Then you'll need [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/).

For starting a new mogwai project we'll use the wonderful `cargo-generate`, which
can be installed using `cargo install cargo-generate`.

Then run
```
cargo generate --git https://github.com/schell/mogwai-template.git
```
and give the command line a project name. Then `cd` into your sparkling new
project and
```
wasm-pack build --target no-modules
```
Then, if you don't already have it, `cargo`install basic-http-server` or use your
favorite alternative to serve your app:
```
basic-http-server -a 127.0.0.1:8888
```
Happy hacking! :coffee: :coffee: :coffee:

## more examples please
For more examples, check out

[the sandbox](https://github.com/schell/mogwai/blob/master/examples/sandbox/)

[the todomvc app](https://github.com/schell/mogwai/blob/master/examples/todomvc)

To build the examples use:
```bash
cd examples/whatever && wasm-pack build --target no-modules
```

## sponsorship
Please consider sponsoring the development of this library!

* [sponsor met on github](https://github.com/sponsors/schell/)
