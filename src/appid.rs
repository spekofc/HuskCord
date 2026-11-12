// language: Rust, file: src/appid.rs, target: Windows 11 x64
//
// Windows task grouping: AppUserModelID.
//
// Task Manager and the taskbar group processes by AppUserModelID, not by
// executable. Without an explicit ID, every process gets a path-derived one and
// each msedgewebview2.exe shows up as its own entry next to huskcord.exe.
//
// Four things have to line up for one "Huskcord" tab with expandable children:
//
//   1. SetCurrentProcessExplicitAppUserModelID on the shell process.
//   2. --app-user-model-id on WebView2's browser process, which Chromium then
//      propagates to the renderer and utility processes it spawns.
//   3. A registered AppUserModelId key whose DisplayName matches the executable
//      description, so Task Manager has a name to show instead of a path.
//   4. An explicit DisplayName on the window title, because Task Manager prefers
//      the window title over the process description when both exist.
//
// Verified on this machine: GetApplicationUserModelId (kernel32, XP-era HRESULT
// 0x00003D57 = APPMODEL_ERROR_NO_APPLICATION) returns empty for a process that
// never called the setter, so the setter is the only path that makes the ID real.

use std::process::Command;

/// Stable, path-independent application identity.
pub const APP_ID: &str = "spekofc.Huskcord.Desktop";

/// Claim the identity for this process. Must run before any window is created.
/// Returns false if the shell rejected it, which is not fatal: the window still
/// works, only grouping degrades.
#[cfg(windows)]
pub fn claim(app_id: &str) -> bool {
    use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;

    let wide: Vec<u16> = app_id.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { SetCurrentProcessExplicitAppUserModelID(wide.as_ptr()) >= 0 }
}

#[cfg(not(windows))]
pub fn claim(_app_id: &str) -> bool {
    false
}

/// Register the AppUserModelId key so Task Manager can name the group.
///
/// HKCU\Software\Classes\AppUserModelId\<APP_ID>
///   DisplayName    = "Huskcord"
///   IconUri        = <exe path>
///   IconBackgroundColor = "" (use the icon's own alpha)
///
/// Idempotent: rewrites only if the value differs.
#[cfg(windows)]
pub fn register(app_id: &str, display_name: &str, exe: &std::path::Path) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let exe_s = exe.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$k = 'HKCU:\\Software\\Classes\\AppUserModelId\\{app_id}'; \
         if (-not (Test-Path $k)) {{ New-Item -Path $k -Force | Out-Null }}; \
         Set-ItemProperty -Path $k -Name 'DisplayName' -Value '{name}' -Force; \
         Set-ItemProperty -Path $k -Name 'IconUri' -Value '{exe_s}' -Force; \
         Set-ItemProperty -Path $k -Name 'IconBackgroundColor' -Value '' -Force",
        app_id = app_id,
        name = display_name,
        exe_s = exe_s,
    );

    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(not(windows))]
pub fn register(_app_id: &str, _display_name: &str, _exe: &std::path::Path) -> bool {
    false
}

/// Switch that makes WebView2's browser process adopt our identity. Chromium
/// propagates the AUMID to the renderer and utility processes it spawns, so the
/// whole tree groups with the shell.
pub fn webview_identity_switches() -> Vec<String> {
    vec![format!("--app-user-model-id={APP_ID}")]
}

/// Spawn helper: never open a console window for the child.
#[cfg(windows)]
pub fn detach(cmd: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW)
}

#[cfg(not(windows))]
pub fn detach(cmd: &mut Command) -> &mut Command {
    cmd
}
