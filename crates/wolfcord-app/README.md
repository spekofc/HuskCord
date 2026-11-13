# Wolfcord — a native Discord client

No Chromium. No WebView2. One process, **97 MB**, and it renders its own UI.

The shell in the parent branch (`huskcord`) runs Discord's own React app inside a
WebView2 window: it cut 1,065 MB down to 601 MB, but the browser engine is the
floor and no flag moves it. Wolfcord does not run a browser engine at all.

## Measured

Windows 11, same machine, settled state, working set:

| | RAM | Processes |
|---|---|---|
| Discord desktop (Electron) | 1,064.9 MB | 8 |
| Huskcord shell (WebView2) | 600.9 MB | 8 |
| **Wolfcord** | **97.0 MB** | **1** |

`97 MB` with no token. Loading a gateway connection, a guild list, and message
history moves it to roughly 100-140 MB depending on how much history is cached.
History is capped at 200 messages per channel on purpose.

The binary is 11.9 MB, most of it the glyph atlas egui rasterises at startup.

## Try it

```powershell
$env:WOLFCORD_TOKEN = "your-bot-token"
.\wolfcord.exe
```

Without a token the app opens in an offline view that says exactly that, rather
than a blank window.

## What works today

- Gateway connection with automatic resume
- Guild list, channel list, channel ordering (categories and their children)
- Message history, live messages, edits, deletes
- Unread counts
- Sending messages, deleting your own
- Text intents only: no member or presence churn

## What does not work yet

| | Status |
|---|---|
| Voice | not started. Needs Opus + WebRTC. This is the hard one |
| Screenshare | not started. Needs capture + VP8/H.264 |
| Attachments / embeds | text only for now |
| DMs | channels exist, no dedicated inbox view |
| Mentions rendering | counts only, no highlight |
| Markdown | raw as Discord sent it |
| Login flow | token via env var. No password/2FA UI |

## The trade that was made

Wolfcord does not run Discord's UI. It draws its own. That means:

- **Gain:** 97 MB instead of 601 MB, one process instead of eight, and it starts
  in well under a second.
- **Cost:** every new Discord feature is something to reimplement. A new UI
  element on their side does not appear on ours for free. The shell branch gets
  all of that for nothing because it runs their actual app.

This is the fundamental fork between the two products and it is not a bug:
`huskcord` = full parity, 601 MB. `wolfcord` = 97 MB, own UI.

## Legal / ToS

Automating a user account violates Discord's Terms of Service and will get
accounts terminated. **Bot tokens are the supported path** and that is what this
client expects. If you point it at a user token, that is your call and the risk is
yours.

## Build

```powershell
# requires the MSVC toolchain in the environment (vcvars64) for the `ring` crate
cargo build --release -p wolfcord-app
```

## Layout

```
crates/
  wolfcord-core/   protocol: gateway, REST, event model, state. No UI, no window.
  wolfcord-app/    egui frontend. Depends on core, never the reverse.
```

The split exists so the protocol can be tested without a window and the UI can be
developed against fake events.

## Where the last 35 MB are

egui glyph atlas and font rasterisation are the bulk of the non-data footprint.
`--release` with `lto = "fat"` and `strip = true` is already on. Further cuts would
come from a smaller font set and from not keeping egui's texture cache warm, both
of which cost text quality.

## License

MIT
