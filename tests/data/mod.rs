#![allow(dead_code)]

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

pub fn plain_file(dir: &Path, name: &str, content: &str) -> PathBuf {
    let p = dir.join(name);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).expect("create parent dir for plain_file");
    }
    fs::write(&p, content.as_bytes()).expect("write plain_file");
    p
}

pub fn nested_dir(dir: &Path, name: &str) -> PathBuf {
    let root = dir.join(name);
    let deep = root.join("level1").join("level2").join("level3");
    fs::create_dir_all(&deep).expect("create nested_dir");
    fs::write(root.join("top.txt"), b"top").expect("write top");
    fs::write(deep.join("bottom.txt"), b"bottom").expect("write bottom");
    root
}

pub fn empty_dir(dir: &Path, name: &str) -> PathBuf {
    let root = dir.join(name);
    fs::create_dir_all(&root).expect("create empty_dir");
    root
}

pub fn weird_names(dir: &Path) -> Vec<PathBuf> {
    let root = dir.join("weird");
    fs::create_dir_all(&root).expect("create weird dir");
    let names = [
        "中文文件.txt",
        "emoji_🚀_test.txt",
        "space in name.txt",
        "amp&parens(1).txt",
        "pct%bang!dollar$.txt",
        "semi;hash#.txt",
        "-leading-dash.txt",
    ];
    names
        .iter()
        .map(|n| {
            let p = root.join(n);
            fs::write(&p, format!("content of {}", n)).expect("write weird file");
            p
        })
        .collect()
}

pub fn reserved_names(dir: &Path) -> Vec<PathBuf> {
    // Returns candidate paths with Windows reserved device names (for rejection tests).
    // Does not rely on creating special \\?\CON files unless needed; callers can check path rejection directly.
    ["CON", "NUL", "AUX", "PRN", "COM1", "LPT1"]
        .iter()
        .map(|n| dir.join(n))
        .collect()
}

pub fn readonly_file(dir: &Path, name: &str) -> PathBuf {
    let p = plain_file(dir, name, "readonly content");
    let mut perms = fs::metadata(&p).expect("metadata").permissions();
    perms.set_readonly(true);
    fs::set_permissions(&p, perms).expect("set readonly");
    p
}

#[cfg(windows)]
pub fn hidden_system_file(dir: &Path, name: &str) -> PathBuf {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_SYSTEM,
    };

    let p = plain_file(dir, name, "hidden system content");
    let wide: Vec<u16> = p.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    unsafe {
        SetFileAttributesW(
            PCWSTR(wide.as_ptr()),
            FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM,
        )
        .expect("SetFileAttributesW hidden+system");
    }
    p
}

#[cfg(not(windows))]
pub fn hidden_system_file(dir: &Path, name: &str) -> PathBuf {
    plain_file(dir, name, "hidden system content")
}

/// Creates a directory under `dir` containing a nested path whose total length exceeds 260 characters.
/// Returns `(top_dir, leaf_file_extended_path)`.
pub fn long_path(dir: &Path, name: &str) -> (PathBuf, PathBuf) {
    let top = dir.join(name);
    let mut extended = PathBuf::from(format!(r"\\?\{}", top.display()));
    for i in 0..9 {
        extended.push(format!("seg_{:02}_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", i));
    }
    fs::create_dir_all(&extended).expect("create long_path dir");
    let leaf = extended.join("leaf_file.txt");
    fs::write(&leaf, b"long path payload").expect("write long_path leaf");
    assert!(
        leaf.to_string_lossy().trim_start_matches(r"\\?\").len() > 260,
        "long_path must exceed 260 characters"
    );
    (top, leaf)
}

#[cfg(windows)]
pub fn symlink_file(dir: &Path, link_name: &str, target: &Path) -> std::io::Result<PathBuf> {
    let link = dir.join(link_name);
    std::os::windows::fs::symlink_file(target, &link)?;
    Ok(link)
}

#[cfg(windows)]
pub fn symlink_dir(dir: &Path, link_name: &str, target: &Path) -> std::io::Result<PathBuf> {
    let link = dir.join(link_name);
    std::os::windows::fs::symlink_dir(target, &link)?;
    Ok(link)
}

#[cfg(windows)]
pub fn junction_dir(dir: &Path, link_name: &str, target: &Path) -> PathBuf {
    let link = dir.join(link_name);
    let out = Command::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            &link.to_string_lossy(),
            &target.to_string_lossy(),
        ])
        .output()
        .expect("run mklink /J");
    assert!(
        out.status.success(),
        "mklink /J failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    link
}

#[cfg(windows)]
pub fn dangling_link(dir: &Path, link_name: &str) -> std::io::Result<PathBuf> {
    let missing = dir.join("definitely_missing_target_99999.txt");
    let link = dir.join(link_name);
    std::os::windows::fs::symlink_file(&missing, &link)?;
    Ok(link)
}

pub fn many_small_files(dir: &Path, name: &str, count: usize) -> PathBuf {
    let root = dir.join(name);
    fs::create_dir_all(&root).expect("create many_small_files dir");
    for i in 0..count {
        let p = root.join(format!("f_{:04}.txt", i));
        fs::write(&p, format!("small file {}", i)).expect("write small file");
    }
    root
}

pub fn large_file(dir: &Path, name: &str, size_bytes: u64) -> PathBuf {
    let p = dir.join(name);
    let mut f = File::create(&p).expect("create large_file");
    let chunk = vec![b'A'; 64 * 1024];
    let mut remaining = size_bytes;
    while remaining > 0 {
        let to_write = (remaining as usize).min(chunk.len());
        f.write_all(&chunk[..to_write]).expect("write large_file");
        remaining -= to_write as u64;
    }
    p
}

pub fn deep_tree(dir: &Path, name: &str, depth: usize) -> PathBuf {
    let root = dir.join(name);
    let mut cur = PathBuf::from(format!(r"\\?\{}", root.display()));
    for i in 0..depth {
        cur.push(format!("d{}", i));
    }
    fs::create_dir_all(&cur).expect("create deep_tree");
    fs::write(cur.join("end.txt"), b"deep").expect("write deep end");
    root
}

/// Holds an exclusive lock on a file via a helper child process.
/// Automatically terminates the child process on Drop (R20 unattended requirement).
pub struct LockedFileGuard {
    pub path: PathBuf,
    child: Child,
}

impl Drop for LockedFileGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn locked_file(dir: &Path, name: &str) -> LockedFileGuard {
    let path = plain_file(dir, name, "locked payload");
    let escaped = path.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$fs = [System.IO.File]::Open('{}', [System.IO.FileMode]::Open, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None); \
         [Console]::Out.WriteLine('LOCKED'); \
         [Console]::Out.Flush(); \
         Start-Sleep -Seconds 60; \
         $fs.Close()",
        escaped
    );
    let mut child = Command::new("pwsh")
        .args(["-NoLogo", "-NoProfile", "-Command", &script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .or_else(|_| {
            Command::new("powershell")
                .args(["-NoLogo", "-NoProfile", "-Command", &script])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
        })
        .expect("spawn child locker process");

    let stdout = child.stdout.take().expect("locker stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).expect("read LOCKED marker");
    assert!(
        line.trim() == "LOCKED",
        "locker process failed to acquire lock, got: {:?}",
        line
    );

    LockedFileGuard { path, child }
}
