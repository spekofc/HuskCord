// language: Rust, file: crates/wolfcord-core/src/event.rs
//
// The UI-facing event vocabulary.
//
// Deliberately small and owned: no lifetimes, no serenity types leaking into the
// app. The app matches on these and never on the protocol crate, so swapping the
// protocol implementation later does not touch a single UI line.

use std::sync::Arc;

use crate::state::Message;

#[derive(Debug, Clone)]
pub enum Event {
    /// Gateway is connected and identified. `user` is our own tag.
    Ready { user: String, guild_count: usize },

    /// A guild became available (initial chunk or GUILD_CREATE).
    GuildAvailable { guild_id: u64, name: String },

    /// A guild was removed, lost access, or became unavailable.
    GuildUnavailable { guild_id: u64, name: String },

    /// Channels arrived for a guild. Sent after GuildAvailable for that guild.
    ChannelsUpdated { guild_id: u64, count: usize },

    /// A message arrived, was edited, or was deleted.
    MessageCreate { channel_id: u64, message: Arc<Message> },
    MessageUpdate { channel_id: u64, message: Arc<Message> },
    MessageDelete { channel_id: u64, message_id: u64 },

    /// Our own user changed state (typing, presence, nickname).
    Typing { channel_id: u64, user: String },

    /// Connection lost. The client will reconnect automatically.
    Disconnected { reason: String },

    /// Reconnected after a drop.
    Reconnected,

    /// Anything that should reach the user as a problem, not a crash.
    Error { context: String, detail: String },
}

impl Event {
    /// Short label for logs. Never used for logic.
    pub fn kind(&self) -> &'static str {
        match self {
            Event::Ready { .. } => "ready",
            Event::GuildAvailable { .. } => "guild_available",
            Event::GuildUnavailable { .. } => "guild_unavailable",
            Event::ChannelsUpdated { .. } => "channels_updated",
            Event::MessageCreate { .. } => "message_create",
            Event::MessageUpdate { .. } => "message_update",
            Event::MessageDelete { .. } => "message_delete",
            Event::Typing { .. } => "typing",
            Event::Disconnected { .. } => "disconnected",
            Event::Reconnected => "reconnected",
            Event::Error { .. } => "error",
        }
    }
}
