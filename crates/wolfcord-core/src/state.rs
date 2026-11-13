// language: Rust, file: crates/wolfcord-core/src/state.rs
//
// Local app state. Owned, small, and cheap to render from.
//
// Design rules:
//   - Keep only what the UI draws. No raw JSON, no full guild objects.
//   - Messages are Arc'd so they can move to the UI thread without copies.
//   - No interior mutability: one writer (the gateway task) owns it, the UI reads
//     a snapshot. Simpler than locks and fast enough at chat scale.

use std::collections::{BTreeMap, HashMap, VecDeque};

use crate::MAX_MESSAGE_LEN;

/// Per-channel ring buffer size. A chat view never shows more, and unbounded
/// history is the easiest way to lose the memory advantage.
pub const HISTORY_LIMIT: usize = 200;

#[derive(Debug, Clone)]
pub struct Message {
    pub id: u64,
    pub channel_id: u64,
    pub author_id: u64,
    pub author_name: String,
    pub author_nick: Option<String>,
    /// Raw content, markdown as Discord sent it.
    pub content: String,
    /// Unix millis, as Discord sends it (a snowflake encodes this too).
    pub timestamp_ms: i64,
    pub edited: bool,
    pub attachment_count: usize,
    pub is_mine: bool,
    /// Mentioned us.
    pub mentions_me: bool,
}

impl Message {
    /// One-line preview for channel lists.
    pub fn preview(&self) -> String {
        let s: String = self.content.chars().take(80).collect();
        if self.content.chars().count() > 80 {
            format!("{s}...")
        } else {
            s
        }
    }

    /// Timestamp from the snowflake, without trusting the timestamp field.
    ///
    /// Discord snowflakes are (ms since Discord epoch) shifted left 22. The
    /// epoch is 2015-01-01T00:00:00Z.
    pub fn time_from_snowflake(&self) -> i64 {
        const DISCORD_EPOCH: i64 = 1_420_070_400_000;
        DISCORD_EPOCH + ((self.id >> 22) as i64)
    }
}

#[derive(Debug, Clone)]
pub struct Channel {
    pub id: u64,
    pub guild_id: Option<u64>,
    pub name: String,
    pub topic: Option<String>,
    /// 0 = text, 2 = voice, 4 = category, 5 = announcement
    pub kind: u8,
    pub position: i32,
    pub parent_id: Option<u64>,
    pub messages: VecDeque<Message>,
    pub unread: u32,
}

impl Channel {
    pub fn is_text(&self) -> bool {
        matches!(self.kind, 0 | 5)
    }

    pub fn is_voice(&self) -> bool {
        self.kind == 2
    }

    pub fn is_category(&self) -> bool {
        self.kind == 4
    }

    /// Display name with the channel sigil Discord uses.
    pub fn label(&self) -> String {
        match self.kind {
            2 => format!("🔊 {}", self.name),
            4 => self.name.to_uppercase(),
            _ => format!("# {}", self.name),
        }
    }

    pub fn push_message(&mut self, msg: Message) {
        self.messages.push_back(msg);
        while self.messages.len() > HISTORY_LIMIT {
            self.messages.pop_front();
        }
    }
}

#[derive(Debug, Clone)]
pub struct Guild {
    pub id: u64,
    pub name: String,
    /// Channels in sidebar order. Categories sort before their children.
    pub channels: Vec<u64>,
    pub available: bool,
}

/// The whole client state. One of these exists per process.
#[derive(Debug, Default)]
pub struct State {
    pub user_id: Option<u64>,
    pub user_tag: String,
    pub guilds: BTreeMap<u64, Guild>,
    pub channels: HashMap<u64, Channel>,
    pub active_guild: Option<u64>,
    pub active_channel: Option<u64>,
    /// Unread counts keyed by channel.
    pub unread: HashMap<u64, u32>,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert_guild(&mut self, id: u64, name: String) -> &mut Guild {
        self.guilds.entry(id).or_insert_with(|| Guild {
            id,
            name,
            channels: Vec::new(),
            available: true,
        })
    }

    pub fn upsert_channel(&mut self, ch: Channel) {
        self.channels.insert(ch.id, ch);
    }

    pub fn mark_available(&mut self, guild_id: u64, available: bool) {
        if let Some(g) = self.guilds.get_mut(&guild_id) {
            g.available = available;
        }
    }

    /// Rebuild each guild's channel order: categories in position order, with
    /// their children directly after.
    pub fn sort_channels(&mut self) {
        let mut by_guild: HashMap<u64, Vec<&Channel>> = HashMap::new();
        for ch in self.channels.values() {
            if let Some(gid) = ch.guild_id {
                by_guild.entry(gid).or_default().push(ch);
            }
        }

        for (gid, chans) in by_guild {
            let mut ordered: Vec<u64> = Vec::with_capacity(chans.len());

            let mut top: Vec<&Channel> = chans.iter().copied().filter(|c| c.parent_id.is_none()).collect();
            top.sort_by_key(|c| (c.position, c.id));

            for parent in top {
                ordered.push(parent.id);
                if parent.is_category() {
                    let mut kids: Vec<&Channel> = chans
                        .iter()
                        .copied()
                        .filter(|c| c.parent_id == Some(parent.id))
                        .collect();
                    kids.sort_by_key(|c| (c.position, c.id));
                    ordered.extend(kids.into_iter().map(|c| c.id));
                }
            }

            // Any orphan whose parent is missing still needs to show up.
            for c in chans {
                if !ordered.contains(&c.id) {
                    ordered.push(c.id);
                }
            }

            if let Some(g) = self.guilds.get_mut(&gid) {
                g.channels = ordered;
            }
        }
    }

    pub fn guild_channels(&self, guild_id: u64) -> impl Iterator<Item = &Channel> {
        self.guilds
            .get(&guild_id)
            .map(|g| g.channels.iter())
            .into_iter()
            .flatten()
            .filter_map(|id| self.channels.get(id))
    }

    pub fn channel(&self, id: u64) -> Option<&Channel> {
        self.channels.get(&id)
    }

    pub fn channel_mut(&mut self, id: u64) -> Option<&mut Channel> {
        self.channels.get_mut(&id)
    }

    pub fn push_message(&mut self, channel_id: u64, msg: Message) {
        if let Some(ch) = self.channels.get_mut(&channel_id) {
            ch.push_message(msg);
        }
    }

    pub fn edit_message(&mut self, channel_id: u64, message_id: u64, content: String, ts_ms: i64) {
        if let Some(ch) = self.channels.get_mut(&channel_id) {
            if let Some(m) = ch.messages.iter_mut().find(|m| m.id == message_id) {
                m.content = content;
                m.timestamp_ms = ts_ms;
                m.edited = true;
            }
        }
    }

    pub fn delete_message(&mut self, channel_id: u64, message_id: u64) {
        if let Some(ch) = self.channels.get_mut(&channel_id) {
            ch.messages.retain(|m| m.id != message_id);
        }
    }

    pub fn clear_unread(&mut self, channel_id: u64) {
        self.unread.remove(&channel_id);
        if let Some(ch) = self.channels.get_mut(&channel_id) {
            ch.unread = 0;
        }
    }

    /// Total unread across every channel, for the window title.
    pub fn total_unread(&self) -> u32 {
        self.unread.values().sum()
    }

    /// Clamp a composed message to Discord's limit. Returns the clamped string
    /// and whether it was cut.
    pub fn clamp_message(text: &str) -> (String, bool) {
        let n = text.chars().count();
        if n <= MAX_MESSAGE_LEN {
            (text.to_string(), false)
        } else {
            (text.chars().take(MAX_MESSAGE_LEN).collect(), true)
        }
    }
}
