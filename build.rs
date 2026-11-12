// language: Rust, file: build.rs
//
// Embeds resources/windows.rc (icon + version info) into the exe.
//
// The MSVC linker consumes a compiled .res, not a COFF object. Earlier versions
// tried `llvm-windres -O coff-pe-x86-64` and passed the object with
// `cargo:rustc-link-arg`; link.exe rejected it and the exe shipped with no
// resource directory at all, so the shell and Start menu both showed the
// generic app icon.
//
// Order of preference:
//   1. rc.exe from the Windows SDK (produces a .res link.exe can consume)
//   2. llvm-windres -> .res is not supported, so windres is only a fallback for
//      .o-emitting toolchains and is skipped for MSVC targets
//
// If nothing is found the build still succeeds, just without the branded icon.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn find_rc() -> Option<PathBuf> {
    // 1. explicit override
    if let Ok(p) = std::env::var("HUSKCORD_RC") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }

    // 2. Windows SDK: %ProgramFiles(x86)%\Windows Kits\10\bin\<ver>\x64\rc.exe
    //    Newest version wins.
    for root in [
        std::env::var("ProgramFiles(x86)").ok(),
        std::env::var("ProgramFiles").ok(),
    ]
    .into_iter()
    .flatten()
    {
        let kits = Path::new(&root).join("Windows Kits").join("10").join("bin");
        if let Ok(entries) = fs::read_dir(&kits) {
            let mut versions: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            // sort so 10.0.26100.0 beats 10.0.22621.0 lexicographically-per-segment
            versions.sort();
            for v in versions.iter().rev() {
                let rc = v.join("x64").join("rc.exe");
                if rc.exists() {
                    return Some(rc);
                }
            }
        }
    }

    // 3. PATH
    for name in ["rc.exe", "rc"] {
        if Command::new(name).arg("/?").output().is_ok() {
            return Some(PathBuf::from(name));
        }
    }

    None
}

fn main() {
    println!("cargo:rerun-if-changed=assets/huskcord.ico");
    println!("cargo:rerun-if-changed=resources/windows.rc");
    println!("cargo:rerun-if-env-changed=HUSKCORD_RC");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let rc_src = Path::new("resources/windows.rc");
    if !rc_src.exists() {
        return;
    }

    // Rewrite the icon path to an absolute one so rc.exe finds it from OUT_DIR.
    let icon = fs::canonicalize("assets/huskcord.ico").expect("icon missing");
    let icon_win = icon.to_string_lossy().replace('\\', "/");
    let content = fs::read_to_string(rc_src)
        .expect("read rc")
        .replace("assets/huskcord.ico", &icon_win);
    let rc_out = out.join("windows.rc");
    fs::write(&rc_out, content).expect("write rc");

    let Some(rc_exe) = find_rc() else {
        println!(
            "cargo:warning=rc.exe not found (set HUSKCORD_RC); exe will have no icon resource"
        );
        return;
    };

    let res = out.join("resource.res");
    let status = Command::new(&rc_exe)
        .current_dir(&out)
        .arg("/nologo")
        .arg("/fo")
        .arg(&res)
        .arg(&rc_out)
        .status();

    match status {
        Ok(s) if s.success() && res.exists() => {
            // LINK accepts an object on the command line; a .res is passed the
            // same way and link.exe resolves it.
            println!("cargo:rustc-link-arg={}", res.display());
        }
        Ok(s) => {
            println!("cargo:warning=rc.exe exited {s}; exe icon skipped");
        }
        Err(e) => {
            println!("cargo:warning=rc.exe could not run ({e}); exe icon skipped");
        }
    }
}
