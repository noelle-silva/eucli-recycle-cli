use crate::model::outcome::{EntryKind, ItemOutcome};
use crate::model::report::{ActionReport, ListReport};
use std::fmt::Write;

/// Section 9.2 Human-readable output rendered strictly from the unified internal report (R17).
pub fn render_action_human(report: &ActionReport) -> String {
    let mut out = String::new();

    if let Some(ref err) = report.error {
        let _ = writeln!(out, "[错误]       {}  {}", err.code, err.message);
        let _ = writeln!(out);
        let _ = writeln!(out, "退出码：{}", report.exit_code);
        return out;
    }

    let is_restore = report.command == "restore";

    for item in &report.items {
        let tag = match item.outcome {
            ItemOutcome::Succeeded => {
                if is_restore {
                    "[已恢复]    "
                } else {
                    "[已进回收站]"
                }
            }
            ItemOutcome::Planned => "[计划执行]  ",
            ItemOutcome::Blocked => "[被拒绝]    ",
            ItemOutcome::Failed => "[失败]      ",
            ItemOutcome::NotRun => "[未执行]    ",
            ItemOutcome::Unknown => "[无法确认]  ",
        };

        let subject = if is_restore {
            item.trash_id
                .as_deref()
                .or(item.input.as_deref())
                .unwrap_or("")
        } else {
            item.resolved
                .as_deref()
                .or(item.input.as_deref())
                .unwrap_or("")
        };

        match item.outcome {
            ItemOutcome::Succeeded => {
                if is_restore {
                    let orig = item.original_path.as_deref().unwrap_or("");
                    let _ = writeln!(out, "{} {}  -> {}", tag, subject, orig);
                } else {
                    let tid = item.trash_id.as_deref().unwrap_or("");
                    let _ = writeln!(out, "{} {}  条目标识 {}", tag, subject, tid);
                }
            }
            ItemOutcome::Planned => {
                let _ = writeln!(out, "{} {}  {}", tag, subject, item.message);
            }
            ItemOutcome::NotRun => {
                let _ = writeln!(out, "{} {}  {}", tag, subject, item.message);
            }
            ItemOutcome::Blocked | ItemOutcome::Failed | ItemOutcome::Unknown => {
                let _ = writeln!(out, "{} {}  {}  {}", tag, subject, item.code, item.message);
            }
        }
    }

    let _ = writeln!(out);
    let s = &report.summary;
    let mut parts = Vec::new();
    if s.planned > 0 {
        parts.push(format!("{} 计划执行", s.planned));
    }
    if s.succeeded > 0 {
        parts.push(format!("{} 成功", s.succeeded));
    }
    if s.blocked > 0 {
        parts.push(format!("{} 被拒绝", s.blocked));
    }
    if s.failed > 0 {
        parts.push(format!("{} 失败", s.failed));
    }
    if s.not_run > 0 {
        parts.push(format!("{} 未执行", s.not_run));
    }
    if s.unknown > 0 {
        parts.push(format!("{} 无法确认", s.unknown));
    }
    let _ = writeln!(out, "汇总：{} 个目标，{}", s.total, parts.join("，"));
    let _ = writeln!(out, "退出码：{}", report.exit_code);
    out
}

pub fn render_list_human(report: &ListReport) -> String {
    let mut out = String::new();

    if let Some(ref err) = report.error {
        let _ = writeln!(out, "[错误]       {}  {}", err.code, err.message);
        let _ = writeln!(out);
        let _ = writeln!(out, "退出码：{}", report.exit_code);
        return out;
    }

    for item in &report.items {
        let kind_label = match item.kind {
            EntryKind::File => "file",
            EntryKind::Directory => "dir ",
            EntryKind::Unknown => "unk ",
        };
        let _ = writeln!(
            out,
            "{}  {}  {}  {}",
            item.trash_id, kind_label, item.deleted_at, item.original_path
        );
    }

    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "汇总：共 {} 条，返回 {} 条",
        report.summary.total, report.summary.returned
    );
    let _ = writeln!(out, "退出码：{}", report.exit_code);
    out
}
