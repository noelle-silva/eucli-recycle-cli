pub mod cli;
pub mod config;
pub mod model;
pub mod output;
pub mod platform;
pub mod rules;

use crate::cli::parse::{ListArgs, OutputFormat, PutArgs, RestoreArgs};
use crate::config::load::load_blacklist;
use crate::model::error_code::ErrorCode;
use crate::model::outcome::{ItemOutcome, ItemRecord, ListItemRecord};
use crate::model::report::{ActionReport, ListReport};
use crate::platform::{ListFilter, RecycleBin, TrashEntry};
use crate::rules::absolute::validate_and_normalize_path;
use crate::rules::canonical::{is_same_key, os_path_to_compare_key};
use crate::rules::gate::{evaluate_put_targets, GateOutcome};
use std::path::{Path, PathBuf};

pub fn execute_put(
    cwd: &Path,
    args: &PutArgs,
    pre_exec_hook: Option<Box<dyn FnOnce()>>,
) -> ActionReport {
    let started_at = ActionReport::now_iso8601();
    let bin = match platform::default_recycle_bin() {
        Ok(b) => b,
        Err(e) => {
            return ActionReport::from_top_error("put", started_at, e.code, e.message, 1);
        }
    };
    execute_put_inner(cwd, args, started_at, &bin, pre_exec_hook)
}

/// Test/integration seam allowing injection of a custom `RecycleBin` implementation (e.g. R15 verification tests).
pub fn run_put_with_bin(
    cwd: &Path,
    targets: &[String],
    dir: bool,
    dry_run: bool,
    config: Option<PathBuf>,
    bin: &dyn RecycleBin,
) -> ActionReport {
    let started_at = ActionReport::now_iso8601();
    let args = PutArgs {
        dir,
        dry_run,
        format: OutputFormat::Json,
        config,
        targets: targets.to_vec(),
    };
    execute_put_inner(cwd, &args, started_at, bin, None)
}

/// Test/integration seam allowing a hook between check gate and execution (R8 TOCTOU tests).
pub fn run_put_with_pre_exec_hook(
    cwd: &Path,
    targets: &[String],
    dir: bool,
    dry_run: bool,
    config: Option<PathBuf>,
    hook: Option<Box<dyn FnOnce()>>,
) -> ActionReport {
    let args = PutArgs {
        dir,
        dry_run,
        format: OutputFormat::Json,
        config,
        targets: targets.to_vec(),
    };
    execute_put(cwd, &args, hook)
}

fn execute_put_inner(
    cwd: &Path,
    args: &PutArgs,
    started_at: String,
    bin: &dyn RecycleBin,
    pre_exec_hook: Option<Box<dyn FnOnce()>>,
) -> ActionReport {
    // R19: Freshly load and validate config on every call
    let blacklist = match load_blacklist(cwd, args.config.as_deref()) {
        Ok(bl) => bl,
        Err(err) => {
            return ActionReport::from_top_error("put", started_at, err.code, err.message, 1);
        }
    };

    let cwd_key = match os_path_to_compare_key(cwd) {
        Some(k) => k,
        None => {
            return ActionReport::from_top_error(
                "put",
                started_at,
                ErrorCode::EUnknownSystem,
                format!("无法规范化当前工作目录：{}", cwd.display()),
                1,
            );
        }
    };

    match evaluate_put_targets(&args.targets, args.dir, &cwd_key, &blacklist) {
        GateOutcome::Blocked(items) => ActionReport::from_items("put", started_at, items),
        GateOutcome::Passed(resolved_targets) => {
            if args.dry_run {
                let items = resolved_targets
                    .into_iter()
                    .map(|t| ItemRecord {
                        index: t.index,
                        input: Some(t.input),
                        resolved: Some(t.normalized),
                        kind: Some(t.kind),
                        original_path: None,
                        outcome: ItemOutcome::Planned,
                        code: ErrorCode::EOk,
                        message: "检查通过（dry-run，未执行）".to_string(),
                        trash_id: None,
                        verified: None,
                    })
                    .collect();
                return ActionReport::from_items("put", started_at, items);
            }

            if let Some(hook) = pre_exec_hook {
                hook();
            }

            let outcomes = bin.recycle(&resolved_targets);
            let items = resolved_targets
                .into_iter()
                .zip(outcomes)
                .map(|(t, o)| ItemRecord {
                    index: t.index,
                    input: Some(t.input),
                    resolved: Some(t.normalized),
                    kind: Some(t.kind),
                    original_path: None,
                    outcome: o.outcome,
                    code: o.code,
                    message: o.message,
                    trash_id: o.trash_id,
                    verified: o.verified,
                })
                .collect();
            ActionReport::from_items("put", started_at, items)
        }
    }
}

pub fn execute_list(args: &ListArgs) -> ListReport {
    let started_at = ActionReport::now_iso8601();
    let bin = match platform::default_recycle_bin() {
        Ok(b) => b,
        Err(e) => {
            return ListReport::from_error(started_at, e.code, e.message, 1);
        }
    };

    let from_key = args
        .from
        .as_deref()
        .and_then(|s| validate_and_normalize_path(s).ok())
        .map(|n| n.compare_key);

    let filter = ListFilter {
        from_key,
        name_substr: args.name.clone(),
        after: args.after,
        before: args.before,
    };

    let entries = match bin.enumerate(&filter) {
        Ok(list) => list,
        Err(e) => {
            return ListReport::from_error(started_at, e.code, e.message, 3);
        }
    };

    let total = entries.len();
    let iter = entries.into_iter().skip(args.offset);
    let sliced: Vec<TrashEntry> = match args.limit {
        Some(lim) => iter.take(lim).collect(),
        None => iter.collect(),
    };

    let items = sliced
        .into_iter()
        .map(|e| ListItemRecord {
            trash_id: e.trash_id,
            name: e.name,
            original_path: e.original_path,
            deleted_at: e.deleted_at,
            kind: e.kind,
        })
        .collect();

    ListReport::from_entries(started_at, total, items)
}

pub fn execute_restore(cwd: &Path, args: &RestoreArgs) -> ActionReport {
    let started_at = ActionReport::now_iso8601();
    let bin = match platform::default_recycle_bin() {
        Ok(b) => b,
        Err(e) => {
            return ActionReport::from_top_error("restore", started_at, e.code, e.message, 1);
        }
    };

    let blacklist = match load_blacklist(cwd, args.config.as_deref()) {
        Ok(bl) => bl,
        Err(err) => {
            return ActionReport::from_top_error("restore", started_at, err.code, err.message, 1);
        }
    };

    let n = args.trash_ids.len();
    let lower_ids: Vec<String> = args
        .trash_ids
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect();
    let mut errors: Vec<Option<(ErrorCode, String)>> = vec![None; n];
    let mut matched: Vec<Option<TrashEntry>> = vec![None; n];

    // Section 6.4: Duplicate trash_id check before touching anything
    for i in 0..n {
        for j in (i + 1)..n {
            if lower_ids[i] == lower_ids[j] && errors[j].is_none() {
                errors[j] = Some((
                    ErrorCode::EDuplicateTrashId,
                    format!("与第 {} 个条目标识重复：{}", i, args.trash_ids[j]),
                ));
            }
        }
    }

    let all_entries = if errors.iter().any(|e| e.is_some()) {
        Vec::new()
    } else {
        match bin.enumerate(&ListFilter::default()) {
            Ok(list) => list,
            Err(e) => {
                return ActionReport::from_top_error("restore", started_at, e.code, e.message, 3);
            }
        }
    };

    if errors.iter().all(|e| e.is_none()) {
        let mut planned_dest_keys: Vec<String> = Vec::new();

        for i in 0..n {
            let hits: Vec<&TrashEntry> = all_entries
                .iter()
                .filter(|e| e.trash_id == lower_ids[i])
                .collect();

            if hits.is_empty() {
                errors[i] = Some((
                    ErrorCode::ETrashIdNotFound,
                    format!("回收站中不存在该条目标识：{}", args.trash_ids[i]),
                ));
                continue;
            }
            if hits.len() > 1 {
                errors[i] = Some((
                    ErrorCode::ETrashIdAmbiguous,
                    format!(
                        "条目标识 {} 命中 {} 条记录，标识不唯一，请用 list 重新获取",
                        args.trash_ids[i],
                        hits.len()
                    ),
                ));
                continue;
            }

            let entry = hits[0].clone();

            // Section 8: Check if original path is protected by blacklist
            if let Ok(norm) = validate_and_normalize_path(&entry.original_path) {
                if let Err(bl_err) = blacklist.check(&norm) {
                    errors[i] = Some(bl_err);
                    matched[i] = Some(entry);
                    continue;
                }
            }

            // Section 8: Check original parent dir exists
            if !platform::parent_dir_exists(&entry.original_path) {
                errors[i] = Some((
                    ErrorCode::EOriginDirMissing,
                    format!(
                        "原父目录已不存在（{}），请先手动建好目录再恢复",
                        entry.original_parent
                    ),
                ));
                matched[i] = Some(entry);
                continue;
            }

            // Section 8: Check original location not occupied (never overwrite)
            match platform::path_exists_no_follow(&entry.original_path) {
                Ok(true) => {
                    errors[i] = Some((
                        ErrorCode::EOriginOccupied,
                        format!("原位置已有同名对象（{}），绝不覆盖", entry.original_path),
                    ));
                    matched[i] = Some(entry);
                    continue;
                }
                Err(win_err) => {
                    errors[i] = Some((
                        ErrorCode::EAccessDenied,
                        format!("无法检查原位置状态（Win32 错误码 {}）", win_err),
                    ));
                    matched[i] = Some(entry);
                    continue;
                }
                Ok(false) => {}
            }

            // Prevent two entries in same restore batch colliding on the same original path
            if planned_dest_keys
                .iter()
                .any(|k| is_same_key(k, &entry.original_path_key))
            {
                errors[i] = Some((
                    ErrorCode::EOriginOccupied,
                    format!("与批内其他条目恢复到同一原路径：{}", entry.original_path),
                ));
                matched[i] = Some(entry);
                continue;
            }

            planned_dest_keys.push(entry.original_path_key.clone());
            matched[i] = Some(entry);
        }
    }

    if errors.iter().any(|e| e.is_some()) {
        let mut records = Vec::with_capacity(n);
        for i in 0..n {
            let orig = matched[i].as_ref().map(|e| e.original_path.clone());
            if let Some((code, msg)) = errors[i].take() {
                records.push(ItemRecord {
                    index: i,
                    input: None,
                    resolved: None,
                    kind: matched[i].as_ref().map(|e| e.kind),
                    original_path: orig,
                    outcome: ItemOutcome::Blocked,
                    code,
                    message: msg,
                    trash_id: Some(args.trash_ids[i].clone()),
                    verified: None,
                });
            } else {
                records.push(ItemRecord {
                    index: i,
                    input: None,
                    resolved: None,
                    kind: matched[i].as_ref().map(|e| e.kind),
                    original_path: orig,
                    outcome: ItemOutcome::NotRun,
                    code: ErrorCode::EStopped,
                    message: "批内存在被拒绝的条目，未执行恢复".to_string(),
                    trash_id: Some(args.trash_ids[i].clone()),
                    verified: None,
                });
            }
        }
        return ActionReport::from_items("restore", started_at, records);
    }

    let entries_to_restore: Vec<TrashEntry> = matched.into_iter().map(|e| e.unwrap()).collect();
    let outcomes = bin.restore(&entries_to_restore);
    let records = entries_to_restore
        .into_iter()
        .enumerate()
        .zip(outcomes)
        .map(|((idx, entry), o)| ItemRecord {
            index: idx,
            input: None,
            resolved: None,
            kind: Some(entry.kind),
            original_path: Some(entry.original_path),
            outcome: o.outcome,
            code: o.code,
            message: o.message,
            trash_id: Some(entry.trash_id),
            verified: o.verified,
        })
        .collect();

    ActionReport::from_items("restore", started_at, records)
}
