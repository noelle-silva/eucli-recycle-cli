use crate::model::error_code::ErrorCode;
use crate::rules::absolute::NormalizedPath;
use crate::rules::canonical::{is_same_key, is_strict_ancestor};

/// Section 6.3.2 & R9: Current working directory and all its ancestors are unconditionally rejected.
pub fn check_current_dir(target: &NormalizedPath, cwd_key: &str) -> Result<(), (ErrorCode, String)> {
    let key = target.compare_key.as_str();
    if is_same_key(key, cwd_key) {
        return Err((
            ErrorCode::ECwdItself,
            format!("禁止操作当前工作目录本身：{}", target.normalized),
        ));
    }
    if is_strict_ancestor(key, cwd_key) {
        return Err((
            ErrorCode::ECwdAncestor,
            format!("禁止操作当前工作目录的上级目录：{}", target.normalized),
        ));
    }
    Ok(())
}
