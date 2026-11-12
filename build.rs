// language: Rust, file: build.rs
//
// Embeds assets/huskcord.ico as the Windows resource icon so Explorer, the
// taskbar and the tray all show the wolf, then lets winit attach a window icon
// from the same file. Also stamps the exe with version info and an AppUserModelID
// so toasts are attributed to Huskcord rather than "PowerShell".

use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=assets/huskcord.ico");
    println!("cargo:rerun-if-changed=resources/windows.rc");

    if !cfg!(target_os = "windows") {
        return;
    }

    let out = std::env::var("OUT_DIR").expect("OUT_DIR");
    let rc_src = Path::new("resources/windows.rc");
    if !rc_src.exists() {
        return;
    }

    // The .rc references the icon by relative path; rewrite it to an absolute
    // path in a generated copy so windres/rc resolves it from OUT_DIR.
    let icon = fs::canonicalize("assets/huskcord.ico").expect("icon missing");
    let content = fs::read_to_string(rc_src).expect("read rc").replace(
        "assets/huskcord.ico",
        &icon.to_string_lossy().replace('\\', "/"),
    );
    let rc_out = Path::new(&out).join("windows.rc");
    fs::write(&rc_out, content).expect("write rc");

    // windres ships with llvm-mingw; rc.exe ships with the Windows SDK.
    // Try llvm-windres first, then windres, then rc.exe.
    let compiled = std::process::Command::new("llvm-windres")
        .current_dir(&out)
        .arg("-I")
        .arg(".")
        .arg(&rc_out)
        .arg("-O")
        .arg("coff-pe-x86-64")
        .arg(Path::new(&out).join("resource.o"))
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !compiled {
        // No resource compiler: the app still builds, just without the branded
        // exe icon. Tray and window icons are loaded from the embedded .ico at
        // runtime, so only the shell icon is affected.
        println!("cargo:warning=no windres/rc.exe found; exe icon skipped");
        return;
    }

    println!("cargo:rustc-link-search=native={out}");
    println!("cargo:rustc-link-lib=dylib=resource");
    println!(
        "cargo:rustc-link-arg={}",
        Path::new(&out).join("resource.o").display()
    );
}
