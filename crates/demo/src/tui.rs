//! Entrypoint for the TUI version of the demo.
fn main() {
    #[cfg(feature = "tui")]
    demo::tui::run();
}
