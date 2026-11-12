# Project B — native Discord client

This branch is the start of a Discord client with **no Chromium in it at all**.
It is a different project that happens to share a repository, not a successor to
the shell.

The Wolf name stays. The husk does not.

## Why this exists

The shell plateaued at ~400-600 MB because Discord's UI is a React app and needs
a browser engine to run. Every attempt to shrink the engine failed or backfired
([memory-experiments.md](../docs/memory-experiments.md)). The only way below
that number is to not run a browser engine.

```
shell (main branch)     ~400-600 MB    full Discord parity, WebView2
Project B                 ~40-80 MB    own UI, own protocol, partial parity
```

## What it has to do

The floor for a usable Discord client is smaller than the feature list suggests:

| Capability | Minimum viable | Notes |
|---|---|---|
| Login | email + password, or token | MFA and captcha are the hard part |
| Gateway | WebSocket, identify, heartbeat, resume | `wss://gateway.discord.gg/?encoding=json&v=10` |
| Read | guilds, channels, messages, history | REST + gateway dispatch |
| Write | send, edit, delete, react | REST |
| Presence | typing, online/offline | gateway opcodes |
| Voice | join, speak, listen | Opus + WebRTC. **This is the wall.** |
| Media | images, embeds, avatars | fetch + decode + cache |

## What "done" looks like at each stage

Stage 1 is the honest first target and it is achievable:

**Stage 1 — read-only client.** Log in with a token, connect to the gateway, list
guilds and channels, read message history. No sending, no voice. This proves the
protocol client works and gives something you can look at.

**Stage 2 — writing.** Send, edit, delete, react. Now it is a usable client for
text.

**Stage 3 — presence and typing.** Gateway presence updates, typing indicators.

**Stage 4 — voice.** The wall. `serenity` and `songbird` exist; wiring them means
carrying an Opus encoder and a WebRTC stack.

**Stage 5 — attachment/media.** Image decode, cache, inline preview.

## Architecture

```
crates/
  huskcord-core/       protocol: REST client, gateway socket, event model
  huskcord-ui/         egui or iced, renders state, emits intents
  huskcord-app/        binary: wires core into ui, owns the session
```

Separating the protocol from the UI is the whole design. A protocol crate can be
tested against Discord without a window, and a UI can be developed against a
fake event source.

## Dependencies under consideration

| Need | Candidate | Caveat |
|---|---|---|
| Discord protocol | `serenity` | Mature, async, opinionated. Pulls tokio + TLS |
| Gateway only | `twilight` | More modular than serenity |
| Voice | `songbird` | Works with serenity; the heavy part |
| UI | `egui` | Immediate mode, tiny, no native deps |
| UI | `iced` | Elm-style, more structure, heavier |
| Async runtime | `tokio` | Unavoidable for a socket client |
| HTTP | `reqwest` | Standard |

`egui` is the default bet: it compiles to a small binary, has no system
dependencies, and text-heavy UIs are its strength. `iced` is the alternative if
the layout starts fighting back.

## Constraints inherited from the shell

1. **No Chromium.** That is the entire point. No WebView2, no embedded browser.
2. **The 40-80 MB target is real.** If this lands at 300 MB, it failed and the
   shell won.
3. **Do not touch main's constraints.** This branch may diverge freely, but the
   shell keeps working.

## Constraints that are new

1. **Automating a user account is against Discord's Terms of Service.** A client
   that logs in with a user token is *not* sanctioned. Self-botting gets accounts
   terminated. Any public release of this is a real risk to its users; that has to
   be stated plainly wherever it ships.
2. **Bot accounts are the sanctioned path.** A bot token has a different gateway
   flow, different rate limits, and no voice in most cases.
3. **A Discord application id is needed** for anything beyond plain user flow.

## Status

Not started. This branch holds the plan, the stage targets, and the constraints,
so the decision is recorded before any code exists.

The first real commit will be `huskcord-core`: a gateway client that connects,
identifies, and prints one READY payload. Nothing visual until Stage 1 is proven.

## Why this is in the same repo

The measurement work in `bench/` is shared, the naming is shared, and the person
is the same. It is not in the same *product*, which is why it is a branch and not
a rewrite.
