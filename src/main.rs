// language: Rust, file: src/main.rs, target: Windows 11 x64, MSVC
//
// Huskcord — a native shell for Discord's web app.
//
// Replaces Electron's process stack (main, GPU, network and audio utilities,
// crashpad) with one ~900 KB binary and a WebView2 window. Discord's UI is
// unchanged: the server-delivered React app, in the same Chromium engine family,
// with Discord's own client user agent.
//
// Usage:
//   huskcord                      %APPDATA%\Huskcord
//   huskcord --profile work       separate session
//   huskcord --portable           ./data next to the exe
//   huskcord --data-dir <path>    explicit
//   huskcord --flag <switch>      append one extra Chromium switch (repeatable)
//   huskcord --no-tray --no-presence --no-shortcuts
//
// Build: cargo build --release -> target/release/huskcord.exe

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod icon;
mod notify;
mod rich_presence;
mod session;
mod shortcuts;
mod tray;

use std::path::PathBuf;
use std::time::Duration;

use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
use wry::{WebViewBuilder, WebViewBuilderExtWindows, WebContext};

const DEFAULT_URL: &str = "https://discord.com/app";

// Discord's own client string: discord.com/app gates part of its feature surface
// on the presence of the discord/ and Electron/ tokens.
const UA_DESKTOP: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) discord/1.0.9267 Chrome/140.0.0.0 Electron/42.7.1 Safari/537.36";

// Chromium systems a chat client never uses. Unknown names are ignored by
// Chromium, so over-listing is safe; listing something Discord needs is not.
//
// NOTE: every entry here was measured or reasoned about. Do not add switches
// that disable compositing - see docs/memory-experiments.md.
const DISABLED_FEATURES: &str = "\
MediaRouter,\
OptimizationHints,\
InterestFeedContentSuggestions,\
Translate,\
TranslateUI,\
AutofillServerCommunication,\
CalculateNativeWinOcclusion,\
HardwareMediaKeyHandling,\
MediaSessionService,\
IdleDetection,\
DialMediaRouteProvider,\
GlobalMediaControls,\
SpareRendererForSitePerProcess,\
BackForwardCache,\
LensOverlay,\
SideSearch,\
BrowsingTopics,\
PrivacySandboxAdsAPIs,\
msWebOOUI,\
msPdfOOUI,\
msSmartScreenProtection";

const BROWSER_SWITCHES: [&str; 15] = [
    "--disable-background-networking",
    "--disable-component-update",
    "--disable-sync",
    "--disable-domain-reliability",
    "--disable-client-side-phishing-detection",
    "--disable-pdf-extension",
    "--disable-breakpad",
    "--metrics-recording-only",
    "--no-first-run",
    "--no-default-browser-check",
    "--renderer-process-limit=1",
    "--process-per-site",
    "--enable-low-end-device-mode",
    "--disable-smooth-scrolling",
    "--disable-extensions",
];

fn browser_args(extra: &[String]) -> String {
    let mut args = String::with_capacity(1200);
    args.push_str("--disable-features=");
    args.push_str(DISABLED_FEATURES);
    for s in BROWSER_SWITCHES {
        args.push(' ');
        args.push_str(s);
    }
    for s in extra {
        args.push(' ');
        args.push_str(s);
    }
    args
}

/// Messages the page sends us through window.ipc.postMessage.
fn on_ipc(body: String) {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
        match v.get("kind").and_then(|k| k.as_str()) {
            Some("notify") => {
                let p = v.get("payload").cloned().unwrap_or_default();
                let title = p.get("title").and_then(|s| s.as_str()).unwrap_or("Huskcord");
                let text = p.get("body").and_then(|s| s.as_str()).unwrap_or("");
                notify::show(title, text);
            }
            Some("badge") => {
                let _n = v
                    .get("payload")
                    .and_then(|p| p.get("count"))
                    .and_then(|c| c.as_u64())
                    .unwrap_or(0);
            }
            _ => {}
        }
    }
}

/// Tear down in dependency order, then leave.
///
/// The "Not Responding" window on close was a teardown-order bug: the event
/// loop exited while the WebView2 controller and the tray thread were still
/// live. Close the webview first, then the tray, then terminate our own
/// msedgewebview2 tree (it otherwise outlives us by several seconds).
fn shutdown(webview: &mut Option<wry::WebView>, tray: &mut Option<tray::Tray>, data_dir: &str) {
    *webview = None;
    *tray = None;
    std::thread::sleep(Duration::from_millis(120));
    kill_webview_children(data_dir);
    std::process::exit(0);
}

fn kill_webview_children(data_dir: &str) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let marker = data_dir.replace('\'', "''");
    let script = format!(
        "$m='{marker}'; \
         Get-CimInstance Win32_Process -Filter \"Name='msedgewebview2.exe'\" -EA SilentlyContinue | \
         Where-Object {{ $_.CommandLine -and $_.CommandLine.Contains($m) }} | \
         ForEach-Object {{ try {{ $_.Terminate() | Out-Null }} catch {{}} }}",
        marker = marker
    );

    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

struct AppState {
    url: String,
    profile: Option<session::Profile>,
    args: Vec<String>,
    extra_flags: Vec<String>,
    /// Resolved WebView2 data dir, kept so teardown can identify our own
    /// msedgewebview2 processes by command line.
    data_dir: String,
    enable_tray: bool,
    enable_presence: bool,
    window: Option<Window>,
    webview: Option<wry::WebView>,
    tray: Option<tray::Tray>,
    presence: Option<rich_presence::Presence>,
}

impl AppState {
    fn new(url: String, profile: session::Profile, args: &[String], extra_flags: Vec<String>) -> Self {
        Self {
            url,
            profile: Some(profile),
            args: args.to_vec(),
            extra_flags,
            data_dir: String::new(),
            enable_tray: !args.iter().any(|a| a == "--no-tray"),
            enable_presence: !args.iter().any(|a| a == "--no-presence"),
            window: None,
            webview: None,
            tray: None,
            presence: None,
        }
    }
}

impl ApplicationHandler for AppState {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let mut attrs = Window::default_attributes()
            .with_title("Huskcord")
            .with_inner_size(LogicalSize::new(1280.0, 800.0))
            .with_min_inner_size(LogicalSize::new(800.0, 600.0))
            .with_visible(true);

        if let Some(ic) = icon::load() {
            attrs = attrs.with_window_icon(Some(ic));
        }

        let window = match event_loop.create_window(attrs) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("window creation failed: {e}");
                event_loop.exit();
                return;
            }
        };

        // Store the window FIRST: anything below that fails hard must not leave
        // the app with a live window and no handle to it.
        self.window = Some(window);

        let size = self.window.as_ref().unwrap().inner_size();

        // ---- persistent WebView2 profile --------------------------------
        let data_dir: PathBuf = self
            .profile
            .as_ref()
            .map(|p| p.dir.clone())
            .unwrap_or_else(|| PathBuf::from("./huskcord-data"));

        if let Err(e) = std::fs::create_dir_all(&data_dir) {
            eprintln!("profile dir {} not creatable: {e}", data_dir.display());
        }
        self.data_dir = data_dir.to_string_lossy().into_owned();

        // ---- first-run integration ---------------------------------------
        // Start menu entry + desktop shortcut, once per profile. A failure here
        // is cosmetic, never fatal.
        if shortcuts::should_run(&data_dir, &self.args) {
            let portable = self.profile.as_ref().map(|p| p.portable).unwrap_or(false);
            match shortcuts::install(&data_dir, portable) {
                Ok(()) => eprintln!("shortcuts installed (start menu + desktop)"),
                Err(e) => eprintln!("shortcuts skipped: {e}"),
            }
        }

        eprintln!(
            "profile: {}  ({})",
            self.profile.as_ref().map(|p| p.name.as_str()).unwrap_or("default"),
            data_dir.display()
        );

        let mut context = WebContext::new(Some(data_dir));

        let builder = WebViewBuilder::new_with_web_context(&mut context)
            .with_url(&self.url)
            .with_user_agent(UA_DESKTOP)
            .with_transparent(false)
            .with_browser_accelerator_keys(false)
            .with_additional_browser_args(browser_args(&self.extra_flags))
            .with_initialization_script(&notify::init_script())
            .with_ipc_handler(|req| on_ipc(req.body().to_string()))
            .with_devtools(cfg!(debug_assertions));

        let win = self.window.as_ref().expect("window stored above");
        match builder.build_as_child(win) {
            Ok(wv) => {
                let _ = wv.set_bounds(wry::Rect {
                    position: LogicalPosition::new(0, 0).into(),
                    size: LogicalSize::new(size.width as f64, size.height as f64).into(),
                });
                eprintln!("webview ok: {}", self.url);
                self.webview = Some(wv);
            }
            Err(e) => {
                eprintln!("webview build failed: {e}");
                event_loop.exit();
                return;
            }
        }

        // ---- tray --------------------------------------------------------
        if self.enable_tray {
            self.tray = tray::Tray::new("Huskcord");
            match &self.tray {
                Some(_) => eprintln!("tray icon installed"),
                None => eprintln!("tray unavailable (continuing without it)"),
            }
        }

        // ---- rich presence ----------------------------------------------
        if self.enable_presence {
            if let Ok(cid) = std::env::var("HUSKCORD_CLIENT_ID") {
                self.presence = rich_presence::Presence::connect(&cid);
                if self.presence.is_some() {
                    eprintln!("rich presence connected");
                } else {
                    eprintln!("rich presence: no Discord IPC pipe found");
                }
            }
        }
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => shutdown(&mut self.webview, &mut self.tray, &self.data_dir),
            WindowEvent::Resized(size) => {
                if let Some(wv) = &self.webview {
                    let _ = wv.set_bounds(wry::Rect {
                        position: LogicalPosition::new(0, 0).into(),
                        size: LogicalSize::new(size.width as f64, size.height as f64).into(),
                    });
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // tray events arrive on another thread; drain them here.
        // Collect first: handling a Quit needs &mut self.
        let mut events = Vec::new();
        if let Some(tray) = &self.tray {
            while let Some(ev) = tray.poll() {
                events.push(ev);
            }
        }
        for ev in events {
            match ev {
                tray::TrayEvent::Quit => {
                    shutdown(&mut self.webview, &mut self.tray, &self.data_dir);
                }
                tray::TrayEvent::Show | tray::TrayEvent::Activate => {
                    if let Some(w) = &self.window {
                        w.set_visible(true);
                        w.focus_window();
                    }
                }
                tray::TrayEvent::ToggleMute => {}
            }
        }

        // drain Discord IPC replies without blocking the pump
        if let Some(p) = &mut self.presence {
            p.pump();
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // --flag <switch> may repeat. Useful for measuring a Chromium switch without
    // rebuilding; see docs/memory-experiments.md.
    let mut extra_flags = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--flag" {
            if let Some(v) = args.get(i + 1) {
                extra_flags.push(v.clone());
                i += 1;
            }
        }
        i += 1;
    }

    let profile = session::resolve();
    let url = std::env::var("DISCORD_SHELL_URL").unwrap_or_else(|_| DEFAULT_URL.into());

    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut state = AppState::new(url, profile, &args, extra_flags);

    if let Err(e) = event_loop.run_app(&mut state) {
        eprintln!("event loop error: {e}");
        std::process::exit(1);
    }
}
