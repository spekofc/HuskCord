// language: Rust, file: src/notify.rs
//
// OS notifications without pulling in a notification crate.
//
// Two paths, both fired from the same injected script:
//
//   1. Windows toast via PowerShell's Windows.UI.Notifications WinRT binding.
//      Spawn a hidden powershell with a one-liner toast XML. No dependency,
//      no COM registration, works with an unpackaged exe under the app's own
//      AppUserModelID.
//
//   2. Unread badge via the same PowerShell session: the taskbar badge is set
//      with the AppUserModelID override, which needs the shell to be told the
//      window belongs to our AUMID. We do that once at startup.
//
// The injected JS wraps window.Notification before the page loads, so Discord's
// web app calls our shim and never touches the browser notification API.

/// Script installed on every document before page scripts run.
pub fn init_script() -> String {
    r#"
(function () {
  if (window.__huskcordInjected) return;
  window.__huskcordInjected = true;

  const ipc = (window.__huskcordIpc || null);

  function post(kind, payload) {
    try {
      if (window.ipc && window.ipc.postMessage) {
        window.ipc.postMessage(JSON.stringify({ kind, payload }));
      }
    } catch (e) {}
  }

  // ---- notification shim -------------------------------------------------
  const Native = window.Notification;
  function HuskcordNotification(title, opts) {
    opts = opts || {};
    post('notify', { title: String(title || ''), body: String(opts.body || ''),
                     icon: String(opts.icon || ''), tag: String(opts.tag || '') });
    this.title = title; this.body = opts.body || ''; this.onclick = null;
    HuskcordNotification._last = this;
  }
  HuskcordNotification.permission = 'granted';
  HuskcordNotification.requestPermission = function (cb) {
    if (cb) cb('granted');
    return Promise.resolve('granted');
  };
  HuskcordNotification._last = null;
  window.Notification = HuskcordNotification;

  // ---- unread badge ------------------------------------------------------
  // Discord calls app.setBadgeCount; on web it is a no-op. We intercept the
  // document title, which Discord updates with the unread count, and forward.
  let lastBadge = -1;
  function pushBadge() {
    const m = document.title.match(/^\((\d+)\)/);
    const n = m ? parseInt(m[1], 10) : 0;
    if (n !== lastBadge) { lastBadge = n; post('badge', { count: n }); }
  }
  if (document.addEventListener) {
    document.addEventListener('DOMContentLoaded', pushBadge);
  }
  new MutationObserver(pushBadge).observe(document, { subtree: true, childList: true });
})();
"#
    .to_string()
}

/// PowerShell one-liner that raises a Windows toast.
pub fn toast_command(title: &str, body: &str) -> String {
    // single-quote the strings for PowerShell; escape embedded quotes
    let t = title.replace('\'', "''");
    let b = body.replace('\'', "''");

    format!(
        r#"powershell -NoProfile -WindowStyle Hidden -Command "
Add-Type -AssemblyName System.Runtime.WindowsRuntime | Out-Null
$ErrorActionPreference='SilentlyContinue'
$toast=[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType=WindowsRuntime]
[xml]$xml=@'
<toast><visual><binding template='ToastGeneric'>
<text>{t}</text><text>{b}</text>
</binding></visual></toast>
'@
$d=[Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02)
$d.GetElementsByTagName('text').Item(0).AppendChild($d.CreateTextNode('{t}')) | Out-Null
$d.GetElementsByTagName('text').Item(1).AppendChild($d.CreateTextNode('{b}')) | Out-Null
$n=[Windows.UI.Notifications.ToastNotification]::new($d)
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Huskcord').Show($n)
""#
    )
}

/// Fire a toast. Detached: we never wait on PowerShell.
pub fn show(title: &str, body: &str) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let _ = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &toast_command(title, body),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}
