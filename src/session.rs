// language: Rust, file: src/session.rs
//
// Which WebView2 data directory to use.
//
// Command line:
//   huskcord                       -> %APPDATA%\Huskcord
//   huskcord --profile work        -> %APPDATA%\Huskcord\profiles\work
//   huskcord --portable            -> .\data next to the exe
//   huskcord --data-dir <path>     -> exactly <path>
//
// A profile is one logged-in session: cookies, localStorage, service worker,
// cache. Switching profiles means re-logging in, same as a second browser.

use std::path::PathBuf;

pub struct Profile {
    pub name: String,
    pub dir: PathBuf,
    pub portable: bool,
}

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn resolve() -> Profile {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if let Some(dir) = arg_value(&args, "--data-dir") {
        let p = PathBuf::from(&dir);
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "custom".into());
        return Profile {
            name,
            dir: p,
            portable: false,
        };
    }

    if args.iter().any(|a| a == "--portable") {
        return Profile {
            name: "portable".into(),
            dir: exe_dir().join("data"),
            portable: true,
        };
    }

    let name = arg_value(&args, "--profile").unwrap_or_else(|| "default".into());

    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| exe_dir())
        .join("Huskcord");

    // --profile default stays at the top so an upgrade from a version without
    // profiles keeps its login.
    let dir = if name == "default" {
        base
    } else {
        base.join("profiles").join(&name)
    };

    Profile {
        name,
        dir,
        portable: false,
    }
}
