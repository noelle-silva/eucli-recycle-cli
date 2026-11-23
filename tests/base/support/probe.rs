#![allow(dead_code)]

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use std::os::windows::ffi::{OsStrExt, OsStringExt};

pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Recursively collects all `.rs` files under `src/`.
pub fn all_src_rs_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_rs(&workspace_root().join("src"), &mut out);
    out.sort();
    out
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_rs(&p, out);
            } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(p);
            }
        }
    }
}

/// Probes whether 8.3 short name exists for a given path using Windows `GetShortPathNameW`.
#[cfg(windows)]
pub fn probe_short_path(path: &Path) -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetShortPathNameW;

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut buf = vec![0u16; 1024];
    let len = unsafe { GetShortPathNameW(PCWSTR(wide.as_ptr()), Some(&mut buf)) };
    if len == 0 {
        return None;
    }
    let short = OsString::from_wide(&buf[..len as usize])
        .to_string_lossy()
        .to_string();
    let orig = path.to_string_lossy().to_string();
    if short.eq_ignore_ascii_case(&orig) {
        None
    } else {
        Some(short)
    }
}

#[cfg(not(windows))]
pub fn probe_short_path(_path: &Path) -> Option<String> {
    None
}
