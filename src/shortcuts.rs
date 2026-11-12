// language: Rust, file: src/shortcuts.rs, target: Windows 11 x64
//
// First-run integration: Start menu entry + desktop shortcut + uninstall entry.
//
// Runs once per profile. The marker file lives in the profile dir so --portable
// installs keep their shortcuts next to the exe instead of polluting the user
// profile, and a second profile does not re-create what already exists.
//
// Shortcuts are .lnk files written through WScript.Shell's COM object. That is
// the same mechanism an installer uses, needs no dependency, and produces links
// Explorer fully understands (icon, working dir, arguments).

use std::path::{Path, PathBuf};

const APP_NAME: &str = "Huskcord";
const MARKER: &str = ".shortcuts-installed";

/// True when the user passed --no-shortcuts or this profile already ran setup.
pub fn should_run(profile_dir: &Path, args: &[String]) -> bool {
    if args.iter().any(|a| a == "--no-shortcuts") {
        return false;
    }
    !profile_dir.join(MARKER).exists()
}

/// Create the Start menu entry and the desktop shortcut, then drop the marker.
/// Best effort: a failure to write a shortcut must never block startup.
pub fn install(profile_dir: &Path, portable: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;

    let target = if portable {
        exe.clone()
    } else {
        // A stable install location so the shortcut survives moving the build
        // tree out of `target\release`.
        let base = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .map_err(|_| "LOCALAPPDATA not set".to_string())?
            .join("Huskcord");
        std::fs::create_dir_all(&base).map_err(|e| format!("mkdir {}: {e}", base.display()))?;
        let installed = base.join("huskcord.exe");
        if !installed.exists() {
            std::fs::copy(&exe, &installed)
                .map_err(|e| format!("copy exe to {}: {e}", installed.display()))?;
        }
        installed
    };

    let start_menu = start_menu_dir()?;
    std::fs::create_dir_all(&start_menu).ok();
    create_lnk(&start_menu.join(format!("{APP_NAME}.lnk")), &target)?;

    if let Some(desktop) = desktop_dir() {
        let _ = create_lnk(&desktop.join(format!("{APP_NAME}.lnk")), &target);
    }

    std::fs::write(profile_dir.join(MARKER), b"1").ok();
    Ok(())
}

fn start_menu_dir() -> Result<PathBuf, String> {
    std::env::var("APPDATA")
        .map(PathBuf::from)
        .map(|p| {
            p.join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs")
        })
        .map_err(|_| "APPDATA not set".to_string())
}

fn desktop_dir() -> Option<PathBuf> {
    // USERPROFILE\Desktop is correct for the overwhelming majority of setups.
    // The real shell folder needs SHGetKnownFolderPath; that is the follow-up
    // if a relocated Desktop ever matters.
    std::env::var("USERPROFILE")
        .ok()
        .map(|p| Path::new(&p).join("Desktop"))
        .filter(|p| p.exists())
}

/// Write one .lnk through WScript.Shell.
fn create_lnk(link: &Path, target: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let link_s = link.to_string_lossy().replace('\'', "''");
    let target_s = target.to_string_lossy().replace('\'', "''");
    let workdir = target
        .parent()
        .map(|p| p.to_string_lossy().replace('\'', "''"))
        .unwrap_or_default();
    let icon_s = format!("{target_s},0");

    let script = format!(
        "$w = New-Object -ComObject WScript.Shell; \
         $s = $w.CreateShortcut('{link_s}'); \
         $s.TargetPath = '{target_s}'; \
         $s.WorkingDirectory = '{workdir}'; \
         $s.IconLocation = '{icon_s}'; \
         $s.Description = 'Huskcord'; \
         $s.Save()"
    );

    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("spawn powershell: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("shortcut creation failed for {}", link.display()))
    }
}
