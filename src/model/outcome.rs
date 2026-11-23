use crate::model::error_code::ErrorCode;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemOutcome {
    Planned,
    Succeeded,
    Blocked,
    Failed,
    NotRun,
    Unknown,
}

impl ItemOutcome {
    pub const fn as_str(&self) -> &'static str {
        match self {
            ItemOutcome::Planned => "planned",
            ItemOutcome::Succeeded => "succeeded",
            ItemOutcome::Blocked => "blocked",
            ItemOutcome::Failed => "failed",
            ItemOutcome::NotRun => "not_run",
            ItemOutcome::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    File,
    Directory,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerificationInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub absent_at_source: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub present_in_bin: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub present_at_original: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub absent_in_bin: Option<bool>,
}

/// Per-target record for `put` and `restore`.
#[derive(Debug, Clone, Serialize)]
pub struct ItemRecord {
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<EntryKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_path: Option<String>,
    pub outcome: ItemOutcome,
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trash_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified: Option<VerificationInfo>,
}

/// Per-entry record for `list`.
#[derive(Debug, Clone, Serialize)]
pub struct ListItemRecord {
    pub trash_id: String,
    pub name: String,
    pub original_path: String,
    pub deleted_at: String,
    pub kind: EntryKind,
}
