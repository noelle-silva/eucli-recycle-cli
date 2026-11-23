use crate::model::outcome::EntryKind;
use crate::platform::{TargetProbeResult, TargetSnapshot};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND,
    ERROR_SHARING_VIOLATION,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, GetFileAttributesW, GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    INVALID_FILE_ATTRIBUTES, OPEN_EXISTING,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{FOLDERID_RoamingAppData, SHGetKnownFolderPath, KF_FLAG_DEFAULT};

pub fn to_wide_null(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

/// Converts a normalized drive path to an extended-length wide path ONLY when needed (>240 UTF-16 units)
/// for read-only Win32 metadata probing (Section 5.2 allows >260 chars through gate).
fn probe_wide_path(path: &str) -> Vec<u16> {
    if path.encode_utf16().count() > 240 && !path.starts_with(r"\\?\") {
        let extended = format!(r"\\?\{}", path);
        to_wide_null(&extended)
    } else {
        to_wide_null(path)
    }
}

/// Probes an object without following symlinks/junctions (R10), capturing an identity snapshot for R8.
pub fn probe_target(path: &str) -> TargetProbeResult {
    let wide = probe_wide_path(path);
    let attrs = unsafe { GetFileAttributesW(PCWSTR(wide.as_ptr())) };
    if attrs == INVALID_FILE_ATTRIBUTES {
        let err = unsafe { GetLastError() };
        return match err {
            ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => TargetProbeResult::NotFound,
            ERROR_ACCESS_DENIED => TargetProbeResult::AccessDenied(format!("Win32 错误码 {}", err.0)),
            ERROR_SHARING_VIOLATION => {
                // Object exists even if sharing violation occurred on attributes query
                TargetProbeResult::Found {
                    kind: EntryKind::File,
                    is_reparse_point: false,
                    snapshot: TargetSnapshot {
                        attributes: 0,
                        is_reparse_point: false,
                        is_directory: false,
                        volume_serial: None,
                        file_index: None,
                        creation_time: None,
                    },
                }
            }
            _ => TargetProbeResult::SystemError(format!("GetFileAttributesW 失败，Win32 错误码 {}", err.0)),
        };
    }

    let is_reparse_point = (attrs & FILE_ATTRIBUTE_REPARSE_POINT.0) != 0;
    let is_directory = (attrs & FILE_ATTRIBUTE_DIRECTORY.0) != 0;
    let kind = if is_reparse_point {
        EntryKind::File
    } else if is_directory {
        EntryKind::Directory
    } else {
        EntryKind::File
    };

    let (volume_serial, file_index, creation_time) = query_handle_identity(&wide);

    TargetProbeResult::Found {
        kind,
        is_reparse_point,
        snapshot: TargetSnapshot {
            attributes: attrs,
            is_reparse_point,
            is_directory,
            volume_serial,
            file_index,
            creation_time,
        },
    }
}

fn query_handle_identity(wide: &[u16]) -> (Option<u32>, Option<u64>, Option<u64>) {
    unsafe {
        let handle = CreateFileW(
            PCWSTR(wide.as_ptr()),
            0, // metadata-only access; works on locked files and reparse points
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
            None,
        );
        let Ok(h) = handle else {
            return (None, None, None);
        };
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        let ok = GetFileInformationByHandle(h, &mut info).is_ok();
        let _ = CloseHandle(h);
        if ok {
            let idx = ((info.nFileIndexHigh as u64) << 32) | (info.nFileIndexLow as u64);
            let ctime = ((info.ftCreationTime.dwHighDateTime as u64) << 32)
                | (info.ftCreationTime.dwLowDateTime as u64);
            (Some(info.dwVolumeSerialNumber), Some(idx), Some(ctime))
        } else {
            (None, None, None)
        }
    }
}

/// Non-following existence check.
pub fn path_exists_no_follow(path: &str) -> Result<bool, u32> {
    let wide = probe_wide_path(path);
    let attrs = unsafe { GetFileAttributesW(PCWSTR(wide.as_ptr())) };
    if attrs == INVALID_FILE_ATTRIBUTES {
        let err = unsafe { GetLastError() };
        if err == ERROR_FILE_NOT_FOUND || err == ERROR_PATH_NOT_FOUND {
            Ok(false)
        } else {
            Err(err.0)
        }
    } else {
        Ok(true)
    }
}

/// Returns true iff `path`'s parent directory exists and is a directory.
pub fn parent_dir_exists(path: &str) -> bool {
    let Some(parent) = Path::new(path).parent() else {
        return false;
    };
    let parent_str = parent.to_string_lossy();
    let wide = probe_wide_path(&parent_str);
    let attrs = unsafe { GetFileAttributesW(PCWSTR(wide.as_ptr())) };
    attrs != INVALID_FILE_ATTRIBUTES && (attrs & FILE_ATTRIBUTE_DIRECTORY.0) != 0
}

/// Queries Roaming AppData via Windows Known Folder API (never reads environment variables — R23).
fn known_roaming_appdata() -> Option<PathBuf> {
    unsafe {
        let pwstr = SHGetKnownFolderPath(&FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, None).ok()?;
        let s = pwstr.to_string().ok();
        CoTaskMemFree(Some(pwstr.0 as *const _));
        s.map(PathBuf::from)
    }
}

/// Returns `C:\Users\<current_user>\AppData` via Known Folder API (Section 6.2).
pub fn user_appdata_root() -> Option<PathBuf> {
    known_roaming_appdata().and_then(|p| p.parent().map(|pp| pp.to_path_buf()))
}

/// Returns `<RoamingAppData>\eucli-recycle` via Known Folder API (Section 6.2 & 11.1).
pub fn default_config_dir() -> Option<PathBuf> {
    known_roaming_appdata().map(|p| p.join("eucli-recycle"))
}
