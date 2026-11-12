# Security Policy

## Supported Versions

Huskcord is a small project. Only the latest release receives fixes.

| Version | Supported |
| ------- | --------- |
| 0.2.x   | Yes       |
| < 0.2   | No        |

## Reporting a Vulnerability

**Do not open a public issue for a security problem.**

Report privately through GitHub's advisory system:

1. Go to https://github.com/spekofc/HuskCord/security/advisories
2. Click **New draft security advisory**
3. Describe the issue, the version, and how to reproduce it

You will get a response within 72 hours. If the issue is confirmed, a fix
follows and the advisory is published with credit.

## Scope

Huskcord is a shell: it opens a WebView2 window pointing at `discord.com` and
does not handle accounts, tokens, or message content itself. The following are
in scope:

- IPC handler (`on_ipc` in `src/main.rs`) - parses JSON from the page and
  forwards to PowerShell for notifications
- The initialization script in `src/notify.rs` - runs inside the page, and any
  injection flaw there is in scope
- Profile/data directory handling (`src/session.rs`) - path construction from
  command line arguments
- The Rich Presence client (`src/rich_presence.rs`) - pipe path and framing

The following are **not** Huskcord issues:

- Vulnerabilities in Discord's web app
- Vulnerabilities in the WebView2 runtime
- Anything that requires a compromised local account

## Known design decisions

These are intentional, not vulnerabilities:

- **The user agent is spoofed** to carry Discord's own client string. This is
  required for `discord.com/app` to expose its desktop feature surface.
- **The injected script overrides `window.Notification`.** This is the whole
  point of the notification feature and is documented in `src/notify.rs`.
- **Toasts are raised by spawning `powershell.exe`.** There is no COM
  registration and no installer, so WinRT toast APIs are reached through the
  shell host. The command line is built from page-supplied strings and quoted;
  see `notify::toast_command`.

## Security-relevant configuration

Huskcord runs the same Chromium engine Discord's own desktop client bundles,
with a smaller feature set. It does **not** disable site isolation, mixed
content blocking, or HTTPS enforcement. It does disable a list of browser
features a chat client never uses (see `DISABLED_FEATURES` in `src/main.rs`).
