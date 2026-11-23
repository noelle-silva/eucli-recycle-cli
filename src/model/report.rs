use crate::model::error_code::ErrorCode;
use crate::model::outcome::{ItemOutcome, ItemRecord, ListItemRecord};
use chrono::Local;
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct ReportSummary {
    pub total: usize,
    pub succeeded: usize,
    pub blocked: usize,
    pub failed: usize,
    pub not_run: usize,
    pub unknown: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub planned: usize,
}

fn is_zero(v: &usize) -> bool {
    *v == 0
}

#[derive(Debug, Clone, Serialize)]
pub struct TopLevelError {
    pub code: ErrorCode,
    pub message: String,
}

/// Unified report for `put` and `restore` (and top-level parse/config errors).
/// Single source of truth for both human and JSON output (R17).
#[derive(Debug, Clone, Serialize)]
pub struct ActionReport {
    pub schema: u32,
    pub command: String,
    pub started_at: String,
    pub finished_at: String,
    pub exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<TopLevelError>,
    pub summary: ReportSummary,
    pub items: Vec<ItemRecord>,
}

impl ActionReport {
    pub fn now_iso8601() -> String {
        Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
    }

    /// Computes summary and exit code exclusively from `items` (Section 10.2, R16, R17).
    pub fn from_items(command: &str, started_at: String, items: Vec<ItemRecord>) -> Self {
        let mut summary = ReportSummary {
            total: items.len(),
            ..Default::default()
        };
        for item in &items {
            match item.outcome {
                ItemOutcome::Planned => summary.planned += 1,
                ItemOutcome::Succeeded => summary.succeeded += 1,
                ItemOutcome::Blocked => summary.blocked += 1,
                ItemOutcome::Failed => summary.failed += 1,
                ItemOutcome::NotRun => summary.not_run += 1,
                ItemOutcome::Unknown => summary.unknown += 1,
            }
        }

        let exit_code = compute_exit_code(&summary);
        Self {
            schema: 1,
            command: command.to_string(),
            started_at,
            finished_at: Self::now_iso8601(),
            exit_code,
            error: None,
            summary,
            items,
        }
    }

    /// Top-level argument/config/platform error before per-item evaluation.
    pub fn from_top_error(command: &str, started_at: String, code: ErrorCode, message: String, exit_code: i32) -> Self {
        Self {
            schema: 1,
            command: command.to_string(),
            started_at,
            finished_at: Self::now_iso8601(),
            exit_code,
            error: Some(TopLevelError { code, message }),
            summary: ReportSummary::default(),
            items: Vec::new(),
        }
    }
}

/// Section 10.2 Exit Code rules derived purely from summary counts.
pub fn compute_exit_code(summary: &ReportSummary) -> i32 {
    if summary.failed > 0 {
        3
    } else if summary.unknown > 0 {
        4
    } else if summary.blocked > 0 {
        2
    } else {
        0
    }
}

/// Unified report for `list`.
#[derive(Debug, Clone, Serialize)]
pub struct ListReport {
    pub schema: u32,
    pub command: String,
    pub started_at: String,
    pub finished_at: String,
    pub exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<TopLevelError>,
    pub summary: ListSummary,
    pub items: Vec<ListItemRecord>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ListSummary {
    pub total: usize,
    pub returned: usize,
}

impl ListReport {
    pub fn from_entries(started_at: String, total: usize, items: Vec<ListItemRecord>) -> Self {
        let returned = items.len();
        Self {
            schema: 1,
            command: "list".to_string(),
            started_at,
            finished_at: ActionReport::now_iso8601(),
            exit_code: 0,
            error: None,
            summary: ListSummary { total, returned },
            items,
        }
    }

    pub fn from_error(started_at: String, code: ErrorCode, message: String, exit_code: i32) -> Self {
        Self {
            schema: 1,
            command: "list".to_string(),
            started_at,
            finished_at: ActionReport::now_iso8601(),
            exit_code,
            error: Some(TopLevelError { code, message }),
            summary: ListSummary::default(),
            items: Vec::new(),
        }
    }
}
