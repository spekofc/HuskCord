// language: Rust, file: crates/wolfcord-app/src/app.rs
//
// eframe application state. Owns the client, drains gateway events, and holds the
// small amount of purely-UI state (compose box, selection).
//
// eframe 0.37 renamed `App::update(ctx, frame)` to `App::ui(ui, frame)`. `logic`
// is where non-drawing work (draining the gateway queue) belongs.
//
// Borrow discipline: `ui` copies a view model out of the client, draws from it,
// and returns actions; the client is only touched again after drawing finishes.
// That is what keeps `&mut self` available to the panels.

use std::time::Duration;

use wolfcord_core::{Event, WolfcordClient};

use crate::ui::Actions;

const EVENT_WAIT: Duration = Duration::from_millis(2);
const MAX_EVENTS_PER_FRAME: usize = 256;
const REPAINT_INTERVAL: Duration = Duration::from_millis(33);

pub struct AppState {
    pub client: Option<WolfcordClient>,
    pub connected: bool,
    pub status: String,
    pub compose: String,
    pub focus_compose: bool,
    pub events_last_frame: usize,
}

impl AppState {
    pub fn new(cc: &eframe::CreationContext<'_>, client: Option<WolfcordClient>) -> Self {
        crate::ui::install_fonts(&cc.egui_ctx);
        crate::ui::install_style(&cc.egui_ctx);

        let has_client = client.is_some();
        Self {
            client,
            connected: false,
            status: if has_client {
                "connecting...".into()
            } else {
                "no token - set WOLFCORD_TOKEN".into()
            },
            compose: String::new(),
            focus_compose: false,
            events_last_frame: 0,
        }
    }

    /// Drain pending gateway events. Called from `logic`, never from `ui`.
    pub fn drain_events(&mut self) {
        let Some(client) = &mut self.client else {
            self.events_last_frame = 0;
            return;
        };

        // Collect first: `client` is borrowed mutably here, `self.handle` needs
        // `&mut self`.
        let mut batch = Vec::new();
        while batch.len() < MAX_EVENTS_PER_FRAME {
            let Some(ev) = client.next_event(EVENT_WAIT) else {
                break;
            };
            batch.push(ev);
        }
        self.events_last_frame = batch.len();
        for ev in batch {
            self.handle(ev);
        }
    }

    fn handle(&mut self, ev: Event) {
        match ev {
            Event::Ready { user, guild_count } => {
                self.connected = true;
                self.status = format!("{user} - {guild_count} guilds");
            }
            Event::Disconnected { reason } => {
                self.connected = false;
                self.status = format!("disconnected: {reason}");
            }
            Event::Reconnected => {
                self.connected = true;
                self.status = "reconnected".into();
            }
            Event::Error { context, detail } => {
                self.connected = false;
                self.status = format!("{context}: {detail}");
            }
            Event::GuildAvailable { name, .. } => {
                self.status = format!("joined {name}");
            }
            Event::GuildUnavailable { name, .. } => {
                self.status = format!("left {name}");
            }
            _ => {}
        }
    }

    /// Apply what the user did in the frame that just drew.
    pub fn apply(&mut self, actions: Actions) {
        let Some(client) = self.client.as_ref() else {
            return;
        };

        if let Some(gid) = actions.select_guild {
            client.with_state_mut(|s| s.active_guild = Some(gid));
        }
        if let Some(cid) = actions.select_channel {
            client.with_state_mut(|s| {
                s.active_channel = Some(cid);
                s.clear_unread(cid);
            });
        }
        if let Some((cid, text)) = actions.send {
            client.send_message(cid, text);
        }
    }
}

impl eframe::App for AppState {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
        // Keep polling while idle so incoming messages appear without input.
        ctx.request_repaint_after(REPAINT_INTERVAL);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        match &self.client {
            Some(client) => {
                let snap = crate::ui::snapshot(client);
                let actions = crate::ui::draw(ui, self, &snap);
                self.apply(actions);
            }
            None => crate::ui::draw_disconnected(ui, self),
        }
    }
}
