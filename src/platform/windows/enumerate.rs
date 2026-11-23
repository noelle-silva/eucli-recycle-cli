use crate::model::error_code::ErrorCode;
use crate::model::outcome::EntryKind;
use crate::platform::windows::volume::to_wide_null;
use crate::platform::{ListFilter, PlatformError, TrashEntry};
use crate::rules::canonical::{is_same_key, is_strict_ancestor};
use chrono::{DateTime, Local, TimeZone, Utc};
use sha2::{Digest, Sha256};
use windows::core::{GUID, PCWSTR};
use windows::Win32::Foundation::{FILETIME, HWND, PROPERTYKEY, S_OK};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::SystemServices::{SFGAO_FOLDER, SFGAO_STREAM};
use windows::Win32::System::Variant::{VariantToFileTime, VariantToUInt64, PSTF_UTC};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    FOLDERID_RecycleBinFolder, IShellFolder2, IShellItem, KF_FLAG_DEFAULT, SHCONTF_FOLDERS,
    SHCONTF_INCLUDEHIDDEN, SHCONTF_NONFOLDERS, SHCreateItemWithParent, SHGetDesktopFolder,
    SHGetKnownFolderIDList,
};

const SCID_ORIGINAL_LOCATION: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0x9b174b33_40ff_11d2_a27e_00c04fc30871),
    pid: 2,
};

const SCID_DATE_DELETED: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0x9b174b33_40ff_11d2_a27e_00c04fc30871),
    pid: 3,
};

const PKEY_FILE_NAME: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0xb725f130_47ef_101a_a5f1_02608c9eebac),
    pid: 10,
};

const PKEY_SIZE: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0xb725f130_47ef_101a_a5f1_02608c9eebac),
    pid: 12,
};

/// Section 9.4: SHA-256 of (original_path_key, name, deleted_at_filetime, size), first 8 bytes as lowercase hex.
pub fn compute_trash_id(
    original_path_key: &str,
    name: &str,
    deleted_at_filetime: u64,
    size: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(original_path_key.as_bytes());
    hasher.update(b"\0");
    hasher.update(name.as_bytes());
    hasher.update(b"\0");
    hasher.update(deleted_at_filetime.to_le_bytes());
    hasher.update(b"\0");
    hasher.update(size.to_le_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..8])
}

pub fn filetime_to_u64(ft: FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
}

/// Converts Windows UTC FILETIME (100-ns intervals since 1601-01-01) to local DateTime.
pub fn filetime_to_local_datetime(ft: u64) -> Option<DateTime<Local>> {
    const EPOCH_DIFF_100NS: u64 = 116_444_736_000_000_000;
    if ft < EPOCH_DIFF_100NS {
        return None;
    }
    let since_unix_100ns = ft - EPOCH_DIFF_100NS;
    let secs = (since_unix_100ns / 10_000_000) as i64;
    let nanos = ((since_unix_100ns % 10_000_000) * 100) as u32;
    let utc = Utc.timestamp_opt(secs, nanos).single()?;
    Some(utc.with_timezone(&Local))
}

pub fn open_recycle_bin_folder() -> Result<IShellFolder2, PlatformError> {
    unsafe {
        let desktop = SHGetDesktopFolder().map_err(|e| PlatformError {
            code: ErrorCode::EUnknownSystem,
            message: format!("无法获取桌面 Shell 文件夹：0x{:08X}", e.code().0 as u32),
        })?;
        let rb_pidl =
            SHGetKnownFolderIDList(&FOLDERID_RecycleBinFolder, KF_FLAG_DEFAULT.0 as u32, None)
                .map_err(|e| PlatformError {
                    code: ErrorCode::EUnknownSystem,
                    message: format!("无法打开系统回收站接口：0x{:08X}", e.code().0 as u32),
                })?;
        let folder: Result<IShellFolder2, _> = desktop.BindToObject(rb_pidl, None);
        CoTaskMemFree(Some(rb_pidl as *const _));
        folder.map_err(|e| PlatformError {
            code: ErrorCode::EUnknownSystem,
            message: format!("无法绑定回收站 IShellFolder2：0x{:08X}", e.code().0 as u32),
        })
    }
}

fn join_original_path(parent: &str, name: &str) -> String {
    let trimmed = parent.trim_end_matches('\\');
    if trimmed.len() == 2 && trimmed.ends_with(':') {
        format!("{}\\{}", trimmed, name)
    } else if trimmed.is_empty() {
        name.to_string()
    } else {
        format!("{}\\{}", trimmed, name)
    }
}

fn normalize_original_key(path: &str) -> String {
    let s = path.replace('/', "\\");
    let trimmed = s.trim_end_matches('\\');
    if trimmed.len() == 2 && trimmed.ends_with(':') {
        format!("{}\\", trimmed.to_ascii_lowercase())
    } else {
        trimmed.to_ascii_lowercase()
    }
}

fn read_pidl_kind(rb_folder: &IShellFolder2, pidl: *const ITEMIDLIST) -> EntryKind {
    unsafe {
        let mut attrs: u32 = SFGAO_FOLDER.0 | SFGAO_STREAM.0;
        let arr = [pidl];
        if rb_folder.GetAttributesOf(&arr, &mut attrs).is_ok() {
            let is_folder = (attrs & SFGAO_FOLDER.0) != 0;
            let is_stream = (attrs & SFGAO_STREAM.0) != 0;
            if is_folder && !is_stream {
                EntryKind::Directory
            } else {
                EntryKind::File
            }
        } else {
            EntryKind::Unknown
        }
    }
}

fn read_pidl_entry(rb_folder: &IShellFolder2, pidl: *const ITEMIDLIST) -> Option<TrashEntry> {
    unsafe {
        let name = rb_folder
            .GetDetailsEx(pidl, &PKEY_FILE_NAME)
            .ok()
            .map(|v| v.to_string())?;
        let original_parent = rb_folder
            .GetDetailsEx(pidl, &SCID_ORIGINAL_LOCATION)
            .ok()
            .map(|v| v.to_string())
            .unwrap_or_default();

        let deleted_ft_u64 = rb_folder
            .GetDetailsEx(pidl, &SCID_DATE_DELETED)
            .ok()
            .and_then(|v| VariantToFileTime(&v, PSTF_UTC).ok())
            .map(filetime_to_u64)
            .unwrap_or(0);

        let size = rb_folder
            .GetDetailsEx(pidl, &PKEY_SIZE)
            .ok()
            .and_then(|v| VariantToUInt64(&v).ok())
            .unwrap_or(0);

        let original_path = join_original_path(&original_parent, &name);
        let original_path_key = normalize_original_key(&original_path);
        let trash_id = compute_trash_id(&original_path_key, &name, deleted_ft_u64, size);
        let deleted_at_dt = filetime_to_local_datetime(deleted_ft_u64);
        let deleted_at = deleted_at_dt
            .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, false))
            .unwrap_or_default();
        let kind = read_pidl_kind(rb_folder, pidl);

        Some(TrashEntry {
            trash_id,
            name,
            original_parent,
            original_path,
            original_path_key,
            deleted_at,
            deleted_at_dt,
            deleted_at_filetime: deleted_ft_u64,
            size,
            kind,
        })
    }
}

/// Fast verification of a newly created Recycle Bin item returned by `PostDeleteItem` (V3 + Section 7.4).
pub fn verify_newly_created_item(
    parsing_name: &str,
    expected_original_key: &str,
    window_start: DateTime<Local>,
    window_end: DateTime<Local>,
) -> Option<TrashEntry> {
    let rb_folder = open_recycle_bin_folder().ok()?;
    let wide = to_wide_null(parsing_name);
    unsafe {
        let mut child_pidl: *mut ITEMIDLIST = std::ptr::null_mut();
        let hr = rb_folder.ParseDisplayName(
            HWND::default(),
            None,
            PCWSTR(wide.as_ptr()),
            None,
            &mut child_pidl,
            std::ptr::null_mut(),
        );
        if hr.is_err() || child_pidl.is_null() {
            return None;
        }
        let entry = read_pidl_entry(&rb_folder, child_pidl);
        CoTaskMemFree(Some(child_pidl as *const _));

        let entry = entry?;
        if !is_same_key(&entry.original_path_key, expected_original_key) {
            return None;
        }
        if let Some(dt) = entry.deleted_at_dt {
            let min_t = window_start - chrono::Duration::seconds(10);
            let max_t = window_end + chrono::Duration::seconds(10);
            if dt < min_t || dt > max_t {
                return None;
            }
        }
        Some(entry)
    }
}

/// Enumerates entries in the Windows Recycle Bin using official `IShellFolder2::EnumObjects`.
pub fn enumerate_recycle_bin(filter: &ListFilter) -> Result<Vec<TrashEntry>, PlatformError> {
    let rb_folder = open_recycle_bin_folder()?;
    let mut results = Vec::new();

    unsafe {
        let mut enum_id_list = None;
        rb_folder
            .EnumObjects(
                HWND::default(),
                (SHCONTF_FOLDERS.0 | SHCONTF_NONFOLDERS.0 | SHCONTF_INCLUDEHIDDEN.0) as u32,
                &mut enum_id_list,
            )
            .ok()
            .map_err(|e| PlatformError {
                code: ErrorCode::EUnknownSystem,
                message: format!("枚举回收站失败：0x{:08X}", e.code().0 as u32),
            })?;

        let Some(enumerator) = enum_id_list else {
            return Ok(results);
        };

        let mut batch: [*mut ITEMIDLIST; 64] = [std::ptr::null_mut(); 64];
        loop {
            let mut fetched = 0u32;
            let hr = enumerator.Next(&mut batch, Some(&mut fetched));
            if fetched == 0 {
                break;
            }
            for &pidl in &batch[..fetched as usize] {
                // Fast pre-filtering on filename when --name is given
                if let Some(ref name_sub) = filter.name_substr {
                    let matches_name = rb_folder
                        .GetDetailsEx(pidl, &PKEY_FILE_NAME)
                        .ok()
                        .map(|v| v.to_string().contains(name_sub.as_str()))
                        .unwrap_or(false);
                    if !matches_name {
                        CoTaskMemFree(Some(pidl as *const _));
                        continue;
                    }
                }

                // Fast pre-filtering on original location when --from is given
                if let Some(ref from_key) = filter.from_key {
                    let matches_from = rb_folder
                        .GetDetailsEx(pidl, &SCID_ORIGINAL_LOCATION)
                        .ok()
                        .map(|v| {
                            let loc_key = normalize_original_key(&v.to_string());
                            is_same_key(&loc_key, from_key)
                                || is_strict_ancestor(from_key, &loc_key)
                        })
                        .unwrap_or(false);
                    if !matches_from {
                        CoTaskMemFree(Some(pidl as *const _));
                        continue;
                    }
                }

                if let Some(entry) = read_pidl_entry(&rb_folder, pidl) {
                    let time_ok = match (filter.after, filter.before, entry.deleted_at_dt) {
                        (Some(after), _, Some(dt)) if dt < after => false,
                        (_, Some(before), Some(dt)) if dt > before => false,
                        (Some(_), _, None) | (_, Some(_), None) => false,
                        _ => true,
                    };
                    if time_ok {
                        results.push(entry);
                    }
                }

                CoTaskMemFree(Some(pidl as *const _));
            }
            if hr != S_OK {
                break;
            }
        }
    }

    // Sort deterministically by deleted_at descending, then trash_id ascending (R23)
    results.sort_by(|a, b| {
        b.deleted_at_filetime
            .cmp(&a.deleted_at_filetime)
            .then_with(|| a.trash_id.cmp(&b.trash_id))
    });

    Ok(results)
}

/// Finds all Shell items inside Recycle Bin whose computed `trash_id` equals `target_id`.
/// Used by `restore` to obtain the live `IShellItem` inside `FOLDERID_RecycleBinFolder`.
pub fn find_shell_items_by_trash_id(target_id: &str) -> Result<Vec<(TrashEntry, IShellItem)>, PlatformError> {
    let target_lower = target_id.to_ascii_lowercase();
    let rb_folder = open_recycle_bin_folder()?;
    let mut matches = Vec::new();

    unsafe {
        let mut enum_id_list = None;
        rb_folder
            .EnumObjects(
                HWND::default(),
                (SHCONTF_FOLDERS.0 | SHCONTF_NONFOLDERS.0 | SHCONTF_INCLUDEHIDDEN.0) as u32,
                &mut enum_id_list,
            )
            .ok()
            .map_err(|e| PlatformError {
                code: ErrorCode::EUnknownSystem,
                message: format!("枚举回收站失败：0x{:08X}", e.code().0 as u32),
            })?;

        let Some(enumerator) = enum_id_list else {
            return Ok(matches);
        };

        let mut batch: [*mut ITEMIDLIST; 64] = [std::ptr::null_mut(); 64];
        loop {
            let mut fetched = 0u32;
            let hr = enumerator.Next(&mut batch, Some(&mut fetched));
            if fetched == 0 {
                break;
            }
            for &pidl in &batch[..fetched as usize] {
                if let Some(entry) = read_pidl_entry(&rb_folder, pidl) {
                    if entry.trash_id == target_lower {
                        if let Ok(item) = SHCreateItemWithParent::<_, IShellItem>(None, &rb_folder, pidl) {
                            matches.push((entry, item));
                        }
                    }
                }
                CoTaskMemFree(Some(pidl as *const _));
            }
            if hr != S_OK {
                break;
            }
        }
    }

    Ok(matches)
}

/// Verifies whether any entry with `trash_id` still exists via fresh `EnumObjects` scan (Discovery 3).
pub fn is_trash_id_present_in_bin(trash_id: &str) -> Result<bool, PlatformError> {
    let found = find_shell_items_by_trash_id(trash_id)?;
    Ok(!found.is_empty())
}
