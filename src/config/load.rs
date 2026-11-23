use crate::model::error_code::ErrorCode;
use crate::platform;
use crate::rules::absolute::validate_and_normalize_path;
use crate::rules::blacklist::Blacklist;
use crate::rules::canonical::{is_root_key, is_same_key, is_strict_ancestor, os_path_to_compare_key};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

const SUPPORTED_SCHEMA: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    schema: u32,
    #[serde(default)]
    protect: RawProtect,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProtect {
    #[serde(rename = "self", default)]
    self_layer: RawPathList,
    #[serde(default)]
    tree: RawPathList,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPathList {
    #[serde(default)]
    paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ConfigError {
    pub code: ErrorCode,
    pub message: String,
}

/// Loads built-in hard protections (Section 6.2) combined with user config (Section 11).
/// Always reads fresh from disk on every invocation with zero caching (R19).
pub fn load_blacklist(cwd: &Path, explicit_config: Option<&Path>) -> Result<Blacklist, ConfigError> {
    let (builtin_self, builtin_tree) = builtin_hard_protections();

    let config_path: Option<PathBuf> = match explicit_config {
        Some(p) => {
            let full = if p.is_absolute() {
                p.to_path_buf()
            } else {
                cwd.join(p)
            };
            if !full.exists() {
                return Err(ConfigError {
                    code: ErrorCode::EConfigNotFound,
                    message: format!("指定的配置文件不存在：{}", full.display()),
                });
            }
            Some(full)
        }
        None => {
            let default_path = platform::default_config_dir().map(|d| d.join("config.toml"));
            match default_path {
                Some(p) if p.exists() => Some(p),
                _ => None,
            }
        }
    };

    let (user_self, user_tree) = match config_path {
        None => (Vec::new(), Vec::new()),
        Some(path) => parse_and_validate_config(&path)?,
    };

    Ok(Blacklist::new(builtin_self, builtin_tree, user_self, user_tree))
}

fn parse_and_validate_config(path: &Path) -> Result<(Vec<String>, Vec<String>), ConfigError> {
    let content = fs::read_to_string(path).map_err(|e| ConfigError {
        code: ErrorCode::EConfigParse,
        message: format!("无法读取配置文件 {}：{}", path.display(), e),
    })?;

    let raw: RawConfig = toml::from_str(&content).map_err(|e| ConfigError {
        code: ErrorCode::EConfigParse,
        message: format!("配置文件格式错误（{}）：{}", path.display(), e),
    })?;

    if raw.schema != SUPPORTED_SCHEMA {
        return Err(ConfigError {
            code: ErrorCode::EConfigValue,
            message: format!(
                "配置文件 schema 版本 {} 不受支持（当前仅支持 {}）",
                raw.schema, SUPPORTED_SCHEMA
            ),
        });
    }

    let mut self_keys = Vec::with_capacity(raw.protect.self_layer.paths.len());
    for p in &raw.protect.self_layer.paths {
        let norm = validate_and_normalize_path(p).map_err(|(_, msg)| ConfigError {
            code: ErrorCode::EConfigValue,
            message: format!("protect.self 路径非法 `{}`：{}", p, msg),
        })?;
        if norm.is_root || is_root_key(&norm.compare_key) {
            return Err(ConfigError {
                code: ErrorCode::EConfigValue,
                message: format!("配置文件不允许填入盘符根 `{}`（已由硬保护覆盖）", p),
            });
        }
        if self_keys.iter().any(|existing: &String| is_same_key(existing, &norm.compare_key)) {
            return Err(ConfigError {
                code: ErrorCode::EConfigValue,
                message: format!("protect.self 中存在重复路径：`{}`", p),
            });
        }
        self_keys.push(norm.compare_key);
    }

    let mut tree_keys = Vec::with_capacity(raw.protect.tree.paths.len());
    for p in &raw.protect.tree.paths {
        let norm = validate_and_normalize_path(p).map_err(|(_, msg)| ConfigError {
            code: ErrorCode::EConfigValue,
            message: format!("protect.tree 路径非法 `{}`：{}", p, msg),
        })?;
        if norm.is_root || is_root_key(&norm.compare_key) {
            return Err(ConfigError {
                code: ErrorCode::EConfigValue,
                message: format!("配置文件不允许填入盘符根 `{}`（已由硬保护覆盖）", p),
            });
        }
        if tree_keys.iter().any(|existing: &String| is_same_key(existing, &norm.compare_key)) {
            return Err(ConfigError {
                code: ErrorCode::EConfigValue,
                message: format!("protect.tree 中存在重复路径：`{}`", p),
            });
        }
        if self_keys.iter().any(|existing: &String| is_same_key(existing, &norm.compare_key)) {
            return Err(ConfigError {
                code: ErrorCode::EConfigValue,
                message: format!("protect.tree 与 protect.self 存在重复路径：`{}`", p),
            });
        }
        tree_keys.push(norm.compare_key);
    }

    // Section 11.3: Disallow parent-child overlap between `tree` and `self`
    for t in &tree_keys {
        for s in &self_keys {
            if is_strict_ancestor(t, s) || is_strict_ancestor(s, t) {
                return Err(ConfigError {
                    code: ErrorCode::EConfigValue,
                    message: format!(
                        "protect.tree (`{}`) 与 protect.self (`{}`) 存在父子关系重叠",
                        t, s
                    ),
                });
            }
        }
    }

    Ok((self_keys, tree_keys))
}

/// Section 6.2 Built-in hard protections (cannot be removed by config).
fn builtin_hard_protections() -> (Vec<String>, Vec<String>) {
    let builtin_self: Vec<String> = Vec::new();
    let mut builtin_tree: Vec<String> = vec![
        r"c:\windows".to_string(),
        r"c:\program files".to_string(),
        r"c:\program files (x86)".to_string(),
        r"c:\programdata".to_string(),
    ];

    // C:\Users\<current_user>\AppData (derived via Windows Known Folders, never env vars)
    if let Some(appdata_dir) = platform::user_appdata_root() {
        if let Some(key) = os_path_to_compare_key(&appdata_dir) {
            push_unique(&mut builtin_tree, key);
        }
    }

    // Default config directory %APPDATA%\eucli-recycle
    if let Some(cfg_dir) = platform::default_config_dir() {
        if let Some(key) = os_path_to_compare_key(&cfg_dir) {
            push_unique(&mut builtin_tree, key);
        }
    }

    // Executable file and its containing directory
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            if let Some(key) = os_path_to_compare_key(parent) {
                push_unique(&mut builtin_tree, key);
            }
        }
    }

    (builtin_self, builtin_tree)
}

fn push_unique(list: &mut Vec<String>, key: String) {
    if !list.iter().any(|k| is_same_key(k, &key)) {
        list.push(key);
    }
}
