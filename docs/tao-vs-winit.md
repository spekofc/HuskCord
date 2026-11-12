# tao vs winit: why the first shell never rendered

Attempt 1 of Huskcord used `tao` 0.34 for the window. It built, launched, and
opened a real 1280x800 window titled "Discord". It never rendered anything.

## What we saw

```text
shell process     19.0 MB, window handle valid, MainWindowTitle "Discord"
msedgewebview2    1 process  (browser only)
  children        1  ->  vsjitdebugger.exe
renderer          0 processes
gpu-process       0 processes
utilities         0 processes
```

That `vsjitdebugger.exe` child is Windows launching the Just-In-Time Debugger:
the WebView2 browser process faulted or hung at startup and Windows offered a
debugger instead of an error.

The shell stayed alive at 19 MB forever. No exit, no crash dialog, no log.

## Why

`tao` owns the main thread and runs its own message loop
(`ControlFlow::Wait`). `webview2-com` needs that same thread's message pump to
deliver COM completion callbacks. Looking at wry's WebView2 backend:

```rust
// wry-0.53.5/src/webview2/mod.rs
fn create_controller(hwnd, env, ...) -> Result<ICoreWebView2Controller> {
    let (tx, rx) = mpsc::channel();
    let handler = CreateCoreWebView2ControllerCompletedHandler::create(...);
    env.CreateCoreWebView2Controller(hwnd, &handler)?;
    webview2_com::wait_with_pump(rx)?   // <-- needs the thread's pump
}
```

`wait_with_pump` spins a nested pump while waiting for the controller. Under
`tao` the outer loop already owns the thread contract, so the controller
completion never arrives and the call never returns. The process sits in a
wait state with a valid window and no renderer.

Ten threads, all `Wait`/`EventPairLow`. CPU near zero. Nothing to see.

## The fix

`winit` 0.30 — the framework `wry`'s own `examples/winit.rs` targets. Same API
shape (`ApplicationHandler`, `resumed()`, `create_window`), but the pump handshake
between the window library and `webview2-com` works.

Same binary structure, one dependency swapped:

```text
tao  -> window ok, browser process only, renderer never spawns
winit -> full Chromium tree (browser, gpu, 2+ utilities, renderer) in ~2 s
```

Measured against `example.com`, immediately after the switch:

```text
t= 2s shell= 20MB wv2=6 [browser,crashpad-handler,gpu-process,utility,utility,renderer] tot=337.1MB
t=20s shell= 20MB wv2=6 [...] tot=348.9MB
```

## What this cost to find

The process tree was the tell, not the code. A window that exists and a shell
that idles look like success until you count renderers. The lesson is the
measurement: `prowl.ps1` checks for the process set, not just the parent.

## Bonus: the flags that made it worse

Attempt 3 added a large Chromium kill-list (`--disable-features=MediaRouter,...`)
to the working winit shell. Measured result: **~780 MB**, against attempt 2's
613 MB. The feature-disable list was net negative on this machine.

What survived into the shipped binary is the subset that did not regress:
background-networking, component-update, sync, breakpad, domain-reliability,
phishing detection, PDF extension, extensions, and renderer-process-limit=1.

The regression is unattributed. Candidate causes, in order of suspected weight:
`--enable-low-end-device-mode` (changes caching and memory policy aggressively),
`BackForwardCache` disable (may cause re-decode of navigation entries),
`SpareRendererForSitePerProcess` disable (forces cold renderer spawns).

Anyone reproducing this should bisect the flag list one at a time rather than
trusting the aggregate.
