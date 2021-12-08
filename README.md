<div align="center">
  <h1>
      <img src="https://raw.githubusercontent.com/schell/mogwai/master/img/gizmo.svg" />
    <br />
    mogwai
  </h1>
</div>

> **m**inimal, **o**bvious, **g**raphical **w**eb **a**pplication **i**nterface

release: [![Crates.io][ci]][cl] ![cicd](https://github.com/schell/mogwai/workflows/cicd/badge.svg?branch=release)

master: ![cicd](https://github.com/schell/mogwai/workflows/cicd/badge.svg?branch=master)

[ci]: https://img.shields.io/crates/v/mogwai.svg
[cl]: https://crates.io/crates/mogwai/

`mogwai` is a view library for creating GUI applications.
It is written in Rust and runs in your browser and has enough functionality server-side
to do rendering. It is an alternative to React, Backbone, Ember, Elm, Purescript, etc.

## toc
- [goals](#goals)
- [concepts](#concepts)
- [example](#example)
- [intro](#introduction)
- [why](#why)
- [beginning](#ok---where-do-i-start)
- [more examples](#more-examples)
- [cookbook](#cookbook)
- [sponsor this project](#sponsorship)

## goals

* provide a declarative approach to creating and managing DOM nodes
* encapsulate component state and compose components easily
* explicate DOM updates
* be small and fast (aka keep it snappy)

If mogwai achieves these goals, which I think it does, then maintaining
application state, composing widgets and reasoning about your program will be
easy. Furthermore, your users will be happy because their UI is snappy!

## concepts
The main concepts behind `mogwai` are

* **sinks and streams instead of callbacks** - view events like clicks, blurs, etc are sent
  through streams instead of invoking a callback. Streams can be branched and may have their
  output messages be mapped, filtered and folded.

* **views are dumb** - a `View` is just a struct that mutates the UI tree after
  receiving a message from a stream.
  `View`s may be constructed and nested using plain Rust functions or an RSX macro.

* **widget logic is a loop over event messages** - user interface widgets in `mogwai` are
  asynchronous loops that receive events and update the view.

## example
Here is an example of a button that counts the number of times it has been clicked:

```rust
use mogwai::prelude::*;
```

## introduction
If you're interested in learning more - please read the [introduction and
documentation](https://docs.rs/mogwai/).

## why
Rust is beginning to have a good number of frontend libraries. Many
encorporate a virtual DOM with a magical update phase. Even in a language that
has performance to spare this step can cause unwanted slowness and can be hard to
reason about _what_ is updating, exactly.

In `mogwai`, streams, sinks and a declarative view builder are used to
define components and how they change over time.

DOM mutation is explicit and happens as a result of views receiving messages, so there is no performance overhead from vdom diffing.

If you prefer a functional style of programming with lots of maps and folds - or if you're looking to go _vroom!_ then maybe `mogwai` is right for you :)

Please do keep in mind that `mogwai` is still in alpha and the API is actively
changing - PRs, issues and questions are welcomed. As of the `0.5` release we
expect that the API will be relatively backwards compatible.

### made for rustaceans, by a rustacean
`mogwai` is a Rust first library. There is no requirement that you have `npm` or
`node`. Getting your project up and running without writing any javascript is easy
enough.

### benchmarketing
`mogwai` is snappy! Here are some very handwavey and sketchy todomvc metrics:

![mogwai performance benchmarking](img/perf.png)

## ok - where do i start?
First you'll need new(ish) version of the rust toolchain. For that you can visit
https://rustup.rs/ and follow the installation instructions.

Then you'll need [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/).

For starting a new mogwai project we'll use the wonderful `cargo-generate`, which
can be installed using `cargo install cargo-generate`.

Then run
```shell
cargo generate --git https://github.com/schell/mogwai-template.git
```
and give the command line a project name. Then `cd` into your sparkling new
project and
```shell
wasm-pack build --target web
```
Then, if you don't already have it, `cargo install basic-http-server` or use your
favorite alternative to serve your app:
```shell
basic-http-server -a 127.0.0.1:8888
```
Happy hacking! :coffee: :coffee: :coffee:

## more examples please
Examples can be found in [the examples folder](https://github.com/schell/mogwai/blob/master/examples/).

To build the examples use:
```shell
wasm-pack build --target web examples/{the example}
```

Additional external examples include:
- [mogwai-realworld](https://github.com/schell/mogwai-realworld/) A "real world" app implementation (WIP)
- [the benchmark suite](https://github.com/schell/todo-mvc-bench/)
- your example here ;)

## cookbook
:green_book: [Cooking with Mogwai](https://zyghost.com/guides/mogwai-cookbook/index.html) is a series
of example solutions to various UI problems. It aims to be a good reference doc but not a step-by-step tutorial.

## group channel :phone:
Hang out and talk about `mogwai` in the support channel:

* [direct link to element app](https://app.element.io/#/room/#mogwai:matrix.org)
* invitation https://matrix.to/#/!iABugogSTxJNzlrcMW:matrix.org?via=matrix.org.

## sponsorship
Please consider sponsoring the development of this library!

* [sponsor me on github](https://github.com/sponsors/schell/)
