#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Isolated sandbox directory for a single test.
/// Enforces Section 13.3 path guard: every executable target MUST lie strictly inside the sandbox.
pub struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    pub fn new(label: &str) -> Self {
        let base = std::env::var("EUCLI_RECYCLE_TEST_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("eucli-recycle-test-sandboxes"));

        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let dir_name = format!("{}_{}_{}_{}", sanitize(label), pid, ts, n);
        let root = base.join(dir_name);
        fs::create_dir_all(&root).expect("create sandbox root");

        // Ensure canonical-ish normalized absolute Windows path without \\?\
        let abs = fs::canonicalize(&root).unwrap_or(root);
        let cleaned = strip_verbatim(&abs);
        Self { root: cleaned }
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn work_dir(&self) -> PathBuf {
        let wd = self.root.join("_cwd");
        fs::create_dir_all(&wd).expect("create sandbox _cwd");
        wd
    }

    pub fn data_dir(&self) -> PathBuf {
        let dd = self.root.join("data");
        fs::create_dir_all(&dd).expect("create sandbox data");
        dd
    }

    /// Panics immediately if `target` is NOT strictly beneath this sandbox root (Section 13.3 rule 2).
    pub fn assert_inside(&self, target: &Path) {
        let root_norm = normalize_cmp(&self.root);
        let target_norm = normalize_cmp(target);
        let prefix = format!("{}\\", root_norm);
        assert!(
            target_norm.starts_with(&prefix),
            "SAFETY GUARD VIOLATION: executable target `{}` is NOT inside sandbox `{}`",
            target.display(),
            self.root.display()
        );
    }

    pub fn checked_arg(&self, target: &Path) -> String {
        self.assert_inside(target);
        target.to_string_lossy().to_string()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        // Only remove leftover files inside our own temporary sandbox; NEVER touch the Recycle Bin.
        let ext = format!(r"\\?\{}", self.root.display());
        clear_readonly_recursive(Path::new(&ext));
        let _ = fs::remove_dir_all(&ext);
    }
}

fn clear_readonly_recursive(path: &Path) {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return;
        }
        if meta.permissions().readonly() {
            let mut perms = meta.permissions();
            perms.set_readonly(false);
            let _ = fs::set_permissions(path, perms);
        }
        if meta.is_dir() {
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    clear_readonly_recursive(&entry.path());
                }
            }
        }
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect()
}

fn strip_verbatim(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        p.to_path_buf()
    }
}

fn normalize_cmp(p: &Path) -> String {
    let s = p.to_string_lossy().replace('/', "\\");
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
    s.trim_end_matches('\\').to_ascii_lowercase()
}
