# Huskcord

A native shell for Discord's web app. Replaces Electron's process stack with a
single ~20 MB Rust binary and a WebView2 window.

Discord's UI is unchanged: it is the server-delivered React app, running in the
same Chromium engine family, with Discord's own client user agent.

## Measured

Windows 11, Discord 1.0.9267 (Electron 42.7.1), WebView2 157.0.4341.46.
Same machine, same login, both settled after ~30 s.

| | Working Set | Private Commit |
|---|---|---|
| Discord desktop | 1,064.9 MB | 756.5 MB |
| **Huskcord** | **~600 MB** | **~400 MB** |
| delta | **-455 MB (-42%)** | **-355 MB (-47%)** |

Per-process at settle, Huskcord:

```text
shell (huskcord.exe)     21 MB
browser                 125 MB
renderer                247 MB
gpu-process             110 MB
network utility          50 MB
storage utility          20 MB
audio utility            22 MB
crashpad                 18 MB
```

Against Discord's equivalent: main 176, renderer 509, GPU 190, audio 95,
network 65, crashpad 35.

> Numbers move ±100 MB with page load and Discord's own update cadence. The
> harness in `bench/` is what produces them; run it yourself rather than
> trusting a table.

## Build

```powershell
cargo build --release
.\target\release\huskcord.exe
```

Requires the WebView2 runtime (present on Windows 10/11 by default).

## Why winit and not tao

First attempt used `tao`. It produced a window and a WebView2 browser process
that never spawned a renderer — `webview2-com`'s controller callback never
completed under `tao`'s message loop. `winit` 0.30, which is what `wry`'s own
example targets, hands the pump correctly and the renderer comes up in ~2 s.

The failure was silent: the shell process stayed alive at ~19 MB with a valid
window handle. What exposed it was the renderer never appearing in the process
tree after 60 s. Full notes in `docs/tao-vs-winit.md`.

## What this is not

Not a Discord client rewrite. There is no local Discord UI to rewrite — the app
arrives over the network, same as in a browser. Huskcord gives that app a native
shell without Electron's tax.

A true native client (custom UI, custom protocol, ~40-80 MB steady) is a
different project. It would trade every server-delivered feature for that.

## Status

Prototype. Loads and renders `discord.com/app`. Missing, in rough priority:
session persistence, tray icon, unread badge, OS notifications, window geometry
restore, push-to-talk, Rich Presence, auto-update.

The one built-in win beyond memory: Discord's web app updates server-side, so
there is no client updater to break.

## License

MIT
