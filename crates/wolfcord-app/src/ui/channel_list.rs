// language: Rust, file: crates/wolfcord-app/src/ui/channel_list.rs
//
// Channel list helpers. The panel itself lives in ui/mod.rs; layout logic that
// could grow lives here.

use egui::{Color32, RichText};

use super::{ChannelRow, ACCENT, TEXT_MUTED, TEXT_PRIMARY};

/// 0 = text, 2 = voice, 4 = category, 5 = announcement
pub fn kind_color(kind: u8) -> Color32 {
    match kind {
        4 => TEXT_MUTED,
        _ => TEXT_PRIMARY,
    }
}

pub fn row_text(ch: &ChannelRow, selected: bool) -> RichText {
    let mut t = RichText::new(&ch.label).size(14.0).color(kind_color(ch.kind));
    if ch.unread > 0 {
        t = t.strong().color(ACCENT);
    } else if !selected {
        t = t.color(TEXT_MUTED);
    }
    t
}
