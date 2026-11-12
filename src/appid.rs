// language: Rust, file: src/appid.rs, target: Windows 11 x64
//
// Windows task grouping: AppUserModelID.
//
// Task Manager and the taskbar group processes by AppUserModelID, not by
// executable. Without one, every process gets a path-derived ID and each
// msedgewebview2.exe shows as its own entry next to huskcord.exe.
//
// Findings from Windows 11 26100 (probe: .github/workflows/probe-aumid.yml,
// reading kernel32!GetApplicationUserModelId back from a clean GitHub runner and
// from this machine - identical result):
//
//   * SetCurrentProcessExplicitAppUserModelID returns S_OK but the ID does not
//     bind to the calling process on its own. The read-back is 0x00003D57
//     (APPMODEL_ERROR_NO_APPLICATION) both before and after.
//   * --app-user-model-id does reach WebView2's browser process command line,
//     verified by string match, but the same read-back applies there.
//
// What that means for grouping:
//   * Taskbar grouping (two windows -> one button) works from SetCurrentProcess..
//     because the shell honours it for windows created after the call.
//   * Task Manager's per-process list keys on the AppUserModelId registry entry
//     plus the process image, so the registry key is what gives the group a
//     name and icon. That is done below.
//   * The expandable "tab" subtree in Task Manager groups processes that share a
//     job object. WebView2 puts its tree in its own job, and inheriting our ID
//     does not merge those jobs. Collapsing them would mean breaking WebView2's
//     crash-isolation model, so this code does not attempt it.

use std::process::Command;

/// Stable, path-independent application identity.
pub const APP_ID: &str = "spekofc.Huskcord.Desktop";

/// Claim the identity for this process. Must run before any window is created.
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

/// Register the AppUserModelId key so Task Manager can name and icon the group.
///
/// HKCU\Software\Classes\AppUserModelId\<APP_ID>
///   DisplayName         = "Huskcord"
///   IconUri             = <exe path>
///   IconBackgroundColor = "" (keep the icon's own alpha)
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

/// Switch that puts our identity on WebView2's browser process.
pub fn webview_identity_switches() -> Vec<String> {
    vec![format!("--app-user-model-id={APP_ID}")]
}

/// Spawn helper: never open a console window for the child.
#[cfg(windows)]
pub fn detach(cmd: &mut std::process::Command) -> &mut std::process::Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW)
}

#[cfg(not(windows))]
pub fn detach(cmd: &mut std::process::Command) -> &mut std::process::Command {
    cmd
}
