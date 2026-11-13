// language: Rust, file: crates/wolfcord-app/src/ui/mod.rs
//
// Rendering for egui 0.37.
//
// 0.37 changed three things this file depends on:
//   - Panel::left("id") / Panel::bottom("id") replace SidePanel / TopBottomPanel
//   - Rounding is now CornerRadius
//   - Context::style()/set_style() became all_styles_mut()/set_visuals()
//
// Draw takes an immutable view model plus a command sink. It never holds a
// reference to the client, so the app can hand out &mut self for UI state at the
// same time.

use crate::app::AppState;

pub mod channel_list;
pub mod chat;
pub mod theme;

pub use theme::*;

/// Fonts and text styles. egui's defaults are already small; only sizes differ.
pub fn install_fonts(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (egui::TextStyle::Heading, egui::FontId::proportional(18.0)),
            (egui::TextStyle::Body, egui::FontId::proportional(14.0)),
            (egui::TextStyle::Monospace, egui::FontId::monospace(13.0)),
            (egui::TextStyle::Button, egui::FontId::proportional(14.0)),
            (egui::TextStyle::Small, egui::FontId::proportional(11.0)),
        ]
        .into();
    });
}

/// Dark theme tuned for a chat client: low-contrast background, crisp text.
pub fn install_style(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        style.visuals.dark_mode = true;
        style.visuals.panel_fill = BG_DEEP;
        style.visuals.window_fill = BG_PANEL;
        style.visuals.extreme_bg_color = BG_INPUT;
        style.visuals.override_text_color = Some(TEXT_PRIMARY);
        style.visuals.widgets.noninteractive.bg_fill = BG_PANEL;
        style.visuals.widgets.inactive.bg_fill = BG_INPUT;
        style.visuals.widgets.hovered.bg_fill = BG_HOVER;
        style.visuals.widgets.active.bg_fill = BG_ACTIVE;
        style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    });
}

/// Commands the UI wants performed against the client. Collected during draw and
/// applied afterwards, so drawing never mutates protocol state mid-frame.
#[derive(Default)]
pub struct Actions {
    pub select_guild: Option<u64>,
    pub select_channel: Option<u64>,
    pub send: Option<(u64, String)>,
}

#[derive(Default)]
pub struct SnapshotView {
    pub guilds: Vec<(u64, String, usize)>,
    pub active_guild: Option<u64>,
    pub active_channel: Option<u64>,
    pub channels: Vec<ChannelRow>,
    pub messages: Vec<MessageRow>,
    pub channel_title: Option<String>,
    pub user: String,
    pub total_unread: u32,
}

pub struct ChannelRow {
    pub id: u64,
    pub label: String,
    pub kind: u8,
    pub unread: u32,
}

pub struct MessageRow {
    pub author: String,
    pub content: String,
    pub is_mine: bool,
    pub edited: bool,
    pub ts_ms: i64,
}

/// Copy the parts the UI needs out of core state. Once per frame.
pub fn snapshot(client: &wolfcord_core::WolfcordClient) -> SnapshotView {
    client.with_state(|st| SnapshotView {
        guilds: st
            .guilds
            .values()
            .map(|g| (g.id, g.name.clone(), g.channels.len()))
            .collect(),
        active_guild: st.active_guild,
        active_channel: st.active_channel,
        channels: st
            .active_guild
            .map(|gid| {
                st.guild_channels(gid)
                    .map(|c| ChannelRow {
                        id: c.id,
                        label: c.label(),
                        kind: c.kind,
                        unread: c.unread,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        messages: st
            .active_channel
            .and_then(|cid| st.channel(cid))
            .map(|c| {
                c.messages
                    .iter()
                    .map(|m| MessageRow {
                        author: m
                            .author_nick
                            .clone()
                            .unwrap_or_else(|| m.author_name.clone()),
                        content: m.content.clone(),
                        is_mine: m.is_mine,
                        edited: m.edited,
                        ts_ms: m.timestamp_ms,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        channel_title: st
            .active_channel
            .and_then(|cid| st.channel(cid))
            .map(|c| c.name.clone()),
        user: st.user_tag.clone(),
        total_unread: st.total_unread(),
    })
}

/// Draw the three-pane UI. Returns the actions the caller should apply.
pub fn draw(ui: &mut egui::Ui, app: &mut AppState, snap: &SnapshotView) -> Actions {
    let ctx = ui.ctx().clone();
    let mut actions = Actions::default();

    // ---- left rail: guilds ------------------------------------------------
    egui::Panel::left("guilds")
        .exact_size(62.0)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(BG_RAIL))
        .show_inside(ui, |ui| {
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                for (id, name, _count) in &snap.guilds {
                    let selected = snap.active_guild == Some(*id);
                    let label: String =
                        name.chars().take(2).flat_map(|c| c.to_uppercase()).collect();
                    let btn = egui::Button::new(egui::RichText::new(label).size(13.0))
                        .fill(if selected { ACCENT } else { BG_INPUT })
                        .corner_radius(egui::CornerRadius::same(18))
                        .min_size(egui::vec2(40.0, 40.0));
                    if ui.add(btn).on_hover_text(name).clicked() {
                        actions.select_guild = Some(*id);
                    }
                }
            });
        });

    // ---- channel list -----------------------------------------------------
    egui::Panel::left("channels")
        .default_size(220.0)
        .resizable(true)
        .frame(egui::Frame::NONE.fill(BG_PANEL))
        .show_inside(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(
                        snap.channel_title.clone().unwrap_or_else(|| "channels".into()),
                    )
                    .strong()
                    .color(TEXT_PRIMARY),
                );
            });
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                for ch in &snap.channels {
                    let selected = snap.active_channel == Some(ch.id);
                    let text = channel_list::row_text(ch, selected);
                    if ui.selectable_label(selected, text).clicked() {
                        actions.select_channel = Some(ch.id);
                    }
                }
            });
        });

    // ---- status bar -------------------------------------------------------
    egui::Panel::bottom("status")
        .exact_size(22.0)
        .frame(egui::Frame::NONE.fill(BG_RAIL))
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let dot = if app.connected { ONLINE } else { OFFLINE };
                ui.label(egui::RichText::new("*").color(dot).size(12.0));
                ui.label(egui::RichText::new(&app.status).size(11.0).color(TEXT_MUTED));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if snap.total_unread > 0 {
                        ui.label(
                            egui::RichText::new(format!("{} unread", snap.total_unread))
                                .size(11.0)
                                .color(ACCENT),
                        );
                    }
                });
            });
        });

    // ---- chat pane --------------------------------------------------------
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(BG_DEEP))
        .show_inside(ui, |ui| {
            let compose_h = 70.0;
            let chat_h = (ui.available_height() - compose_h).max(80.0);

            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .max_height(chat_h)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if snap.messages.is_empty() {
                        ui.add_space(40.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("no messages yet")
                                    .color(TEXT_MUTED)
                                    .italics(),
                            );
                        });
                    }
                    for m in &snap.messages {
                        chat::message_row(ui, m);
                    }
                    ui.add_space(8.0);
                });

            ui.separator();

            ui.horizontal(|ui| {
                let w = (ui.available_width() - 80.0).max(120.0);
                let resp = ui.add(
                    egui::TextEdit::multiline(&mut app.compose)
                        .desired_rows(1)
                        .desired_width(w)
                        .hint_text("message"),
                );

                if app.focus_compose {
                    resp.request_focus();
                    app.focus_compose = false;
                }

                let submitted = resp.has_focus()
                    && ctx.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
                if submitted && !app.compose.trim().is_empty() {
                    if let Some(ch) = snap.active_channel {
                        actions.send = Some((ch, app.compose.trim().to_string()));
                        app.compose.clear();
                    }
                }

                if ui
                    .add(egui::Button::new("send").min_size(egui::vec2(60.0, 28.0)))
                    .clicked()
                    && !app.compose.trim().is_empty()
                {
                    if let Some(ch) = snap.active_channel {
                        actions.send = Some((ch, app.compose.trim().to_string()));
                        app.compose.clear();
                    }
                }
            });
        });

    actions
}

/// Shown when there is no token: explains the next step instead of a blank frame.
pub fn draw_disconnected(ui: &mut egui::Ui, app: &mut AppState) {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(BG_DEEP))
        .show_inside(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(120.0);
                ui.label(
                    egui::RichText::new("Wolfcord")
                        .size(34.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("a native Discord client - no browser")
                        .size(14.0)
                        .color(TEXT_MUTED),
                );
                ui.add_space(40.0);
                ui.label(
                    egui::RichText::new("Set a token to connect:")
                        .size(13.0)
                        .color(TEXT_MUTED),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("$env:WOLFCORD_TOKEN = \"your-bot-token\"")
                        .monospace()
                        .color(ACCENT),
                );
                ui.add_space(24.0);
                ui.label(egui::RichText::new(&app.status).size(12.0).color(OFFLINE));
            });
        });
}
