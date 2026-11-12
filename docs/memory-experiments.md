# Memory experiments

Everything tried to reduce Huskcord's footprint, what it measured, and why it
was kept or dropped. The rule for this file: a number only counts if the flags
used were confirmed on the process command line, and the renderer actually
loaded Discord.

## Where the memory goes

Settled, `discord.com/app` loaded, 8 processes:

```
shell (huskcord.exe)    20 MB
renderer               130-250 MB    <- Discord's React app, varies with load
browser                110-125 MB    <- Chromium host: session, network, storage
gpu-process             57-110 MB    <- compositor
network utility         50-55 MB
storage utility         20 MB
audio utility           22 MB
crashpad                18 MB
```

Total observed range: **405-610 MB** depending on cache warmth and how much of
Discord has been walked through. Against Discord desktop's settled ~1,065 MB.

## Attempt 1: the tao/winit swap

Not a memory change, an existence change. `tao` never spawned the renderer.
See [tao-vs-winit.md](tao-vs-winit.md).

## Attempt 2: big Chromium feature kill-list

Added ~20 `--disable-features` entries. Result: **~780 MB**, worse than the
613 MB baseline.

Kept only the subset that did not regress. The suspected culprit was
`--enable-low-end-device-mode`, which changes caching and memory policy
aggressively and was later confirmed in attempt 4 below to be an outlier.

## Attempt 3: blanket `--disable-gpu`

First run said "no-gpu saves 18 MB". That measurement was **invalid**: a
self-measuring harness had driven the event loop with `ControlFlow::Poll`,
which starved the WebView2 pump so the renderer never loaded. Two processes,
one page that never rendered, and a number that looked like a win.

Reverted the harness. Measure with `bench/measure.ps1`, which samples a real
launched process after Discord has actually painted.

## Attempt 4: per-process, flags confirmed on the command line

Ran twice, fresh profile each time.

| config | gpu-process | renderer | browser | total |
|---|---|---|---|---|
| baseline | 56.9 MB | 128.9 MB | 108.8 MB | 405.2 MB |
| `--disable-gpu` | 110.7 MB | 234.8 MB | 123.1 MB | 599.5 MB |

Second run, `--disable-gpu` only: 589.0 MB avg vs 563.5 MB baseline.

**Verdict: disabling the GPU is worse, not better.** Without GPU compositing,
Blink rasterises on the renderer thread: 128.9 MB → 234.8 MB. The gpu-process
does not even shrink. Nothing was shipped from this.

## Attempt 5: self-measuring harness sweep

Built a `--measure` mode that ran four configurations in one process. Results
looked plausible (no-gpu −18 MB) but were produced with the same starved pump
as attempt 3. **Discarded, removed from the tree.**

What survived is `--flag <switch>`, which appends a Chromium switch to the real
launch so a single flag can be A/B tested without rebuilding:

```powershell
.\bench\measure.ps1 -Label base -Settle 30
.\bench\measure.ps1 -Label test -Settle 30 -Flag "--some-switch"
```

`bench/measure.ps1` launches with `--flag`, samples the real process tree, and
prints per-process numbers.

## What did not get explored

Ordered by expected payoff, with the reason each was left alone:

1. **Storage/network utility trimming.** Two utility processes hold 70-90 MB
   combined. Disabling them breaks offline reads and WebSocket keepalive.
2. **Aggressive V8 heap caps** (`--js-flags=--max-old-space-size`). React plus
   Discord's client state does not tolerate a tight cap; a mid-session OOM in a
   chat client is worse than 50 MB.
3. **`--disable-features=CalculateNativeWinOcclusion`** is already off; the
   occlusion tracker was in the original kill-list. No isolated measurement of
   its own contribution exists.
4. **WebView2's own `--single-process`.** Crashes on any renderer fault and
   takes the shell with it.

## Floor

~400 MB is the practical floor for running Discord's real UI in a Chromium
engine. Below that requires not running Chromium at all, which is Project B: a
native client with its own UI (see the project roadmap). Estimated 40-80 MB,
at the cost of every server-delivered feature.
