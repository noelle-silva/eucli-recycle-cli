use crate::model::error_code::ErrorCode;
use crate::rules::absolute::NormalizedPath;
use crate::rules::canonical::{is_same_key, is_strict_ancestor};

/// Two-tier protection policy (Section 6.1–6.3). Pure path logic, zero filesystem I/O.
#[derive(Debug, Clone)]
pub struct Blacklist {
    /// Protected itself only; children inside are allowed.
    pub self_keys: Vec<String>,
    /// Protected itself AND entire subtree underneath.
    pub tree_keys: Vec<String>,
}

impl Blacklist {
    pub fn new(
        builtin_self: Vec<String>,
        builtin_tree: Vec<String>,
        user_self: Vec<String>,
        user_tree: Vec<String>,
    ) -> Self {
        let mut self_keys = builtin_self;
        self_keys.extend(user_self);
        let mut tree_keys = builtin_tree;
        tree_keys.extend(user_tree);
        Self {
            self_keys,
            tree_keys,
        }
    }

    /// Checks a normalized target against drive-root, volume recycle-bin dir, built-in and user rules.
    pub fn check(&self, target: &NormalizedPath) -> Result<(), (ErrorCode, String)> {
        // 1. All drive roots X:\ are hard-protected (Section 6.2)
        if target.is_root {
            return Err((
                ErrorCode::EDriveRoot,
                format!("禁止操作盘符根目录：{}", target.normalized),
            ));
        }

        let key = target.compare_key.as_str();

        // 2. Per-volume system recycle bin directory (protect tree, Section 6.2)
        let vol_bin_key = format!(
            "{}:\\{}",
            target.drive_letter.to_ascii_lowercase(),
            "\x24recycle.bin"
        );
        if is_same_key(key, &vol_bin_key) {
            return Err((
                ErrorCode::EBlacklistTree,
                format!("目标命中内置整片保护（卷回收站目录）：{}", target.normalized),
            ));
        }
        if is_strict_ancestor(&vol_bin_key, key) {
            return Err((
                ErrorCode::EProtectedParent,
                format!("目标落在内置整片保护范围内：{}", target.normalized),
            ));
        }

        // 3. Exact match on `protect.tree`
        for tree in &self.tree_keys {
            if is_same_key(key, tree) {
                return Err((
                    ErrorCode::EBlacklistTree,
                    format!("目标命中「保护整片」规则：{}", target.normalized),
                ));
            }
        }

        // 4. Exact match on `protect.self`
        for s in &self.self_keys {
            if is_same_key(key, s) {
                return Err((
                    ErrorCode::EBlacklistSelf,
                    format!("目标命中「保护自身」规则：{}", target.normalized),
                ));
            }
        }

        // 5. Containment rule (Section 6.3.1):
        //    - Target falls inside any `protect.tree` -> E_PROTECTED_PARENT
        //    - Target is an ancestor containing any `protect.tree` or `protect.self` -> E_PROTECTED_PARENT
        for tree in &self.tree_keys {
            if is_strict_ancestor(tree, key) {
                return Err((
                    ErrorCode::EProtectedParent,
                    format!("目标落在受保护目录范围内（保护规则：{}）", tree),
                ));
            }
            if is_strict_ancestor(key, tree) {
                return Err((
                    ErrorCode::EProtectedParent,
                    format!("目标包含受保护对象（{}），拒绝删除上级目录", tree),
                ));
            }
        }

        for s in &self.self_keys {
            if is_strict_ancestor(key, s) {
                return Err((
                    ErrorCode::EProtectedParent,
                    format!("目标包含受保护对象（{}），拒绝删除上级目录", s),
                ));
            }
        }

        Ok(())
    }
}
