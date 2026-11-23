use crate::model::error_code::ErrorCode;

pub const MAX_UTF16_PATH_LEN: usize = 32_767;
pub const MAX_UTF16_SEGMENT_LEN: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedPath {
    /// Original drive letter + normalized backslashes, no trailing slash except root `X:\`.
    pub normalized: String,
    /// Lowercased form for case-insensitive Windows comparisons (Section 5.4).
    pub compare_key: String,
    pub drive_letter: char,
    pub is_root: bool,
}

/// Validates and normalizes an input path according to Section 5.1, 5.2, and 6.3.3.
pub fn validate_and_normalize_path(input: &str) -> Result<NormalizedPath, (ErrorCode, String)> {
    // 1. Empty string
    if input.is_empty() {
        return Err((ErrorCode::EEmptyPath, "路径不能为空字符串".to_string()));
    }

    let bytes = input.as_bytes();

    // 5. Device path: \\?\ or \\.\ (also handle / equivalents)
    if bytes.len() >= 4
        && is_sep(bytes[0])
        && is_sep(bytes[1])
        && (bytes[2] == b'?' || bytes[2] == b'.')
        && is_sep(bytes[3])
    {
        return Err((
            ErrorCode::EDevicePath,
            format!("拒绝设备路径写法：{}", input),
        ));
    }

    // 6. UNC path: \\server\share (not device path)
    if bytes.len() >= 2 && is_sep(bytes[0]) && is_sep(bytes[1]) {
        return Err((
            ErrorCode::EUncUnsupported,
            format!("v1 不支持 UNC 网络路径：{}", input),
        ));
    }

    // 4. Leading separator without drive letter: \work\a.txt or /e/work/a.txt
    if is_sep(bytes[0]) {
        return Err((
            ErrorCode::ENotAbsolute,
            format!("路径必须以盘符绝对路径开头（如 E:\\...）：{}", input),
        ));
    }

    // Check drive prefix: must have `:` at byte index 1
    if bytes.len() >= 2 && bytes[1] == b':' {
        // 12. Drive letter must be ASCII A-Z / a-z
        if !bytes[0].is_ascii_alphabetic() {
            return Err((
                ErrorCode::EBadDriveLetter,
                format!("盘符非法（必须是 A-Z）：{}", input),
            ));
        }
        // 3. Drive letter without immediate separator (e.g. `E:` or `E:work\a.txt`)
        if bytes.len() < 3 || !is_sep(bytes[2]) {
            return Err((
                ErrorCode::EDriveRelative,
                format!("盘符后必须紧跟 \\（不接受盘符相对路径）：{}", input),
            ));
        }
    } else {
        // 2. Does not start with drive letter (relative path, `.`, `..`, etc.)
        return Err((
            ErrorCode::ENotAbsolute,
            format!("只接受盘符绝对路径（如 E:\\...），拒绝相对路径：{}", input),
        ));
    }

    // Section 5.2: Total UTF-16 length limit
    let utf16_len = input.encode_utf16().count();
    if utf16_len > MAX_UTF16_PATH_LEN {
        return Err((
            ErrorCode::EPathTooLong,
            format!(
                "路径 UTF-16 长度 {} 超过上限 {}",
                utf16_len, MAX_UTF16_PATH_LEN
            ),
        ));
    }

    let drive_char = (bytes[0] as char).to_ascii_uppercase();
    let original_drive_char = bytes[0] as char;
    let rest = &input[3..];

    let mut clean_segments: Vec<&str> = Vec::new();
    for raw_seg in rest.split(|c| c == '\\' || c == '/') {
        if raw_seg.is_empty() {
            // Accepted: duplicate separators and trailing separator are collapsed (5.1)
            continue;
        }

        // Section 5.2: Per-segment UTF-16 length limit
        if raw_seg.encode_utf16().count() > MAX_UTF16_SEGMENT_LEN {
            return Err((
                ErrorCode::EPathTooLong,
                format!(
                    "路径段 `{}` 长度超过上限 {}",
                    raw_seg, MAX_UTF16_SEGMENT_LEN
                ),
            ));
        }

        // 7. `.` or `..` segment
        if raw_seg == "." || raw_seg == ".." {
            return Err((
                ErrorCode::EDotComponent,
                format!("路径包含歧义段 `{}`：{}", raw_seg, input),
            ));
        }

        // 8. Segment ending with `.` or space
        if raw_seg.ends_with('.') || raw_seg.ends_with(' ') {
            return Err((
                ErrorCode::ETrailingDotSpace,
                format!("路径段 `{}` 以点或空格结尾，存在 Windows 改写歧义", raw_seg),
            ));
        }

        // 9. Alternate data stream colon
        if raw_seg.contains(':') {
            return Err((
                ErrorCode::EAltDataStream,
                format!("路径段 `{}` 包含备用数据流冒号", raw_seg),
            ));
        }

        // 10. Wildcards
        if raw_seg.contains('*') || raw_seg.contains('?') {
            return Err((
                ErrorCode::EWildcard,
                format!("路径段 `{}` 包含通配符（本工具不做通配展开）", raw_seg),
            ));
        }

        // 11. 8.3 short name form
        if is_83_short_name(raw_seg) {
            return Err((
                ErrorCode::EShortName,
                format!("路径段 `{}` 匹配 8.3 短名形式，拒绝处理", raw_seg),
            ));
        }

        // Section 6.3.3: Windows reserved device names
        if is_reserved_device_name(raw_seg) {
            return Err((
                ErrorCode::EReservedName,
                format!("路径段 `{}` 是 Windows 保留设备名", raw_seg),
            ));
        }

        clean_segments.push(raw_seg);
    }

    let is_root = clean_segments.is_empty();
    let normalized = if is_root {
        format!("{}:\\", original_drive_char)
    } else {
        format!("{}:\\{}", original_drive_char, clean_segments.join("\\"))
    };
    let compare_key = if is_root {
        format!("{}:\\", drive_char.to_ascii_lowercase())
    } else {
        format!(
            "{}:\\{}",
            drive_char.to_ascii_lowercase(),
            clean_segments.join("\\").to_ascii_lowercase()
        )
    };

    Ok(NormalizedPath {
        normalized,
        compare_key,
        drive_letter: drive_char,
        is_root,
    })
}

fn is_sep(b: u8) -> bool {
    b == b'\\' || b == b'/'
}

/// Detects Windows 8.3 short names such as `PROGRA~1`, `EUCLI_~1`, `LONGFI~1.TXT`.
pub fn is_83_short_name(seg: &str) -> bool {
    if !seg.is_ascii() {
        return false;
    }
    let (stem, ext) = match seg.rfind('.') {
        Some(idx) => (&seg[..idx], Some(&seg[idx + 1..])),
        None => (seg, None),
    };
    if stem.is_empty() || stem.len() > 8 {
        return false;
    }
    if let Some(e) = ext {
        if e.is_empty() || e.len() > 3 || e.contains('.') {
            return false;
        }
    }
    if let Some(tilde_pos) = stem.rfind('~') {
        let prefix = &stem[..tilde_pos];
        let digits = &stem[tilde_pos + 1..];
        if !prefix.is_empty()
            && !digits.is_empty()
            && digits.bytes().all(|b| b.is_ascii_digit())
            && digits.bytes().any(|b| b != b'0')
        {
            return true;
        }
    }
    false
}

/// Detects Windows reserved device names: CON, PRN, AUX, NUL, COM1-COM9, LPT1-LPT9 (with or without extension).
pub fn is_reserved_device_name(seg: &str) -> bool {
    let base = seg.split('.').next().unwrap_or(seg);
    let upper = base.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}
