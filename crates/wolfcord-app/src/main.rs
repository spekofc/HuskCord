// language: Rust, file: crates/wolfcord-app/src/main.rs
//
// Wolfcord — a native Discord client with no browser engine.
//
// The whole reason this exists: Discord's own UI is a React app, which needs a
// Chromium to render and therefore cannot go below ~400 MB. This renders its own
// UI in egui and speaks the protocol directly.
//
// Threading model:
//   - gateway task: serenity, its own tokio runtime (inside wolfcord-core)
//   - UI thread: eframe/egui, polls for events each frame, never blocks
//   - no shared locks on the render path: the core hands back a snapshot

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod ui;

use std::time::Duration;

use wolfcord_core::{ClientConfig, WolfcordClient};

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "wolfcord=info,serenity=warn".into()),
        )
        .init();

    let cfg = ClientConfig::from_env();

    let client = match cfg {
        Some(cfg) => match WolfcordClient::start(cfg) {
            Ok(c) => Some(c),
            Err(e) => {
                tracing::error!("could not start client: {e}");
                None
            }
        },
        None => {
            tracing::warn!("WOLFCORD_TOKEN not set; starting in offline view");
            None
        }
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Wolfcord"),
        ..Default::default()
    };

    eframe::run_native(
        "Wolfcord",
        native_options,
        Box::new(move |cc| {
            let state = app::AppState::new(cc, client);
            Ok(Box::new(state))
        }),
    )
}
