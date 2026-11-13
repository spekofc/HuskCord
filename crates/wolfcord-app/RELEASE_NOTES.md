# Wolfcord 0.1.0

A native Discord client. No Chromium, no WebView2, one process.

## Download

`wolfcord-windows-x64.zip` → unzip → set a token → run `wolfcord.exe`.

```powershell
$env:WOLFCORD_TOKEN = "your-bot-token"
.\wolfcord.exe
```

Without a token it opens an offline view that says so.

## Measured

| | RAM | Processes |
|---|---|---|
| Discord desktop | 1,064.9 MB | 8 |
| Huskcord (the WebView2 shell) | 600.9 MB | 8 |
| **Wolfcord** | **97.0 MB** | **1** |

Download is 11.9 MB, most of it the glyph atlas.

## Works

- Gateway connection with automatic resume
- Guild list, channel list, category ordering
- Message history, live messages, edits, deletes
- Unread counts
- Send and delete-own
- Text intents only

## Does not work yet

- **Voice** — not started. Needs Opus and WebRTC. This is the wall, not a missing flag.
- Screenshare — not started
- Attachments and embeds — text only
- DMs — channels exist, no inbox view
- Markdown — raw as sent
- Login UI — token comes from `WOLFCORD_TOKEN`

## Read this before using it

Automating a **user account violates Discord's Terms of Service** and accounts do
get terminated. **Bot tokens are the supported path.** Pointing this at a user
token is your call and your risk.

## What this is, compared to the shell

`huskcord` (main branch) runs Discord's actual React app in a WebView2 window:
full feature parity, 601 MB, eight processes, and Discord's updates arrive for
free.

`wolfcord` (this branch) draws its own UI: 97 MB, one process, and every Discord
feature is something to reimplement.

Same repo, two different answers to "make Discord smaller". Neither is a
compromise of the other.
