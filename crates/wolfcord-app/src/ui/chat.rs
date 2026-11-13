// language: Rust, file: crates/wolfcord-app/src/ui/chat.rs
//
// One message row. Kept separate so the chat pane stays readable and so
// message layout can be optimized independently of panels.

use egui::{RichText, Ui};

use super::{MessageRow, TEXT_DIM, TEXT_MUTED, TEXT_PRIMARY};

/// Format a unix-millis timestamp as HH:MM in local time.
fn hhmm(ms: i64) -> String {
    // chrono is not a dependency: convert via std's UNIX_EPOCH and read the
    // seconds only. Local timezone offsets are not applied; UTC is honest and
    // costs nothing.
    let secs = ms / 1000;
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    format!("{h:02}:{m:02}")
}

pub fn message_row(ui: &mut Ui, m: &MessageRow) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(
            RichText::new(hhmm(m.ts_ms))
                .size(10.0)
                .color(TEXT_DIM)
                .monospace(),
        );
        ui.add_space(4.0);

        let author_color = if m.is_mine { super::ACCENT } else { TEXT_PRIMARY };
        ui.label(RichText::new(&m.author).size(13.0).strong().color(author_color));

        if m.edited {
            ui.label(RichText::new("(edited)").size(10.0).color(TEXT_DIM));
        }
    });

    ui.horizontal(|ui| {
        ui.add_space(46.0);
        ui.label(RichText::new(&m.content).size(14.0).color(TEXT_MUTED).text());
    });

    ui.add_space(2.0);
}
