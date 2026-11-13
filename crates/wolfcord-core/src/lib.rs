// language: Rust, file: crates/wolfcord-core/src/lib.rs
//
// Wolfcord core: everything about Discord that is not pixels.
//
// Split from the UI on purpose. This crate can be driven from a test, a CLI, or
// a different frontend without touching a windowing library.

pub mod client;
pub mod event;
pub mod state;

pub use client::{ClientConfig, ClientError, WolfcordClient};
pub use event::Event;
pub use state::{Channel, Guild, Message, State};

/// Maximum characters Discord accepts in a message.
pub const MAX_MESSAGE_LEN: usize = 2000;

/// Maximum characters Discord accepts in a channel topic.
pub const MAX_TOPIC_LEN: usize = 1024;
