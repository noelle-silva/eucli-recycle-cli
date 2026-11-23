use serde::Serialize;
use std::fmt;

/// Section 10.1 Error Codes — single authoritative source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum ErrorCode {
    // 参数与路径
    #[serde(rename = "E_OK")]
    EOk,
    #[serde(rename = "E_NO_ARGS")]
    ENoArgs,
    #[serde(rename = "E_UNKNOWN_ARG")]
    EUnknownArg,
    #[serde(rename = "E_EMPTY_PATH")]
    EEmptyPath,
    #[serde(rename = "E_NOT_ABSOLUTE")]
    ENotAbsolute,
    #[serde(rename = "E_DRIVE_RELATIVE")]
    EDriveRelative,
    #[serde(rename = "E_DEVICE_PATH")]
    EDevicePath,
    #[serde(rename = "E_UNC_UNSUPPORTED")]
    EUncUnsupported,
    #[serde(rename = "E_DOT_COMPONENT")]
    EDotComponent,
    #[serde(rename = "E_TRAILING_DOT_SPACE")]
    ETrailingDotSpace,
    #[serde(rename = "E_ALT_DATA_STREAM")]
    EAltDataStream,
    #[serde(rename = "E_WILDCARD")]
    EWildcard,
    #[serde(rename = "E_SHORT_NAME")]
    EShortName,
    #[serde(rename = "E_BAD_DRIVE_LETTER")]
    EBadDriveLetter,
    #[serde(rename = "E_RESERVED_NAME")]
    EReservedName,
    #[serde(rename = "E_PATH_TOO_LONG")]
    EPathTooLong,
    #[serde(rename = "E_DUPLICATE_TARGET")]
    EDuplicateTarget,
    #[serde(rename = "E_OVERLAPPING_TARGET")]
    EOverlappingTarget,
    #[serde(rename = "E_TOO_MANY_TARGETS")]
    ETooManyTargets,
    #[serde(rename = "E_DUPLICATE_TRASH_ID")]
    EDuplicateTrashId,

    // 规矩拦截 (blocked)
    #[serde(rename = "E_DRIVE_ROOT")]
    EDriveRoot,
    #[serde(rename = "E_BLACKLIST_SELF")]
    EBlacklistSelf,
    #[serde(rename = "E_BLACKLIST_TREE")]
    EBlacklistTree,
    #[serde(rename = "E_PROTECTED_PARENT")]
    EProtectedParent,
    #[serde(rename = "E_CWD_ITSELF")]
    ECwdItself,
    #[serde(rename = "E_CWD_ANCESTOR")]
    ECwdAncestor,
    #[serde(rename = "E_NEEDS_DIR_FLAG")]
    ENeedsDirFlag,
    #[serde(rename = "E_DIR_FLAG_ON_FILE")]
    EDirFlagOnFile,
    #[serde(rename = "E_NOT_FOUND")]
    ENotFound,

    // 执行与核实
    #[serde(rename = "E_ACCESS_DENIED")]
    EAccessDenied,
    #[serde(rename = "E_IN_USE")]
    EInUse,
    #[serde(rename = "E_READ_ONLY")]
    EReadOnly,
    #[serde(rename = "E_RECYCLE_REFUSED")]
    ERecycleRefused,
    #[serde(rename = "E_OPERATION_ABORTED")]
    EOperationAborted,
    #[serde(rename = "E_TARGET_CHANGED")]
    ETargetChanged,
    #[serde(rename = "E_UNKNOWN_SYSTEM")]
    EUnknownSystem,
    #[serde(rename = "E_VERIFY_FAILED")]
    EVerifyFailed,
    #[serde(rename = "E_STOPPED")]
    EStopped,

    // 恢复
    #[serde(rename = "E_TRASH_ID_NOT_FOUND")]
    ETrashIdNotFound,
    #[serde(rename = "E_TRASH_ID_AMBIGUOUS")]
    ETrashIdAmbiguous,
    #[serde(rename = "E_ORIGIN_DIR_MISSING")]
    EOriginDirMissing,
    #[serde(rename = "E_ORIGIN_OCCUPIED")]
    EOriginOccupied,
    #[serde(rename = "E_RESTORE_FAILED")]
    ERestoreFailed,

    // 配置
    #[serde(rename = "E_CONFIG_NOT_FOUND")]
    EConfigNotFound,
    #[serde(rename = "E_CONFIG_PARSE")]
    EConfigParse,
    #[serde(rename = "E_CONFIG_VALUE")]
    EConfigValue,
}

impl ErrorCode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::EOk => "E_OK",
            ErrorCode::ENoArgs => "E_NO_ARGS",
            ErrorCode::EUnknownArg => "E_UNKNOWN_ARG",
            ErrorCode::EEmptyPath => "E_EMPTY_PATH",
            ErrorCode::ENotAbsolute => "E_NOT_ABSOLUTE",
            ErrorCode::EDriveRelative => "E_DRIVE_RELATIVE",
            ErrorCode::EDevicePath => "E_DEVICE_PATH",
            ErrorCode::EUncUnsupported => "E_UNC_UNSUPPORTED",
            ErrorCode::EDotComponent => "E_DOT_COMPONENT",
            ErrorCode::ETrailingDotSpace => "E_TRAILING_DOT_SPACE",
            ErrorCode::EAltDataStream => "E_ALT_DATA_STREAM",
            ErrorCode::EWildcard => "E_WILDCARD",
            ErrorCode::EShortName => "E_SHORT_NAME",
            ErrorCode::EBadDriveLetter => "E_BAD_DRIVE_LETTER",
            ErrorCode::EReservedName => "E_RESERVED_NAME",
            ErrorCode::EPathTooLong => "E_PATH_TOO_LONG",
            ErrorCode::EDuplicateTarget => "E_DUPLICATE_TARGET",
            ErrorCode::EOverlappingTarget => "E_OVERLAPPING_TARGET",
            ErrorCode::ETooManyTargets => "E_TOO_MANY_TARGETS",
            ErrorCode::EDuplicateTrashId => "E_DUPLICATE_TRASH_ID",
            ErrorCode::EDriveRoot => "E_DRIVE_ROOT",
            ErrorCode::EBlacklistSelf => "E_BLACKLIST_SELF",
            ErrorCode::EBlacklistTree => "E_BLACKLIST_TREE",
            ErrorCode::EProtectedParent => "E_PROTECTED_PARENT",
            ErrorCode::ECwdItself => "E_CWD_ITSELF",
            ErrorCode::ECwdAncestor => "E_CWD_ANCESTOR",
            ErrorCode::ENeedsDirFlag => "E_NEEDS_DIR_FLAG",
            ErrorCode::EDirFlagOnFile => "E_DIR_FLAG_ON_FILE",
            ErrorCode::ENotFound => "E_NOT_FOUND",
            ErrorCode::EAccessDenied => "E_ACCESS_DENIED",
            ErrorCode::EInUse => "E_IN_USE",
            ErrorCode::EReadOnly => "E_READ_ONLY",
            ErrorCode::ERecycleRefused => "E_RECYCLE_REFUSED",
            ErrorCode::EOperationAborted => "E_OPERATION_ABORTED",
            ErrorCode::ETargetChanged => "E_TARGET_CHANGED",
            ErrorCode::EUnknownSystem => "E_UNKNOWN_SYSTEM",
            ErrorCode::EVerifyFailed => "E_VERIFY_FAILED",
            ErrorCode::EStopped => "E_STOPPED",
            ErrorCode::ETrashIdNotFound => "E_TRASH_ID_NOT_FOUND",
            ErrorCode::ETrashIdAmbiguous => "E_TRASH_ID_AMBIGUOUS",
            ErrorCode::EOriginDirMissing => "E_ORIGIN_DIR_MISSING",
            ErrorCode::EOriginOccupied => "E_ORIGIN_OCCUPIED",
            ErrorCode::ERestoreFailed => "E_RESTORE_FAILED",
            ErrorCode::EConfigNotFound => "E_CONFIG_NOT_FOUND",
            ErrorCode::EConfigParse => "E_CONFIG_PARSE",
            ErrorCode::EConfigValue => "E_CONFIG_VALUE",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
