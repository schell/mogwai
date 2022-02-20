//! Core of the mogwai library.
//!
//! Provides generic operations, wrapper types and traits that
//! domain specific views can implement.
pub mod builder;
pub mod channel;
pub mod constraints;
pub mod error;
pub mod futures;
pub mod model;
pub mod patch;
pub mod relay;
pub mod time;
pub mod view;
