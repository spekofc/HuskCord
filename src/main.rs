// language: Rust, file: src/main.rs, target: Windows 11 x64, MSVC
//
// Huskcord — a native shell for Discord's web app.
//
// Replaces Electron's process stack (main, GPU, network and audio utilities,
// crashpad) with one ~20 MB binary and a WebView2 window. Discord's UI is
// unchanged: the server-delivered React app, in the same Chromium engine family,
// with Discord's own client user agent.
//
// What this shell adds on top of a bare browser window:
//   session    persistent profile that survives restarts (session.rs)
//   tray       system tray icon with show/quit (tray.rs)
//   notify     OS toasts instead of browser notifications (notify.rs)
//   presence   Rich Presence over Discord's local IPC pipe (rich_presence.rs)
//
// Usage:
//   huskcord                      %APPDATA%\Huskcord
//   huskcord --profile work       separate session
//   huskcord --portable           ./data next to the exe
//   huskcord --data-dir <path>    explicit
//   huskcord --no-tray --no-presence
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

fn browser_args() -> String {
    let mut args = String::with_capacity(1024);
    args.push_str("--disable-features=");
    args.push_str(DISABLED_FEATURES);
    for s in BROWSER_SWITCHES {
        args.push(' ');
        args.push_str(s);
    }
    args
}

/// Messages the page sends us through window.ipc.postMessage.
fn on_ipc(body: String) {
    // {"kind":"notify"|"badge","payload":{...}}
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
        match v.get("kind").and_then(|k| k.as_str()) {
            Some("notify") => {
                let p = v.get("payload").cloned().unwrap_or_default();
                let title = p.get("title").and_then(|s| s.as_str()).unwrap_or("Huskcord");
                let text = p.get("body").and_then(|s| s.as_str()).unwrap_or("");
                notify::show(title, text);
            }
            Some("badge") => {
                let n = v
                    .get("payload")
                    .and_then(|p| p.get("count"))
                    .and_then(|c| c.as_u64())
                    .unwrap_or(0);
                set_badge(n);
            }
            _ => {}
        }
    }
}

/// Taskbar overlay badge. Zero means clear.
fn set_badge(_count: u64) {
    // TODO: overlay composited icon via ITaskbarList3::SetOverlayIcon.
    // Left as a no-op so the IPC path is wired end to end; the shim already
    // reports the number, and wiring the icon is a self-contained follow-up.
}

struct AppState {
    url: String,
    profile: Option<session::Profile>,
    args: Vec<String>,
    enable_tray: bool,
    enable_presence: bool,
    window: Option<Window>,
    webview: Option<wry::WebView>,
    tray: Option<tray::Tray>,
    presence: Option<rich_presence::Presence>,
    presence_start: u64,
}

impl AppState {
    fn new(url: String, profile: session::Profile, args: &[String]) -> Self {
        Self {
            url,
            profile: Some(profile),
            args: args.to_vec(),
            enable_tray: !args.iter().any(|a| a == "--no-tray"),
            enable_presence: !args.iter().any(|a| a == "--no-presence"),
            window: None,
            webview: None,
            tray: None,
            presence: None,
            presence_start: 0,
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

        let size = window.inner_size();

        // Store the window FIRST. Anything below that fails hard (WebView2
        // controller, tray thread) must not leave the app with a live window
        // and no handle to it - that is exactly the silent-blank-window mode
        // documented in docs/tao-vs-winit.md.
        self.window = Some(window);

        // ---- persistent WebView2 profile --------------------------------
        let data_dir: PathBuf = self
            .profile
            .as_ref()
            .map(|p| p.dir.clone())
            .unwrap_or_else(|| PathBuf::from("./huskcord-data"));

        if let Err(e) = std::fs::create_dir_all(&data_dir) {
            eprintln!("profile dir {} not creatable: {e}", data_dir.display());
        }

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

        // WebContext must outlive the WebViewBuilder call but not the app; it is
        // dropped at the end of this scope, which is fine because WebView2
        // copies the data-dir string into its own environment at creation.
        let mut context = WebContext::new(Some(data_dir));

        let builder = WebViewBuilder::new_with_web_context(&mut context)
            .with_url(&self.url)
            .with_user_agent(UA_DESKTOP)
            .with_transparent(false)
            .with_browser_accelerator_keys(false)
            .with_additional_browser_args(browser_args())
            // notifications + badge bridge
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
                // A shell without a renderer is worse than no shell: exit loud.
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
                    self.presence_start = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    eprintln!("rich presence connected");
                } else {
                    eprintln!("rich presence: no Discord IPC pipe found");
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
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

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // tray events arrive on another thread; drain them here
        if let Some(tray) = &self.tray {
            while let Some(ev) = tray.poll() {
                match ev {
                    tray::TrayEvent::Quit => event_loop.exit(),
                    tray::TrayEvent::Show | tray::TrayEvent::Activate => {
                        if let Some(w) = &self.window {
                            w.set_visible(true);
                            w.focus_window();
                        }
                    }
                    tray::TrayEvent::ToggleMute => { /* media key path is separate */ }
                }
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
    let profile = session::resolve();
    let url = std::env::var("DISCORD_SHELL_URL").unwrap_or_else(|_| DEFAULT_URL.into());

    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut state = AppState::new(url, profile, &args);

    if let Err(e) = event_loop.run_app(&mut state) {
        eprintln!("event loop error: {e}");
        std::process::exit(1);
    }
}
