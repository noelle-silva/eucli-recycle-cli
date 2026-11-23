use crate::model::error_code::ErrorCode;
use crate::model::outcome::{ItemOutcome, VerificationInfo};
use crate::platform::windows::enumerate::{
    find_shell_items_by_trash_id, is_trash_id_present_in_bin, verify_newly_created_item,
};
use crate::platform::windows::sink::RecycleProgressSink;
use crate::platform::windows::volume::{path_exists_no_follow, probe_target, to_wide_null};
use crate::platform::{RecycleItemOutcome, ResolvedTarget, TargetProbeResult, TargetSnapshot, TrashEntry};
use chrono::Local;
use windows::core::{HRESULT, PCWSTR};
use windows::Win32::Foundation::{E_ABORT, E_ACCESSDENIED, RPC_E_CHANGED_MODE};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{
    FileOperation, COPYENGINE_E_ACCESSDENIED_READONLY, COPYENGINE_E_ACCESS_DENIED_DEST,
    COPYENGINE_E_ACCESS_DENIED_SRC, COPYENGINE_E_CANCELLED, COPYENGINE_E_PATH_TOO_DEEP_DEST,
    COPYENGINE_E_PATH_TOO_DEEP_SRC, COPYENGINE_E_RECYCLE_BIN_NOT_FOUND,
    COPYENGINE_E_RECYCLE_FORCE_NUKE, COPYENGINE_E_RECYCLE_PATH_TOO_LONG,
    COPYENGINE_E_RECYCLE_SIZE_TOO_BIG, COPYENGINE_E_RECYCLE_UNKNOWN_ERROR,
    COPYENGINE_E_REQUIRES_ELEVATION, COPYENGINE_E_SHARING_VIOLATION_DEST,
    COPYENGINE_E_SHARING_VIOLATION_SRC, COPYENGINE_E_USER_CANCELLED, FOFX_RECYCLEONDELETE,
    FOF_ALLOWUNDO, FOF_NO_UI, IFileOperation, IFileOperationProgressSink, IShellItem,
    SHCreateItemFromParsingName,
};

pub struct ComGuard {
    should_uninit: bool,
}

impl ComGuard {
    pub fn init() -> Result<Self, HRESULT> {
        unsafe {
            let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            if hr.is_ok() {
                Ok(Self { should_uninit: true })
            } else if hr == RPC_E_CHANGED_MODE {
                Ok(Self { should_uninit: false })
            } else {
                Err(hr)
            }
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.should_uninit {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

fn snapshots_match(a: &TargetSnapshot, b: &TargetSnapshot) -> bool {
    if a.is_reparse_point != b.is_reparse_point || a.is_directory != b.is_directory {
        return false;
    }
    if let (Some(va), Some(vb)) = (a.volume_serial, b.volume_serial) {
        if va != vb {
            return false;
        }
    }
    if let (Some(ia), Some(ib)) = (a.file_index, b.file_index) {
        if ia != ib {
            return false;
        }
    }
    if let (Some(ca), Some(cb)) = (a.creation_time, b.creation_time) {
        if ca != cb {
            return false;
        }
    }
    true
}

fn classify_delete_hresult(hr: HRESULT, blocked_permanent: bool) -> (ErrorCode, String) {
    if blocked_permanent {
        return (
            ErrorCode::ERecycleRefused,
            "目标无法放入回收站（系统试图降级为永久删除，已被安全拦截，文件仍在原处）".to_string(),
        );
    }
    let raw = hr.0 as u32;
    if hr == COPYENGINE_E_RECYCLE_PATH_TOO_LONG
        || hr == COPYENGINE_E_PATH_TOO_DEEP_SRC
        || hr == COPYENGINE_E_PATH_TOO_DEEP_DEST
        || raw == 0x800700CE // ERROR_FILENAME_EXCED_RANGE
    {
        return (
            ErrorCode::ERecycleRefused,
            format!("路径过长或层级过深，回收站拒绝接收（文件仍在原处，0x{:08X}）", raw),
        );
    }
    if hr == COPYENGINE_E_RECYCLE_SIZE_TOO_BIG {
        return (
            ErrorCode::ERecycleRefused,
            format!("条目过大，回收站拒绝接收（文件仍在原处，0x{:08X}）", raw),
        );
    }
    if hr == COPYENGINE_E_RECYCLE_FORCE_NUKE
        || hr == COPYENGINE_E_RECYCLE_BIN_NOT_FOUND
        || hr == COPYENGINE_E_RECYCLE_UNKNOWN_ERROR
    {
        return (
            ErrorCode::ERecycleRefused,
            format!("回收站拒绝接收该条目（文件仍在原处，0x{:08X}）", raw),
        );
    }
    if hr == COPYENGINE_E_SHARING_VIOLATION_SRC
        || hr == COPYENGINE_E_SHARING_VIOLATION_DEST
        || raw == 0x80070020 // ERROR_SHARING_VIOLATION
        || raw == 0x80070021 // ERROR_LOCK_VIOLATION
    {
        return (
            ErrorCode::EInUse,
            format!("文件正被其他程序占用（0x{:08X}）", raw),
        );
    }
    if hr == COPYENGINE_E_ACCESSDENIED_READONLY {
        return (
            ErrorCode::EReadOnly,
            format!("只读属性导致无法操作（0x{:08X}）", raw),
        );
    }
    if hr == COPYENGINE_E_ACCESS_DENIED_SRC
        || hr == COPYENGINE_E_ACCESS_DENIED_DEST
        || hr == COPYENGINE_E_REQUIRES_ELEVATION
        || hr == E_ACCESSDENIED
    {
        return (
            ErrorCode::EAccessDenied,
            format!("没有权限执行此操作（0x{:08X}）", raw),
        );
    }
    if hr == COPYENGINE_E_USER_CANCELLED || hr == COPYENGINE_E_CANCELLED || hr == E_ABORT {
        return (
            ErrorCode::EOperationAborted,
            format!("操作被系统中止（0x{:08X}）", raw),
        );
    }

    (
        ErrorCode::EUnknownSystem,
        format!("系统返回错误 HRESULT 0x{:08X}", raw),
    )
}

/// Sends a single checked target to the Windows Recycle Bin via an isolated `IFileOperation`
/// instance (Section 7.1 point 3, V8) and verifies the outcome (R15, Section 7.4).
pub fn recycle_single(target: &ResolvedTarget) -> RecycleItemOutcome {
    // R8: Re-verify target identity right before executing IFileOperation
    match probe_target(&target.normalized) {
        TargetProbeResult::Found { snapshot, .. } => {
            if !snapshots_match(&target.snapshot, &snapshot) {
                return RecycleItemOutcome {
                    outcome: ItemOutcome::Failed,
                    code: ErrorCode::ETargetChanged,
                    message: "检查通过后目标对象已发生变化（可能被替换），已停止操作".to_string(),
                    trash_id: None,
                    verified: Some(VerificationInfo {
                        absent_at_source: Some(false),
                        present_in_bin: Some(false),
                        present_at_original: None,
                        absent_in_bin: None,
                    }),
                };
            }
        }
        TargetProbeResult::NotFound => {
            return RecycleItemOutcome {
                outcome: ItemOutcome::Failed,
                code: ErrorCode::ETargetChanged,
                message: "检查通过后目标已消失，已停止操作".to_string(),
                trash_id: None,
                verified: Some(VerificationInfo {
                    absent_at_source: Some(true),
                    present_in_bin: Some(false),
                    present_at_original: None,
                    absent_in_bin: None,
                }),
            };
        }
        TargetProbeResult::AccessDenied(detail) => {
            return RecycleItemOutcome {
                outcome: ItemOutcome::Failed,
                code: ErrorCode::EAccessDenied,
                message: format!("执行前复核权限失败：{}", detail),
                trash_id: None,
                verified: None,
            };
        }
        TargetProbeResult::SystemError(detail) => {
            return RecycleItemOutcome {
                outcome: ItemOutcome::Failed,
                code: ErrorCode::EUnknownSystem,
                message: format!("执行前复核失败：{}", detail),
                trash_id: None,
                verified: None,
            };
        }
    }

    let op_start = Local::now();

    let (perform_res, sink_state) = unsafe {
        let pfo: IFileOperation = match CoCreateInstance(&FileOperation, None, CLSCTX_ALL) {
            Ok(p) => p,
            Err(e) => {
                return RecycleItemOutcome {
                    outcome: ItemOutcome::Failed,
                    code: ErrorCode::EUnknownSystem,
                    message: format!("创建 IFileOperation 失败：0x{:08X}", e.code().0 as u32),
                    trash_id: None,
                    verified: None,
                };
            }
        };

        // Section 7.2: recycle-only, headless flags (no nuke warning or confirmation dialogs)
        if let Err(e) = pfo.SetOperationFlags(FOF_ALLOWUNDO | FOF_NO_UI | FOFX_RECYCLEONDELETE) {
            return RecycleItemOutcome {
                outcome: ItemOutcome::Failed,
                code: ErrorCode::EUnknownSystem,
                message: format!("设置 IFileOperation 标志失败：0x{:08X}", e.code().0 as u32),
                trash_id: None,
                verified: None,
            };
        }

        let sink_iface: IFileOperationProgressSink = RecycleProgressSink::new().into();
        let cookie = match pfo.Advise(&sink_iface) {
            Ok(c) => c,
            Err(e) => {
                return RecycleItemOutcome {
                    outcome: ItemOutcome::Failed,
                    code: ErrorCode::EUnknownSystem,
                    message: format!("注册进度回调失败：0x{:08X}", e.code().0 as u32),
                    trash_id: None,
                    verified: None,
                };
            }
        };

        let wide = to_wide_null(&target.normalized);
        let item: IShellItem = match SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None) {
            Ok(it) => it,
            Err(e) => {
                let _ = pfo.Unadvise(cookie);
                let (code, msg) = classify_delete_hresult(e.code(), false);
                let absent = matches!(path_exists_no_follow(&target.normalized), Ok(false));
                return RecycleItemOutcome {
                    outcome: ItemOutcome::Failed,
                    code,
                    message: msg,
                    trash_id: None,
                    verified: Some(VerificationInfo {
                        absent_at_source: Some(absent),
                        present_in_bin: Some(false),
                        present_at_original: None,
                        absent_in_bin: None,
                    }),
                };
            }
        };

        if let Err(e) = pfo.DeleteItem(&item, None) {
            let _ = pfo.Unadvise(cookie);
            let (code, msg) = classify_delete_hresult(e.code(), false);
            return RecycleItemOutcome {
                outcome: ItemOutcome::Failed,
                code,
                message: msg,
                trash_id: None,
                verified: None,
            };
        }

        let perform_res = pfo.PerformOperations().map_err(|e| e.code());
        let _ = pfo.Unadvise(cookie);
        let sink_impl: &RecycleProgressSink = windows_core::AsImpl::as_impl(&sink_iface);
        let state = sink_impl.state.borrow().clone();
        (perform_res, state)
    };

    let op_end = Local::now();

    // Check failure signals from PreDeleteItem guard, PostDeleteItem hrDelete, or PerformOperations
    let first_failed_hr = sink_state
        .post_delete_hrs
        .iter()
        .copied()
        .find(|hr| hr.is_err());

    let created_parsing = sink_state
        .newly_created_parsing
        .iter()
        .find_map(|opt| opt.clone());

    if sink_state.blocked_permanent_delete || first_failed_hr.is_some() || perform_res.is_err() {
        let hr = first_failed_hr
            .or_else(|| perform_res.err())
            .unwrap_or(COPYENGINE_E_RECYCLE_FORCE_NUKE);
        let (code, message) = classify_delete_hresult(hr, sink_state.blocked_permanent_delete);
        let absent_at_source = matches!(path_exists_no_follow(&target.normalized), Ok(false));
        return RecycleItemOutcome {
            outcome: ItemOutcome::Failed,
            code,
            message,
            trash_id: None,
            verified: Some(VerificationInfo {
                absent_at_source: Some(absent_at_source),
                present_in_bin: Some(false),
                present_at_original: None,
                absent_in_bin: None,
            }),
        };
    }

    // R15 & Section 7.4: Must verify BOTH absent at source AND present in Recycle Bin!
    let absent_at_source = matches!(path_exists_no_follow(&target.normalized), Ok(false));
    let verified_entry = created_parsing.as_deref().and_then(|parsing| {
        verify_newly_created_item(parsing, &target.compare_key, op_start, op_end)
    });
    let present_in_bin = verified_entry.is_some();

    if absent_at_source && present_in_bin {
        let entry = verified_entry.unwrap();
        RecycleItemOutcome {
            outcome: ItemOutcome::Succeeded,
            code: ErrorCode::EOk,
            message: "已进入回收站".to_string(),
            trash_id: Some(entry.trash_id),
            verified: None,
        }
    } else {
        RecycleItemOutcome {
            outcome: ItemOutcome::Unknown,
            code: ErrorCode::EVerifyFailed,
            message: format!(
                "结果无法确认（原位置消失={}，回收站命中={}）",
                absent_at_source, present_in_bin
            ),
            trash_id: verified_entry.map(|e| e.trash_id),
            verified: Some(VerificationInfo {
                absent_at_source: Some(absent_at_source),
                present_in_bin: Some(present_in_bin),
                present_at_original: None,
                absent_in_bin: None,
            }),
        }
    }
}

/// Restores a single entry from the Recycle Bin back to its original path using
/// `IFileOperation::MoveItem` (Section 7.5 — never copy+delete!) and verifies both
/// presence at original path and absence from Recycle Bin enumeration.
pub fn restore_single(entry: &TrashEntry) -> RecycleItemOutcome {
    // Re-locate unique live IShellItem in Recycle Bin right before MoveItem
    let live_matches = match find_shell_items_by_trash_id(&entry.trash_id) {
        Ok(m) => m,
        Err(e) => {
            return RecycleItemOutcome {
                outcome: ItemOutcome::Failed,
                code: e.code,
                message: e.message,
                trash_id: Some(entry.trash_id.clone()),
                verified: None,
            };
        }
    };

    if live_matches.is_empty() {
        return RecycleItemOutcome {
            outcome: ItemOutcome::Failed,
            code: ErrorCode::ETrashIdNotFound,
            message: "执行恢复前回收站条目已不存在".to_string(),
            trash_id: Some(entry.trash_id.clone()),
            verified: None,
        };
    }
    if live_matches.len() > 1 {
        return RecycleItemOutcome {
            outcome: ItemOutcome::Failed,
            code: ErrorCode::ETrashIdAmbiguous,
            message: "执行恢复前条目标识命中多条，已停止操作".to_string(),
            trash_id: Some(entry.trash_id.clone()),
            verified: None,
        };
    }

    let (_, rb_item) = &live_matches[0];

    let (perform_res, sink_state) = unsafe {
        let pfo: IFileOperation = match CoCreateInstance(&FileOperation, None, CLSCTX_ALL) {
            Ok(p) => p,
            Err(e) => {
                return RecycleItemOutcome {
                    outcome: ItemOutcome::Failed,
                    code: ErrorCode::ERestoreFailed,
                    message: format!("创建 IFileOperation 失败：0x{:08X}", e.code().0 as u32),
                    trash_id: Some(entry.trash_id.clone()),
                    verified: None,
                };
            }
        };

        if let Err(e) = pfo.SetOperationFlags(FOF_NO_UI) {
            return RecycleItemOutcome {
                outcome: ItemOutcome::Failed,
                code: ErrorCode::ERestoreFailed,
                message: format!("设置恢复标志失败：0x{:08X}", e.code().0 as u32),
                trash_id: Some(entry.trash_id.clone()),
                verified: None,
            };
        }

        let sink_iface: IFileOperationProgressSink = RecycleProgressSink::new().into();
        let cookie = match pfo.Advise(&sink_iface) {
            Ok(c) => c,
            Err(e) => {
                return RecycleItemOutcome {
                    outcome: ItemOutcome::Failed,
                    code: ErrorCode::ERestoreFailed,
                    message: format!("注册恢复回调失败：0x{:08X}", e.code().0 as u32),
                    trash_id: Some(entry.trash_id.clone()),
                    verified: None,
                };
            }
        };

        let dest_wide = to_wide_null(&entry.original_parent);
        let dest_folder: IShellItem =
            match SHCreateItemFromParsingName(PCWSTR(dest_wide.as_ptr()), None) {
                Ok(it) => it,
                Err(e) => {
                    let _ = pfo.Unadvise(cookie);
                    return RecycleItemOutcome {
                        outcome: ItemOutcome::Failed,
                        code: ErrorCode::ERestoreFailed,
                        message: format!(
                            "无法打开原父目录 {}：0x{:08X}",
                            entry.original_parent,
                            e.code().0 as u32
                        ),
                        trash_id: Some(entry.trash_id.clone()),
                        verified: None,
                    };
                }
            };

        let new_name_wide = to_wide_null(&entry.name);
        if let Err(e) = pfo.MoveItem(rb_item, &dest_folder, PCWSTR(new_name_wide.as_ptr()), None) {
            let _ = pfo.Unadvise(cookie);
            return RecycleItemOutcome {
                outcome: ItemOutcome::Failed,
                code: ErrorCode::ERestoreFailed,
                message: format!("排入恢复移动失败：0x{:08X}", e.code().0 as u32),
                trash_id: Some(entry.trash_id.clone()),
                verified: None,
            };
        }

        let perform_res = pfo.PerformOperations().map_err(|e| e.code());
        let _ = pfo.Unadvise(cookie);
        let sink_impl: &RecycleProgressSink = windows_core::AsImpl::as_impl(&sink_iface);
        let state = sink_impl.state.borrow().clone();
        (perform_res, state)
    };

    let move_failed_hr = sink_state.post_move_hrs.iter().copied().find(|hr| hr.is_err());

    if move_failed_hr.is_some() || perform_res.is_err() {
        let hr = move_failed_hr
            .or_else(|| perform_res.err())
            .unwrap_or(E_ABORT);
        let raw = hr.0 as u32;
        return RecycleItemOutcome {
            outcome: ItemOutcome::Failed,
            code: ErrorCode::ERestoreFailed,
            message: format!("恢复移动失败（HRESULT 0x{:08X}）", raw),
            trash_id: Some(entry.trash_id.clone()),
            verified: Some(VerificationInfo {
                absent_at_source: None,
                present_in_bin: None,
                present_at_original: Some(matches!(
                    path_exists_no_follow(&entry.original_path),
                    Ok(true)
                )),
                absent_in_bin: Some(
                    !is_trash_id_present_in_bin(&entry.trash_id).unwrap_or(true),
                ),
            }),
        };
    }

    // Section 7.5 step 3: Verify BOTH present at original AND absent in bin (via fresh EnumObjects!)
    let present_at_original = matches!(path_exists_no_follow(&entry.original_path), Ok(true));
    let absent_in_bin = match is_trash_id_present_in_bin(&entry.trash_id) {
        Ok(still_present) => !still_present,
        Err(_) => false,
    };

    let verified = VerificationInfo {
        absent_at_source: None,
        present_in_bin: None,
        present_at_original: Some(present_at_original),
        absent_in_bin: Some(absent_in_bin),
    };

    if present_at_original && absent_in_bin {
        RecycleItemOutcome {
            outcome: ItemOutcome::Succeeded,
            code: ErrorCode::EOk,
            message: "已恢复到原位置".to_string(),
            trash_id: Some(entry.trash_id.clone()),
            verified: Some(verified),
        }
    } else {
        RecycleItemOutcome {
            outcome: ItemOutcome::Unknown,
            code: ErrorCode::EVerifyFailed,
            message: format!(
                "恢复结果无法确认（原位置出现={}，回收站消失={}）",
                present_at_original, absent_in_bin
            ),
            trash_id: Some(entry.trash_id.clone()),
            verified: Some(verified),
        }
    }
}
