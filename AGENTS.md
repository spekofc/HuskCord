# AGENTS.md

Instructions for AI agents and contributors working in this repository.

## What this project is

Huskcord is a ~800 KB Rust binary that replaces Discord's Electron wrapper
with a winit window hosting WebView2. It does not reimplement Discord. The UI
is Discord's own server-delivered React app, loaded into a native shell.

Read `docs/tao-vs-winit.md` before touching the event loop. The first shell
attempt (tao) produced a live window with no renderer and no error. That failure
mode is easy to recreate.

## Non-negotiable constraints

1. **Do not switch the window library.** `winit` 0.30 is required because
   `webview2-com`'s controller callback needs `winit`'s message pump. `tao` and
   raw `wry` without a compatible loop will regress to a blank window.

2. **Do not remove the `windows_subsystem = "windows"` attribute** in release
   builds. Without it the shell opens a console window alongside the app.

3. **Do not add browser features back.** `DISABLED_FEATURES` in `src/main.rs`
   removes Chromium subsystems a chat client never uses. Adding one back costs
   RAM and does not buy parity.

4. **Measure before and after any performance change.** Use `bench/prowl.ps1`.
   Record working set and private commit. A change that "feels lighter" and
   measures heavier is a regression regardless of intent.

5. **Never commit** extracted Discord artifacts (`app.asar`, `core.asar`, any
   file from `%LOCALAPPDATA%\Discord`). They are not ours to redistribute.

## Architecture

```
src/main.rs           winit application handler, WebView2 setup, IPC dispatch
src/session.rs        profile/data-directory resolution from argv
src/tray.rs           Shell_NotifyIconW tray thread (own message pump)
src/notify.rs         injected JS (Notification shim, badge watcher) + toast spawn
src/rich_presence.rs  Discord IPC pipe client (\\.\pipe\discord-ipc-N)
src/icon.rs           embedded PNG -> winit Icon
build.rs              embeds resources/windows.rc (icon + version info)
```

The tray owns a hidden message-only window on its own thread. It must not share
the winit thread: `Shell_NotifyIconW` needs a message pump, and winit already
owns the main one.

## Threading rules

- winit event loop: main thread only.
- WebView2 controller callbacks: delivered by winit's pump, never block them.
- Tray: dedicated thread with `GetMessageW` loop.
- PowerShell toast spawns: detached, never awaited.

## Testing

There is no unit test suite yet. Manual verification:

```powershell
cargo build --release
.\target\release\huskcord.exe --data-dir .\test-profile
```

Expected stderr on a healthy start:

```
profile: test-profile  (<abs path>)
webview ok: https://discord.com/app
tray icon installed
```

Expected process tree: 1 `huskcord.exe` + 7 `msedgewebview2.exe` (browser,
crashpad-handler, gpu-process, renderer, and 3 utility processes).

If the renderer never appears, the shell idles around 20 MB with a valid
window. That is the tao failure mode. Check the event loop library first.

## Style

- `snake_case` functions and variables, `PascalCase` types.
- Comment the non-obvious: why a Win32 call is structured a certain way, why a
  flag is disabled, why a thread exists.
- No emoji in source. No restating the obvious in comments.
- Errors that matter to the user go to stderr; the app must not fail silently.

## Commits

Conventional-ish, present tense, one concern per commit:

```
fix(tray): retry Shell_NotifyIconW when the notification area is not ready
feat(session): resolve profile from --profile/--portable/--data-dir
docs: record the tao/winit pump failure
```
