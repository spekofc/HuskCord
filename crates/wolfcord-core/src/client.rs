// language: Rust, file: crates/wolfcord-core/src/client.rs
//
// Gateway client. Wraps serenity, translates its events into our own vocabulary,
// and owns the state.
//
// Threading: serenity runs its own runtime on a dedicated thread. Events come back
// over a bounded channel and are drained on the UI thread with a very short
// timeout. The UI never blocks on the network for longer than 2 ms per frame.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::sync::Arc;
use std::time::Duration;

use serenity::all::{ChannelId, GatewayIntents, GuildId, MessageId};
use serenity::async_trait;
use serenity::model::channel::Message as DiscordMessage;
use serenity::prelude::*;

use crate::event::Event;
use crate::state::{Channel, Message, State};

/// Bounded queue toward the UI. If the UI stalls, the gateway blocks rather than
/// growing without limit - a chat client that eats memory under load has failed.
const EVENT_CHANNEL_CAPACITY: usize = 1024;

#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Bot token, or a user token.
    pub token: String,
    /// Text-only drops the member and presence intents, which are the expensive
    /// ones and which a text client never reads.
    pub text_only: bool,
}

impl ClientConfig {
    pub fn from_env() -> Option<Self> {
        let token = std::env::var("WOLFCORD_TOKEN").ok()?;
        if token.trim().is_empty() {
            return None;
        }
        Some(Self {
            token,
            text_only: true,
        })
    }

    fn intents(&self) -> GatewayIntents {
        let base = GatewayIntents::GUILDS
            | GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT;
        if self.text_only {
            base
        } else {
            base | GatewayIntents::GUILD_MEMBERS | GatewayIntents::GUILD_PRESENCES
        }
    }
}

#[derive(Debug)]
pub enum ClientError {
    NoToken,
    Spawn(std::io::Error),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::NoToken => write!(f, "no token (set WOLFCORD_TOKEN)"),
            ClientError::Spawn(e) => write!(f, "client thread failed to start: {e}"),
        }
    }
}

impl std::error::Error for ClientError {}

/// Handle the UI holds.
pub struct WolfcordClient {
    events: Receiver<Event>,
    /// Guarded snapshot of state. The gateway task owns the writer; the UI takes a
    /// short lock once per frame to copy the small view model out.
    state: Arc<std::sync::Mutex<State>>,
    /// Commands to the gateway task (send/delete). Fire and forget.
    commands: Sender<Command>,
}

enum Command {
    SendMessage { channel_id: u64, content: String },
    DeleteMessage { channel_id: u64, message_id: u64 },
}

impl WolfcordClient {
    /// Start the gateway and return immediately.
    pub fn start(cfg: ClientConfig) -> Result<Self, ClientError> {
        if cfg.token.trim().is_empty() {
            return Err(ClientError::NoToken);
        }

        let (ev_tx, ev_rx) = mpsc::sync_channel(EVENT_CHANNEL_CAPACITY);
        let (cmd_tx, cmd_rx) = mpsc::channel::<Command>();
        let state = Arc::new(std::sync::Mutex::new(State::new()));

        let state_task = Arc::clone(&state);

        std::thread::Builder::new()
            .name("wolfcord-runtime".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = ev_tx.send(Event::Error {
                            context: "runtime".into(),
                            detail: e.to_string(),
                        });
                        return;
                    }
                };

                rt.block_on(async move {
                    let handler = Handler {
                        tx: ev_tx.clone(),
                        state: Arc::clone(&state_task),
                    };

                    let mut client = match Client::builder(&cfg.token, cfg.intents())
                        .event_handler(handler)
                        .await
                    {
                        Ok(c) => c,
                        Err(e) => {
                            let _ = ev_tx.send(Event::Error {
                                context: "client init".into(),
                                detail: e.to_string(),
                            });
                            return;
                        }
                    };

                    let http = Arc::clone(&client.http);

                    // Command pump: receives from the UI and issues REST calls.
                    let cmd_tx2 = ev_tx.clone();
                    tokio::spawn(async move {
                        while let Ok(cmd) = cmd_rx.recv() {
                            match cmd {
                                Command::SendMessage { channel_id, content } => {
                                    let res = ChannelId::new(channel_id)
                                        .send_message(
                                            &http,
                                            serenity::builder::CreateMessage::new().content(content),
                                        )
                                        .await;
                                    if let Err(e) = res {
                                        let _ = cmd_tx2.send(Event::Error {
                                            context: "send".into(),
                                            detail: e.to_string(),
                                        });
                                    }
                                }
                                Command::DeleteMessage { channel_id, message_id } => {
                                    let _ = ChannelId::new(channel_id)
                                        .delete_message(&http, MessageId::new(message_id))
                                        .await;
                                }
                            }
                        }
                    });

                    if let Err(e) = client.start().await {
                        let _ = ev_tx.send(Event::Error {
                            context: "gateway".into(),
                            detail: e.to_string(),
                        });
                    }
                });
            })
            .map_err(ClientError::Spawn)?;

        Ok(Self {
            events: ev_rx,
            state,
            commands: cmd_tx,
        })
    }

    /// Block for up to `timeout`, returning the first event if one arrives.
    pub fn next_event(&mut self, timeout: Duration) -> Option<Event> {
        match self.events.recv_timeout(timeout) {
            Ok(ev) => Some(ev),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => None,
        }
    }

    /// Borrow the state for rendering. Never held across frames.
    pub fn with_state<R>(&self, f: impl FnOnce(&State) -> R) -> R {
        let guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        f(&guard)
    }

    /// Mutate state from the UI (selection, read receipts).
    pub fn with_state_mut<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut guard)
    }

    pub fn send_message(&self, channel_id: u64, content: String) {
        let (text, _cut) = State::clamp_message(&content);
        if text.trim().is_empty() {
            return;
        }
        let _ = self.commands.send(Command::SendMessage {
            channel_id,
            content: text,
        });
    }

    pub fn delete_message(&self, channel_id: u64, message_id: u64) {
        let _ = self.commands.send(Command::DeleteMessage {
            channel_id,
            message_id,
        });
    }
}

struct Handler {
    tx: SyncSender<Event>,
    state: Arc<std::sync::Mutex<State>>,
}

/// Discord epoch (2015-01-01) in unix millis.
const DISCORD_EPOCH_MS: i64 = 1_420_070_400_000;

fn snowflake_ms(id: u64) -> i64 {
    DISCORD_EPOCH_MS + ((id >> 22) as i64)
}

fn channel_kind_code(kind: serenity::model::channel::ChannelType) -> u8 {
    use serenity::model::channel::ChannelType::*;
    match kind {
        Text => 0,
        Private => 1,
        Voice => 2,
        Group => 3,
        Category => 4,
        News => 5,
        _ => 0,
    }
}

fn convert_message(m: &DiscordMessage, me: Option<u64>) -> Message {
    Message {
        id: m.id.get(),
        channel_id: m.channel_id.get(),
        author_id: m.author.id.get(),
        author_name: m.author.name.clone(),
        author_nick: m.member.as_ref().and_then(|mem| mem.nick.clone()),
        content: m.content.clone(),
        timestamp_ms: m.timestamp.unix_timestamp() * 1000,
        edited: m.edited_timestamp.is_some(),
        attachment_count: m.attachments.len(),
        is_mine: me == Some(m.author.id.get()),
        mentions_me: m.mentions.iter().any(|u| Some(u.id.get()) == me),
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: serenity::model::gateway::Ready) {
        let guild_count = ready.guilds.len();
        let tag = ready.user.name.clone();

        if let Ok(mut st) = self.state.lock() {
            st.user_id = Some(ready.user.id.get());
            st.user_tag = tag.clone();
        }
        let _ = self.tx.send(Event::Ready {
            user: tag,
            guild_count,
        });

        // Pull channels for every guild up front so the sidebar is usable at once.
        for entry in &ready.guilds {
            let guild_id = &entry.id;
            let Ok(channels) = ctx.http.get_channels(*guild_id).await else {
                continue;
            };

            // The ready payload only carries ids; guild_create supplies names.
            let guild_name = format!("guild-{}", guild_id.get());

            if let Ok(mut st) = self.state.lock() {
                st.upsert_guild(guild_id.get(), guild_name);
                for c in channels {
                    st.upsert_channel(Channel {
                        id: c.id.get(),
                        guild_id: Some(c.guild_id.get()),
                        name: c.name.clone(),
                        topic: c.topic.clone(),
                        kind: channel_kind_code(c.kind),
                        position: c.position as i32,
                        parent_id: c.parent_id.map(|p| p.get()),
                        messages: Default::default(),
                        unread: 0,
                    });
                }
                st.sort_channels();
            }
        }

        let _ = self.tx.send(Event::ChannelsUpdated {
            guild_id: 0,
            count: ready.guilds.len(),
        });
    }

    async fn guild_create(
        &self,
        _ctx: Context,
        guild: serenity::model::guild::Guild,
        _is_new: Option<bool>,
    ) {
        if let Ok(mut st) = self.state.lock() {
            st.upsert_guild(guild.id.get(), guild.name.clone());
            for c in guild.channels.values() {
                if st.channels.contains_key(&c.id.get()) {
                    continue;
                }
                st.upsert_channel(Channel {
                    id: c.id.get(),
                    guild_id: Some(guild.id.get()),
                    name: c.name.clone(),
                    topic: c.topic.clone(),
                    kind: channel_kind_code(c.kind),
                    position: c.position as i32,
                    parent_id: c.parent_id.map(|p| p.get()),
                    messages: Default::default(),
                    unread: 0,
                });
            }
            st.sort_channels();
        }
        let _ = self.tx.send(Event::GuildAvailable {
            guild_id: guild.id.get(),
            name: guild.name.clone(),
        });
    }

    async fn guild_delete(
        &self,
        _ctx: Context,
        incomplete: serenity::model::guild::UnavailableGuild,
        _full: Option<serenity::model::guild::Guild>,
    ) {
        let guild_id = incomplete.id.get();
        let name = if let Ok(mut st) = self.state.lock() {
            st.mark_available(guild_id, false);
            st.guilds
                .get(&guild_id)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| format!("guild-{guild_id}"))
        } else {
            format!("guild-{guild_id}")
        };
        let _ = self.tx.send(Event::GuildUnavailable {
            guild_id,
            name,
        });
    }

    async fn message(&self, _ctx: Context, msg: DiscordMessage) {
        let me = self.state.lock().ok().and_then(|s| s.user_id);
        let converted = convert_message(&msg, me);
        let channel_id = converted.channel_id;
        let is_mine = converted.is_mine;

        if let Ok(mut st) = self.state.lock() {
            st.push_message(channel_id, converted.clone());
            if !is_mine && Some(channel_id) != st.active_channel {
                let n = st.unread.entry(channel_id).or_insert(0);
                *n += 1;
                let total = *n;
                if let Some(ch) = st.channels.get_mut(&channel_id) {
                    ch.unread = total;
                }
            }
        }

        let _ = self.tx.send(Event::MessageCreate {
            channel_id,
            message: Arc::new(converted),
        });
    }

    async fn message_update(
        &self,
        _ctx: Context,
        _old: Option<DiscordMessage>,
        _new: Option<DiscordMessage>,
        event: serenity::model::event::MessageUpdateEvent,
    ) {
        // serenity 0.12 made these non-optional on MessageUpdateEvent.
        let channel_id = event.channel_id;
        let id = event.id;

        let rebuilt = if let Ok(mut st) = self.state.lock() {
            if let Some(content) = event.content.clone() {
                let ts = event
                    .edited_timestamp
                    .map(|t| t.unix_timestamp() * 1000)
                    .unwrap_or_else(|| snowflake_ms(id.get()));
                st.edit_message(channel_id.get(), id.get(), content, ts);
            }
            st.channel(channel_id.get())
                .and_then(|ch| ch.messages.iter().find(|m| m.id == id.get()).cloned())
        } else {
            None
        };

        if let Some(m) = rebuilt {
            let _ = self.tx.send(Event::MessageUpdate {
                channel_id: channel_id.get(),
                message: Arc::new(m),
            });
        }
    }

    async fn message_delete(
        &self,
        _ctx: Context,
        channel_id: ChannelId,
        message_id: MessageId,
        _guild_id: Option<GuildId>,
    ) {
        if let Ok(mut st) = self.state.lock() {
            st.delete_message(channel_id.get(), message_id.get());
        }
        let _ = self.tx.send(Event::MessageDelete {
            channel_id: channel_id.get(),
            message_id: message_id.get(),
        });
    }

    async fn typing_start(&self, _ctx: Context, event: serenity::model::event::TypingStartEvent) {
        let _ = self.tx.send(Event::Typing {
            channel_id: event.channel_id.get(),
            user: event.user_id.get().to_string(),
        });
    }

    async fn resume(&self, _ctx: Context, _event: serenity::model::event::ResumedEvent) {
        let _ = self.tx.send(Event::Reconnected);
    }
}

