# Contributing to Huskcord

Thanks for looking. This is a small project with a narrow purpose, so the fastest
way in is to read `AGENTS.md` first — it lists the constraints that will bite you.

## Before you write code

1. Read `docs/tao-vs-winit.md`. The first version of this project produced a
   window with no renderer and no error, and it looked like success.
2. Build and run it once: `cargo build --release` then
   `.\target\release\huskcord.exe --data-dir .\test-profile`.

## What is welcome

- Bug fixes, especially anything that makes a failure visible instead of silent
- New platform features that Discord's own desktop client has and we don't
  (tray badge, media keys, global push-to-talk, launch at login)
- Measured performance work. Bring numbers from `bench/prowl.ps1` before and after
- Documentation corrections, including this file

## What needs a discussion first

Open an issue before starting on:

- Anything that touches the event loop or the WebView2 creation path
- Adding a dependency. The whole point is a small binary; new crates need a reason
- Re-adding any disabled Chromium feature
- Anything that stores or transmits credentials

## Pull requests

- One concern per PR. A feature and a refactor in the same branch is two PRs
- Describe what you measured, not just what you changed
- If it changes memory behaviour, paste the before/after table
- `cargo build --release` must pass with no new warnings

## Reporting a security issue

Do not open a public issue. See `SECURITY.md`.

## Code style

Match what's there. `snake_case` functions, `PascalCase` types, comments that
explain *why* rather than *what*. No emoji in source.

## License

Contributions are MIT, same as the project. By opening a PR you agree your work
is licensed under the repository's `LICENSE`.
