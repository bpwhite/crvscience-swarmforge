// Copyright (C) 2026 CRV Science Network (https://crvscience.com)
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://gnu.org>.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/swarmforge.ico");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR not set"));
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let ico_src = manifest_dir.join("assets").join("swarmforge.ico");
    if !ico_src.exists() {
        println!("cargo:warning=Missing assets/swarmforge.ico; skipping Windows PE resource embedding.");
        return;
    }

    let ico_dst = out_dir.join("swarmforge.ico");
    fs::copy(&ico_src, &ico_dst).expect("copy swarmforge.ico to OUT_DIR");

    let rc_path = out_dir.join("app.rc");
    let rc_content = r#"1 ICON "swarmforge.ico"

1 VERSIONINFO
FILEVERSION 1,3,0,0
PRODUCTVERSION 1,3,0,0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "CRV Science Network\0"
      VALUE "FileDescription", "SwarmForge - 3D Isometric Automated Robotic Factory & Swarm Intelligence Simulation\0"
      VALUE "FileVersion", "1.3.0.0\0"
      VALUE "InternalName", "swarmforge\0"
      VALUE "LegalCopyright", "Copyright CRV Science Network (GPL-3.0-or-later)\0"
      VALUE "OriginalFilename", "13_SwarmForge.exe\0"
      VALUE "ProductName", "SwarmForge\0"
      VALUE "ProductVersion", "1.3.0\0"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x0409, 1200
  END
END
"#;
    fs::write(&rc_path, rc_content).expect("write OUT_DIR/app.rc");

    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_env == "gnu" {
        let windres_candidates = ["windres", "x86_64-w64-mingw32-windres"];
        let mut found = None;
        for bin in windres_candidates {
            if let Ok(out) = Command::new(bin)
                .current_dir(&out_dir)
                .args(["-i", "app.rc", "-O", "coff", "-o", "app_res.o"])
                .output()
            {
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    panic!("{bin} failed compiling app.rc: {stderr}");
                }
                found = Some(out_dir.join("app_res.o"));
                break;
            }
        }
        if let Some(obj) = found {
            println!("cargo:rustc-link-arg={}", obj.display());
        } else {
            println!("cargo:warning=windres not found; Windows PE resources were not embedded.");
        }
    } else if target_env == "msvc" {
        if let Some(rc_bin) = find_msvc_rc() {
            let out = Command::new(&rc_bin)
                .current_dir(&out_dir)
                .args(["/nologo", "/fo", "app_res.res", "app.rc"])
                .output()
                .expect("spawn rc.exe");
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                panic!("rc.exe failed compiling app.rc: {stderr}");
            }
            println!("cargo:rustc-link-arg={}", out_dir.join("app_res.res").display());
        } else {
            println!("cargo:warning=rc.exe not found; Windows PE resources were not embedded.");
        }
    }
}

fn find_msvc_rc() -> Option<PathBuf> {
    if Command::new("rc.exe").arg("/?").output().is_ok() {
        return Some(PathBuf::from("rc.exe"));
    }
    let pf86 = env::var_os("ProgramFiles(x86)")?;
    let kits_bin = Path::new(&pf86).join("Windows Kits").join("10").join("bin");
    let entries = fs::read_dir(kits_bin).ok()?;
    let mut versions: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|ent| ent.path()))
        .filter(|p| p.is_dir())
        .collect();
    versions.sort();
    versions.reverse();
    for v in versions {
        let candidate = v.join("x64").join("rc.exe");
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}
