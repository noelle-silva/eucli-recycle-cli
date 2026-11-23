use crate::model::error_code::ErrorCode;
use crate::model::outcome::{EntryKind, ItemOutcome, VerificationInfo};
use chrono::{DateTime, Local};
use std::path::PathBuf;

#[cfg(windows)]
pub mod windows;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetSnapshot {
    pub attributes: u32,
    pub is_reparse_point: bool,
    pub is_directory: bool,
    pub volume_serial: Option<u32>,
    pub file_index: Option<u64>,
    pub creation_time: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ResolvedTarget {
    pub index: usize,
    pub input: String,
    pub normalized: String,
    pub compare_key: String,
    pub kind: EntryKind,
    pub is_reparse_point: bool,
    pub snapshot: TargetSnapshot,
}

#[derive(Debug, Clone)]
pub enum TargetProbeResult {
    Found {
        kind: EntryKind,
        is_reparse_point: bool,
        snapshot: TargetSnapshot,
    },
    NotFound,
    AccessDenied(String),
    SystemError(String),
}

#[derive(Debug, Clone)]
pub struct RecycleItemOutcome {
    pub outcome: ItemOutcome,
    pub code: ErrorCode,
    pub message: String,
    pub trash_id: Option<String>,
    pub verified: Option<VerificationInfo>,
}

#[derive(Debug, Clone, Default)]
pub struct ListFilter {
    pub from_key: Option<String>,
    pub name_substr: Option<String>,
    pub after: Option<DateTime<Local>>,
    pub before: Option<DateTime<Local>>,
}

#[derive(Debug, Clone)]
pub struct TrashEntry {
    pub trash_id: String,
    pub name: String,
    pub original_parent: String,
    pub original_path: String,
    pub original_path_key: String,
    pub deleted_at: String,
    pub deleted_at_dt: Option<DateTime<Local>>,
    pub deleted_at_filetime: u64,
    pub size: u64,
    pub kind: EntryKind,
}

#[derive(Debug, Clone)]
pub struct PlatformError {
    pub code: ErrorCode,
    pub message: String,
}

/// Explicit error for platforms not supported in v1 (Section 15.2).
pub fn unsupported_platform_error(os: &str) -> PlatformError {
    PlatformError {
        code: ErrorCode::EUnknownSystem,
        message: format!("本平台尚未支持（当前仅支持 Windows 10/11 x64，检测到：{}）", os),
    }
}

/// Section 12.1 Platform abstraction interface — only 3 actions exist (R1, R3).
pub trait RecycleBin {
    fn recycle(&self, targets: &[ResolvedTarget]) -> Vec<RecycleItemOutcome>;
    fn enumerate(&self, filter: &ListFilter) -> Result<Vec<TrashEntry>, PlatformError>;
    fn restore(&self, entries: &[TrashEntry]) -> Vec<RecycleItemOutcome>;
}

#[cfg(windows)]
pub fn default_recycle_bin() -> Result<windows::WindowsRecycleBin, PlatformError> {
    Ok(windows::WindowsRecycleBin::new())
}

#[cfg(not(windows))]
pub fn default_recycle_bin() -> Result<UnsupportedRecycleBin, PlatformError> {
    Err(unsupported_platform_error(std::env::consts::OS))
}

#[cfg(not(windows))]
pub struct UnsupportedRecycleBin;

#[cfg(not(windows))]
impl RecycleBin for UnsupportedRecycleBin {
    fn recycle(&self, _targets: &[ResolvedTarget]) -> Vec<RecycleItemOutcome> {
        Vec::new()
    }
    fn enumerate(&self, _filter: &ListFilter) -> Result<Vec<TrashEntry>, PlatformError> {
        Err(unsupported_platform_error(std::env::consts::OS))
    }
    fn restore(&self, _entries: &[TrashEntry]) -> Vec<RecycleItemOutcome> {
        Vec::new()
    }
}

#[cfg(windows)]
pub fn probe_target(path: &str) -> TargetProbeResult {
    windows::volume::probe_target(path)
}

#[cfg(not(windows))]
pub fn probe_target(_path: &str) -> TargetProbeResult {
    TargetProbeResult::SystemError(unsupported_platform_error(std::env::consts::OS).message)
}

#[cfg(windows)]
pub fn path_exists_no_follow(path: &str) -> Result<bool, u32> {
    windows::volume::path_exists_no_follow(path)
}

#[cfg(not(windows))]
pub fn path_exists_no_follow(_path: &str) -> Result<bool, u32> {
    Ok(false)
}

#[cfg(windows)]
pub fn parent_dir_exists(path: &str) -> bool {
    windows::volume::parent_dir_exists(path)
}

#[cfg(not(windows))]
pub fn parent_dir_exists(_path: &str) -> bool {
    false
}

#[cfg(windows)]
pub fn user_appdata_root() -> Option<PathBuf> {
    windows::volume::user_appdata_root()
}

#[cfg(not(windows))]
pub fn user_appdata_root() -> Option<PathBuf> {
    None
}

#[cfg(windows)]
pub fn default_config_dir() -> Option<PathBuf> {
    windows::volume::default_config_dir()
}

#[cfg(not(windows))]
pub fn default_config_dir() -> Option<PathBuf> {
    None
}
