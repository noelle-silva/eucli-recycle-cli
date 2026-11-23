use crate::model::error_code::ErrorCode;
use crate::model::outcome::{EntryKind, ItemOutcome, ItemRecord};
use crate::platform::{self, ResolvedTarget, TargetProbeResult};
use crate::rules::absolute::{validate_and_normalize_path, NormalizedPath};
use crate::rules::blacklist::Blacklist;
use crate::rules::canonical::{is_same_key, is_strict_ancestor};
use crate::rules::current_dir::check_current_dir;

pub enum GateOutcome {
    /// All targets passed every rule; ready for execution (or dry-run reporting).
    Passed(Vec<ResolvedTarget>),
    /// At least one target failed a rule; nothing may execute (R12).
    Blocked(Vec<ItemRecord>),
}

/// Runs the complete Section 5–6 check pipeline across all `put` targets (R6–R12, R22).
pub fn evaluate_put_targets(
    raw_inputs: &[String],
    dir_flag: bool,
    cwd_key: &str,
    blacklist: &Blacklist,
) -> GateOutcome {
    let n = raw_inputs.len();
    let mut normalized_opt: Vec<Option<NormalizedPath>> = vec![None; n];
    let mut errors: Vec<Option<(ErrorCode, String)>> = vec![None; n];
    let mut kinds: Vec<Option<EntryKind>> = vec![None; n];

    // Step 1: Section 5 absolute path & ambiguity validation
    for (idx, raw) in raw_inputs.iter().enumerate() {
        match validate_and_normalize_path(raw) {
            Ok(norm) => normalized_opt[idx] = Some(norm),
            Err(err) => errors[idx] = Some(err),
        }
    }

    // Step 2: Section 6.4 / R22 batch relations (duplicates and parent-child overlap)
    for i in 0..n {
        let Some(ref a) = normalized_opt[i] else {
            continue;
        };
        for j in (i + 1)..n {
            let Some(ref b) = normalized_opt[j] else {
                continue;
            };
            if is_same_key(&a.compare_key, &b.compare_key) {
                if errors[j].is_none() {
                    errors[j] = Some((
                        ErrorCode::EDuplicateTarget,
                        format!("与第 {} 个目标路径重复（大小写/分隔符规范化后相同）", i),
                    ));
                }
            } else if is_strict_ancestor(&a.compare_key, &b.compare_key) {
                if errors[j].is_none() {
                    errors[j] = Some((
                        ErrorCode::EOverlappingTarget,
                        format!("与第 {} 个目标存在上下级包含关系", i),
                    ));
                }
            } else if is_strict_ancestor(&b.compare_key, &a.compare_key) {
                if errors[i].is_none() {
                    errors[i] = Some((
                        ErrorCode::EOverlappingTarget,
                        format!("与第 {} 个目标存在上下级包含关系", j),
                    ));
                }
            }
        }
    }

    // Step 3: Blacklist, CWD, and filesystem probe (non-following) + `--dir` rule (R10, R11)
    let mut resolved_targets: Vec<Option<ResolvedTarget>> = vec![None; n];

    for i in 0..n {
        if errors[i].is_some() {
            continue;
        }
        let norm = normalized_opt[i].as_ref().unwrap();

        if let Err(err) = blacklist.check(norm) {
            errors[i] = Some(err);
            continue;
        }

        if let Err(err) = check_current_dir(norm, cwd_key) {
            errors[i] = Some(err);
            continue;
        }

        // Probe actual object on disk (non-following, captures identity snapshot for R8)
        match platform::probe_target(&norm.normalized) {
            TargetProbeResult::Found {
                kind,
                is_reparse_point,
                snapshot,
            } => {
                // Section 6.5: Reparse points (file symlink, dir symlink, junction, mount point)
                // are treated as file-like links and do NOT require --dir.
                let effective_kind = if is_reparse_point {
                    EntryKind::File
                } else {
                    kind
                };
                kinds[i] = Some(effective_kind);

                match (effective_kind, dir_flag) {
                    (EntryKind::Directory, false) => {
                        errors[i] = Some((
                            ErrorCode::ENeedsDirFlag,
                            "目标是目录，必须显式声明 --dir 才能送进回收站".to_string(),
                        ));
                    }
                    (EntryKind::File, true) => {
                        errors[i] = Some((
                            ErrorCode::EDirFlagOnFile,
                            "声明了 --dir，但实际目标是文件或链接（非普通目录）".to_string(),
                        ));
                    }
                    _ => {
                        resolved_targets[i] = Some(ResolvedTarget {
                            index: i,
                            input: raw_inputs[i].clone(),
                            normalized: norm.normalized.clone(),
                            compare_key: norm.compare_key.clone(),
                            kind: effective_kind,
                            is_reparse_point,
                            snapshot,
                        });
                    }
                }
            }
            TargetProbeResult::NotFound => {
                errors[i] = Some((
                    ErrorCode::ENotFound,
                    format!("目标不存在：{}", norm.normalized),
                ));
            }
            TargetProbeResult::AccessDenied(detail) => {
                errors[i] = Some((
                    ErrorCode::EAccessDenied,
                    format!("没有权限访问目标：{}", detail),
                ));
            }
            TargetProbeResult::SystemError(detail) => {
                errors[i] = Some((
                    ErrorCode::EUnknownSystem,
                    format!("无法检查目标状态：{}", detail),
                ));
            }
        }
    }

    let any_blocked = errors.iter().any(|e| e.is_some());
    if any_blocked {
        let mut records = Vec::with_capacity(n);
        for i in 0..n {
            let resolved = normalized_opt[i].as_ref().map(|p| p.normalized.clone());
            if let Some((code, msg)) = errors[i].take() {
                records.push(ItemRecord {
                    index: i,
                    input: Some(raw_inputs[i].clone()),
                    resolved,
                    kind: kinds[i],
                    original_path: None,
                    outcome: ItemOutcome::Blocked,
                    code,
                    message: msg,
                    trash_id: None,
                    verified: None,
                });
            } else {
                records.push(ItemRecord {
                    index: i,
                    input: Some(raw_inputs[i].clone()),
                    resolved,
                    kind: kinds[i],
                    original_path: None,
                    outcome: ItemOutcome::NotRun,
                    code: ErrorCode::EStopped,
                    message: "批内存在被拒绝的目标，按全查全过规则未执行".to_string(),
                    trash_id: None,
                    verified: None,
                });
            }
        }
        GateOutcome::Blocked(records)
    } else {
        let passed = resolved_targets.into_iter().map(|t| t.unwrap()).collect();
        GateOutcome::Passed(passed)
    }
}
