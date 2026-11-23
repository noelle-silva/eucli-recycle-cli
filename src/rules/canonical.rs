use crate::rules::absolute::NormalizedPath;
use std::path::Path;

/// Normalizes a trusted OS path (such as current_dir or known folder path) into a lowercase
/// comparison key (`x:\foo\bar` or `x:\`).
pub fn os_path_to_compare_key(path: &Path) -> Option<String> {
    let raw = path.to_string_lossy();
    let stripped = raw.strip_prefix(r"\\?\").unwrap_or(&raw);
    let replaced = stripped.replace('/', "\\");
    let bytes = replaced.as_bytes();
    if bytes.len() < 3 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' || bytes[2] != b'\\' {
        return None;
    }
    let drive = (bytes[0] as char).to_ascii_lowercase();
    let segs: Vec<&str> = replaced[3..].split('\\').filter(|s| !s.is_empty()).collect();
    if segs.is_empty() {
        Some(format!("{}:\\", drive))
    } else {
        Some(format!("{}:\\{}", drive, segs.join("\\").to_ascii_lowercase()))
    }
}

pub fn is_same_key(a: &str, b: &str) -> bool {
    a == b
}

/// Returns true if `ancestor` is a strict parent/ancestor directory of `descendant`.
/// Both keys must be normalized lowercase Windows keys (`x:\` or `x:\a\b`).
pub fn is_strict_ancestor(ancestor: &str, descendant: &str) -> bool {
    if ancestor == descendant {
        return false;
    }
    if ancestor.ends_with('\\') {
        // Drive root `x:\` is an ancestor of any non-root path on the same drive.
        descendant.starts_with(ancestor)
    } else {
        descendant.len() > ancestor.len()
            && descendant.starts_with(ancestor)
            && descendant.as_bytes()[ancestor.len()] == b'\\'
    }
}

pub fn is_root_key(key: &str) -> bool {
    let b = key.as_bytes();
    b.len() == 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\'
}

pub fn parent_key(norm: &NormalizedPath) -> Option<String> {
    if norm.is_root {
        return None;
    }
    let idx = norm.compare_key.rfind('\\')?;
    if idx == 2 {
        Some(norm.compare_key[..3].to_string())
    } else {
        Some(norm.compare_key[..idx].to_string())
    }
}
