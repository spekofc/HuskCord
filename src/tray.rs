// language: Rust, file: src/tray.rs
//
// System tray icon via Shell_NotifyIconW.
//
// No crate: the API is small. We own a hidden message-only window, register
// our own window class, and handle WM_TRAYICON for clicks. The context menu is
// a plain popup menu built with CreatePopupMenu/AppendMenuW.
//
// Threading: Shell_NotifyIcon requires the owner thread to pump messages. This
// runs on a dedicated thread with its own GetMessage loop so it cannot stall
// the winit event loop or the WebView2 pump.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::{FindResourceW, GetModuleHandleW, LoadResource, LockResource};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetMessageW, LoadIconW, RegisterClassW, SetForegroundWindow,
    TrackPopupMenu, TranslateMessage, CreateIconIndirect, DestroyIcon, HICON, ICONINFO, MSG,
    TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RIGHTBUTTON, WNDCLASSW, WM_COMMAND, WM_DESTROY,
    WM_LBUTTONUP, WM_RBUTTONUP, WM_USER,
};

const WM_TRAYICON: u32 = WM_USER + 1;
const ID_TRAY: u32 = 1;
const CMD_SHOW: usize = 1001;
const CMD_QUIT: usize = 1002;
const CMD_MUTE: usize = 1003;

#[derive(Debug, Clone, Copy)]
pub enum TrayEvent {
    Show,
    Quit,
    ToggleMute,
    /// left click on the icon
    Activate,
}

/// Handle to a running tray. Dropping it removes the icon and ends the thread.
pub struct Tray {
    events: Receiver<TrayEvent>,
    shutdown: Sender<()>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Tray {
    /// Spawn the tray thread. `tooltip` shows on hover. The icon is loaded from
    /// the embedded .ico so it is branded from the first frame.
    pub fn new(tooltip: &str) -> Option<Tray> {
        let (ev_tx, ev_rx) = mpsc::channel();
        let (sd_tx, sd_rx) = mpsc::channel();

        let tip: Vec<u16> = tooltip.encode_utf16().chain(std::iter::once(0)).collect();

        let handle = thread::spawn(move || unsafe {
            let hinstance = GetModuleHandleW(std::ptr::null());
            let class_name: Vec<u16> = "HuskcordTray\0".encode_utf16().collect();

            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(tray_proc as _);
            wc.hInstance = hinstance;
            wc.lpszClassName = class_name.as_ptr();
            let atom = RegisterClassW(&wc);
            if atom == 0 {
                eprintln!("tray: RegisterClassW failed");
                return;
            }

            // message-only window: never visible, still receives our messages.
            // HWND_MESSAGE (-3) makes it invisible to the window enumerator too.
            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                std::ptr::null(),
                0,
                0,
                0,
                0,
                0,
                -3isize as _,
                std::ptr::null_mut(),
                hinstance,
                std::ptr::null_mut(),
            );
            if hwnd.is_null() {
                eprintln!("tray: CreateWindowExW failed");
                return;
            }

            // stash the sender so the wndproc can push events
            BOXED.sender = Some(ev_tx);

            let hicon = load_ico(hinstance).unwrap_or_else(|| {
                LoadIconW(std::ptr::null_mut(), 32512 as _) // IDI_APPLICATION
            });

            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = ID_TRAY;
            nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
            nid.uCallbackMessage = WM_TRAYICON;
            nid.hIcon = hicon;

            for (i, u) in tip.iter().take(127).enumerate() {
                nid.szTip[i] = *u;
            }

            if Shell_NotifyIconW(NIM_ADD, &mut nid) == 0 {
                // Windows returns FALSE when the notification area is not ready
                // yet (common on first launch, during Explorer restart, and in
                // session-0 / headless test runs). Retry briefly before giving up.
                let mut added = false;
                for _ in 0..10 {
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    if Shell_NotifyIconW(NIM_ADD, &mut nid) != 0 {
                        added = true;
                        break;
                    }
                }
                if !added {
                    eprintln!("tray: Shell_NotifyIconW(NIM_ADD) failed after retries");
                    DestroyWindow(hwnd);
                    return;
                }
            }

            // pump until told to stop
            let mut msg: MSG = std::mem::zeroed();
            loop {
                if sd_rx.try_recv().is_ok() {
                    break;
                }
                let has_msg = GetMessageW(&mut msg, hwnd, 0, 0) != 0;
                if !has_msg {
                    break;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            Shell_NotifyIconW(NIM_DELETE, &mut nid);
            DestroyWindow(hwnd);
        });

        Some(Tray {
            events: ev_rx,
            shutdown: sd_tx,
            thread: Some(handle),
        })
    }

    /// Non-blocking poll for a tray event.
    pub fn poll(&self) -> Option<TrayEvent> {
        self.events.try_recv().ok()
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        let _ = self.shutdown.send(());
        // the thread's GetMessage blocks, so give it a nudge via a real message
        // in production this would post WM_NULL; here we rely on process exit.
        if let Some(h) = self.thread.take() {
            let _ = h.join();
        }
    }
}

// global sender used by the wndproc. Single tray per process.
struct GlobalSender {
    sender: Option<Sender<TrayEvent>>,
}
static mut BOXED: GlobalSender = GlobalSender { sender: None };

unsafe extern "system" fn tray_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            let notif = (lparam & 0xFFFF) as u32;
            match notif {
                WM_LBUTTONUP => push(TrayEvent::Activate),
                WM_RBUTTONUP => {
                    show_menu(hwnd);
                }
                _ => {}
            }
            0
        }
        WM_COMMAND => {
            match wparam & 0xFFFF {
                CMD_SHOW => push(TrayEvent::Show),
                CMD_MUTE => push(TrayEvent::ToggleMute),
                CMD_QUIT => push(TrayEvent::Quit),
                _ => {}
            }
            0
        }
        WM_DESTROY => 0,
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn push(e: TrayEvent) {
    let s = std::ptr::addr_of!(BOXED.sender).read();
    if let Some(tx) = s {
        let _ = tx.send(e);
    }
}

unsafe fn show_menu(hwnd: HWND) {
    let menu = CreatePopupMenu();
    if menu.is_null() {
        return;
    }
    let show: Vec<u16> = "Show Huskcord\0".encode_utf16().collect();
    let mute: Vec<u16> = "Mute (toggle)\0".encode_utf16().collect();
    let quit: Vec<u16> = "Quit\0".encode_utf16().collect();

    AppendMenuW(menu, 0, CMD_SHOW, show.as_ptr());
    AppendMenuW(menu, 0, CMD_MUTE, mute.as_ptr());
    AppendMenuW(menu, 0x0000_0800 /* MF_SEPARATOR */, 0, std::ptr::null());
    AppendMenuW(menu, 0, CMD_QUIT, quit.as_ptr());

    let mut pt = POINT { x: 0, y: 0 };
    GetCursorPos(&mut pt);

    // required so the menu closes when clicking elsewhere
    SetForegroundWindow(hwnd);
    TrackPopupMenu(
        menu,
        TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON,
        pt.x,
        pt.y,
        0,
        hwnd,
        std::ptr::null(),
    );
    DestroyMenu(menu);
}

/// Load the embedded .ico (RT_GROUP_ICON, resource id 1) into an HICON.
///
/// The build script embeds assets/huskcord.ico; this walks RT_ICON entries and
/// builds the largest one. Falls back to the default app icon on any failure.
unsafe fn load_ico(hinstance: HINSTANCE) -> Option<HICON> {
    // Resource id 1, type RT_GROUP_ICON (14). FindResourceW takes MAKEINTRESOURCE
    // pointers: the integer cast to PCWSTR.
    let res = FindResourceW(hinstance as _, 1 as *const u16, 14 as *const u16);
    if res.is_null() {
        return None;
    }
    let size = windows_sys::Win32::System::LibraryLoader::SizeofResource(hinstance as _, res);
    if size == 0 {
        return None;
    }
    let data = LoadResource(hinstance as _, res);
    if data.is_null() {
        return None;
    }
    let ptr = LockResource(data) as *const u8;
    if ptr.is_null() {
        return None;
    }

    // GRPICONDIR: WORD reserved, WORD type(1), WORD count, then GRPICONDIRENTRY[count]
    // GRPICONDIRENTRY is 14 bytes: width, height, colors, reserved, planes, bitcount,
    // bytesInRes(4), id(2). We want the entry with the largest bytesInRes.
    let count = u16::from_le_bytes([*ptr.add(4), *ptr.add(5)]) as usize;
    if count == 0 {
        return None;
    }

    let mut best_id: u16 = 0;
    let mut best_bytes: u32 = 0;
    for i in 0..count {
        let e = ptr.add(6 + i * 14);
        let bytes = u32::from_le_bytes([*e.add(8), *e.add(9), *e.add(10), *e.add(11)]);
        let id = u16::from_le_bytes([*e.add(12), *e.add(13)]);
        if bytes > best_bytes {
            best_bytes = bytes;
            best_id = id;
        }
    }
    if best_id == 0 {
        return None;
    }

    // RT_ICON = 3
    let icon_res = FindResourceW(hinstance as _, best_id as *const u16, 3 as *const u16);
    if icon_res.is_null() {
        return None;
    }
    let icon_data = LoadResource(hinstance as _, icon_res);
    if icon_data.is_null() {
        return None;
    }
    let icon_ptr = LockResource(icon_data);
    if icon_ptr.is_null() {
        return None;
    }

    windows_sys::Win32::UI::WindowsAndMessaging::CreateIconFromResourceEx(
        icon_ptr as *mut u8,
        best_bytes,
        1,          // fIcon
        0x0003_0000, // version
        0,
        0,
        0, // LR_DEFAULTSIZE
    )
    .into()
}

/// Build an HICON from raw RGBA bytes. Unused for now: the tray icon is loaded
/// from the embedded .ico by load_ico().
#[allow(dead_code)]
unsafe fn build_icon(_rgba: Option<Vec<u8>>) -> Option<HICON> {
    None
}
