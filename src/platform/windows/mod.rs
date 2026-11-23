pub mod enumerate;
pub mod operations;
pub mod sink;
pub mod volume;

use crate::model::error_code::ErrorCode;
use crate::model::outcome::ItemOutcome;
use crate::platform::windows::operations::ComGuard;
use crate::platform::{
    ListFilter, PlatformError, RecycleBin, RecycleItemOutcome, ResolvedTarget, TrashEntry,
};

pub struct WindowsRecycleBin;

impl WindowsRecycleBin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsRecycleBin {
    fn default() -> Self {
        Self::new()
    }
}

impl RecycleBin for WindowsRecycleBin {
    fn recycle(&self, targets: &[ResolvedTarget]) -> Vec<RecycleItemOutcome> {
        let _com = match ComGuard::init() {
            Ok(g) => g,
            Err(hr) => {
                return targets
                    .iter()
                    .map(|_| RecycleItemOutcome {
                        outcome: ItemOutcome::Failed,
                        code: ErrorCode::EUnknownSystem,
                        message: format!("COM 初始化失败：0x{:08X}", hr.0 as u32),
                        trash_id: None,
                        verified: None,
                    })
                    .collect();
            }
        };

        let mut results = Vec::with_capacity(targets.len());
        let mut stopped = false;

        // R13 & Section 7.1 point 3: Execute one target at a time, stop immediately on failure
        for target in targets {
            if stopped {
                results.push(RecycleItemOutcome {
                    outcome: ItemOutcome::NotRun,
                    code: ErrorCode::EStopped,
                    message: "前面有目标失败，按规则停止".to_string(),
                    trash_id: None,
                    verified: None,
                });
                continue;
            }
            let outcome = operations::recycle_single(target);
            if outcome.outcome != ItemOutcome::Succeeded {
                stopped = true;
            }
            results.push(outcome);
        }

        results
    }

    fn enumerate(&self, filter: &ListFilter) -> Result<Vec<TrashEntry>, PlatformError> {
        let _com = ComGuard::init().map_err(|hr| PlatformError {
            code: ErrorCode::EUnknownSystem,
            message: format!("COM 初始化失败：0x{:08X}", hr.0 as u32),
        })?;
        enumerate::enumerate_recycle_bin(filter)
    }

    fn restore(&self, entries: &[TrashEntry]) -> Vec<RecycleItemOutcome> {
        let _com = match ComGuard::init() {
            Ok(g) => g,
            Err(hr) => {
                return entries
                    .iter()
                    .map(|e| RecycleItemOutcome {
                        outcome: ItemOutcome::Failed,
                        code: ErrorCode::ERestoreFailed,
                        message: format!("COM 初始化失败：0x{:08X}", hr.0 as u32),
                        trash_id: Some(e.trash_id.clone()),
                        verified: None,
                    })
                    .collect();
            }
        };

        let mut results = Vec::with_capacity(entries.len());
        let mut stopped = false;

        // R13: Stop immediately on first restore failure
        for entry in entries {
            if stopped {
                results.push(RecycleItemOutcome {
                    outcome: ItemOutcome::NotRun,
                    code: ErrorCode::EStopped,
                    message: "前面有条目恢复失败，按规则停止".to_string(),
                    trash_id: Some(entry.trash_id.clone()),
                    verified: None,
                });
                continue;
            }
            let outcome = operations::restore_single(entry);
            if outcome.outcome != ItemOutcome::Succeeded {
                stopped = true;
            }
            results.push(outcome);
        }

        results
    }
}
