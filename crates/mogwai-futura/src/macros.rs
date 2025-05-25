//! "RSX" macros for building various platform-specific views.
#[cfg(feature = "web")]
pub use mogwai_future_rsx::rsx_web;

#[cfg(feature = "ssr")]
pub use mogwai_future_rsx::rsx_ssr;
