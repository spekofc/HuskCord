// language: Rust, file: src/main.rs, target: Windows 11 x64, MSVC
//
// Huskcord — a native shell for Discord's web app.
//
// Replaces Electron's process stack (main, GPU, network and audio utilities,
// crashpad) with one ~20 MB binary and a WebView2 window. Discord's UI is
// unchanged: it is the server-delivered React app, in the same Chromium engine
// family, with Discord's own client user agent.
//
// Build: cargo build --release   ->  target/release/huskcord.exe
// Override the URL with DISCORD_SHELL_URL for testing.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
#[cfg(windows)]
use wry::WebViewBuilderExtWindows;

const DEFAULT_URL: &str = "https://discord.com/app";

// Discord's own client string. discord.com/app gates parts of its feature
// surface on the presence of the discord/ and Electron/ tokens.
const UA_DESKTOP: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) discord/1.0.9267 Chrome/140.0.0.0 Electron/42.7.1 Safari/537.36";

// Chromium systems a chat client never uses. Unknown feature names are ignored
// by Chromium, so over-listing is safe; listing something Discord needs is not.
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

// Switches that cut background work without changing page behaviour.
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

#[derive(Default)]
struct State {
    url: Option<String>,
    window: Option<Window>,
    webview: Option<wry::WebView>,
}

impl ApplicationHandler for State {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // resumed can fire more than once; build the window once.
        if self.window.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title("Huskcord")
            .with_inner_size(LogicalSize::new(1280.0, 800.0))
            .with_min_inner_size(LogicalSize::new(800.0, 600.0))
            .with_visible(true);

        let window = match event_loop.create_window(attrs) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("window creation failed: {e}");
                event_loop.exit();
                return;
            }
        };

        let url = self.url.clone().unwrap_or_else(|| DEFAULT_URL.into());
        let size = window.inner_size();

        let builder = wry::WebViewBuilder::new()
            .with_url(&url)
            .with_user_agent(UA_DESKTOP)
            .with_transparent(false)
            // a chat client is not a browser: no ctrl+T, no ctrl+F beep
            .with_browser_accelerator_keys(false)
            .with_additional_browser_args(browser_args())
            .with_devtools(cfg!(debug_assertions));

        match builder.build_as_child(&window) {
            Ok(wv) => {
                // without explicit bounds some WebView2 builds create the
                // child at 0x0, which reads as a blank window
                let _ = wv.set_bounds(wry::Rect {
                    position: LogicalPosition::new(0, 0).into(),
                    size: LogicalSize::new(size.width as f64, size.height as f64).into(),
                });
                eprintln!("webview ok: {url}");
                self.webview = Some(wv);
            }
            Err(e) => eprintln!("webview build failed: {e}"),
        }

        self.window = Some(window);
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => std::process::exit(0),
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
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut state = State {
        url: std::env::var("DISCORD_SHELL_URL").ok(),
        ..Default::default()
    };

    if let Err(e) = event_loop.run_app(&mut state) {
        eprintln!("event loop error: {e}");
        std::process::exit(1);
    }
}
