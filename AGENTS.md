# mogwai

**m**inimal, **o**bvious, **g**raphical **w**idget **a**pplication **i**nterface.

`mogwai` is a Rust crate for building GUI applications, primarily in the
browser with WebAssembly, with cross-platform support through server-side
rendering (a TUI backend is planned).

Workspace layout:

- `crates/mogwai`: the core library (features: `web`, `ssr`, `future`; all on by default)
- `crates/mogwai-macros`: proc macros (`rsx!`, `#[derive(ViewChild)]`, `#[derive(ViewProperties)]`)
- `crates/xtask`: development tasks (see Commands)
- `crates/mogwai-benches`, `crates/mogwai-js-framework-benchmark`: benchmarks
- `examples/`: six examples, each a workspace member with its own `Trunk.toml`
- `cookbook/`: the "Cooking with Mogwai" mdbook (not yet ported to 0.7, see Cookbook)

`mogwai` and `mogwai-macros` are published to crates.io and versioned
separately.

## Key Concepts

The library's design tenets. New code should honor them.

1. **Widgets are plain Rust types**, generic over `V: View`. There is no
   framework base class.
2. **Events are futures, not callbacks.**
3. **The event loop is pull-based.** Widgets advance one event at a time by
   implementing the `Step`, `StepMut`, `StepWith` and `StepWithMut` traits
   (`crates/mogwai/src/step.rs`): a caller awaits the widget's next event,
   reacts, then awaits again. This convention is shared across the mogwai
   ecosystem (iti, mogwai-md, house-ui, and friends).
4. **Views are built with `rsx!`.** The `let root = ...` binding inside the
   macro is its return value, and a `V: View` type parameter must be in scope.
   Full syntax reference: `crates/mogwai/src/rsx.rs`.
5. **Cross-platform through `View` traits** (`Web` for the browser, `Ssr` for
   server-side rendering), with room for specialization. The platform preludes
   (`mogwai::web::prelude`, `mogwai::ssr::prelude`) re-export the common
   prelude, plus the WASM crates on web.
6. **Reactivity through `Proxy`**, which patches views when values change.

`crates/mogwai/src/an_introduction.rs` is the canonical introduction and is
published on docs.rs.

## Commands

`cargo xtask` is the task runner (alias defined in `.cargo/config.toml`). It
auto-installs missing tools (wasm-pack, mdbook, mdbook-linkcheck,
mdbook-variables, cargo-generate, trunk) unless you pass
`--skip-install-deps`.

```bash
cargo xtask test everything  # the full CI gate
cargo xtask test cargo       # cargo test only
cargo xtask test cargo-doc   # cargo doc only
cargo xtask test wasm        # wasm-pack test --firefox --headless crates/mogwai
cargo xtask test template    # generate a project from mogwai-template and build it
cargo test -p mogwai         # test one crate
cargo test -- test_name      # run a single test by name
cargo check -p mogwai --target wasm32-unknown-unknown  # wasm compile check
cargo clippy -- -D warnings  # lint (CI does not run clippy, so check locally)
cargo xtask build example           # wasm-pack build all examples into book_examples/
cargo xtask build example counter   # build one example
cargo xtask build cookbook          # build the cookbook (see below)
cargo xtask copy-js-framework-dist --copy-into <dir>  # trunk-build the benchmark into the js-framework-benchmark repo
trunk serve --config Trunk.toml     # serve an example, run from its directory
```

Individual gates in `test everything` can be skipped with
`--skip-cargo-test`, `--skip-cargo-doc`, `--skip-wasm-pack-test`,
`--skip-mogwai-template`.

CI (`.github/workflows/test.yml`) runs on every push:

```bash
RUST_LOG=info,mogwai=trace cargo xtask test everything
```

Run the same before pushing.

### Formatting requires nightly

`rustfmt.toml` uses unstable options (`imports_granularity`, `format_strings`,
`wrap_comments`, `blank_lines_upper_bound`, `combine_control_expr`,
`format_code_in_doc_comments`). Always format with:

```bash
cargo +nightly fmt
```

`scripts/bootstrap.sh` installs nightly and sets it as the default toolchain.
If you are on stable, plain `cargo fmt` silently skips the unstable options
and leaves formatting diffs.

## Testing Notes

- **Doc examples are tests.** `README.md` is compiled as a doctest through
  `doc_comment` (see `crates/mogwai/src/lib.rs`), and code blocks in
  `an_introduction.rs` and module docs run as doctests under `cargo test`.
  Changing a doc example changes a test; keep examples compiling.
- **Browser tests** run through wasm-bindgen-test:
  `wasm-pack test --firefox --headless crates/mogwai` (requires Firefox;
  wasm-pack fetches geckodriver). Browser options live in `webdriver.json`.
- **Template test**: `cargo xtask test template` generates a project from
  `schell/mogwai-template` with cargo-generate and builds it with wasm-pack.
  It guards the first-run experience of new users.
- `cargo doc` is part of the gate; keep docs building.

## Cookbook

`cookbook/` is the "Cooking with Mogwai" mdbook. It is **not yet ported to
0.7**: its examples target 0.6, it is commented out of the workspace members,
and its CI build step is disabled. Do not assume cookbook examples compile
against current mogwai.

When working on it anyway:

```bash
cargo xtask build cookbook                  # builds all examples, then mdbook
cargo xtask build cookbook --skip-examples  # mdbook only
cargo xtask push-cookbook                   # build + sync to s3://zyghost.com (requires aws cli)
```

## Code Style

- Nightly rustfmt, max width 100 (see the formatting note above)
- Imports: std first, then external crates, then `crate::` internal modules
- Naming: `PascalCase` types, `snake_case` functions and modules,
  `SCREAMING_SNAKE_CASE` constants
- All dependencies flow through the single `[workspace.dependencies]` block
  in the root `Cargo.toml` (`dep.workspace = true`)
- Core crates use edition 2024
- Document all functions. Prefer module-level documentation (see `step.rs`,
  `rsx.rs`, `view.rs` for the standard) over in-file section-header comments
- Keep the public API cross-platform: generic over `V: View`; no `web-sys`
  types in public signatures unless the API is browser-specific
- Tests live inline in the module they test (`#[cfg(test)]`); wasm-only tests
  use `#[wasm_bindgen_test]`

## Contributor AI Disclosure

All code contributions generated by AI, or in collaboration with AI, must be
disclosed. Author commits with the format:

`{human-author} with {llm-name} {llm-version} <{human-email}>`

1. Make a normal commit: `git commit ...`
2. Amend the author:
   `git commit --amend --author "{human-author} with {llm-name} {llm-version} <{human-email}>"`

## Git Workflow

- **Never edit the `main` checkout directly.** Do each task in a new git
  worktree branched off `main`, e.g.
  `git worktree add -b chore-thing ../chore-thing`.
- Branch naming: `feat/...`, `fix/...`, `docs/...`, `chore/...`.
- Do not commit or push without explicit permission. Feature work lands as
  PRs merged into `main`.
- Don't put beads IDs in commit messages.
- Releases are version-bump commits (e.g. `v0.7.8`). `mogwai` and
  `mogwai-macros` bump and publish separately; keep the `path` plus `version`
  dependency in `crates/mogwai/Cargo.toml` so published builds resolve.

## Beads

Issue tracking uses beads through the `bd` CLI, stored in the **personal
database at `~/.beads`**. This repo has no per-repo `.beads` directory; look
up related work with `bd search` or `bd list` there.