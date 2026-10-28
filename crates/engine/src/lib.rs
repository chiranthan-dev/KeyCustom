//! Supervisor for the single AutoHotkey engine process.
//!
//! The engine is the only component that intercepts keys. This crate owns
//! its lifecycle: it starts it, talks to it over stdin/stdout, stops it, and
//! makes sure it cannot outlive KeyForge.

pub mod controller;
pub mod locate;
mod supervisor;
#[cfg(windows)]
pub mod win;

pub use supervisor::*;
