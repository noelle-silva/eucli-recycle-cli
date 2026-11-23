mod support;

use std::fs;
use support::probe::workspace_root;

#[test]
fn non_windows_platforms_are_explicitly_rejected_in_v1() {
    // 1. Verify platform/mod.rs gates Windows implementation under cfg(windows)
    //    and provides an explicit unsupported error for non-Windows platforms.
    let platform_mod = fs::read_to_string(
        workspace_root().join("src").join("platform").join("mod.rs"),
    )
    .expect("read src/platform/mod.rs");

    assert!(
        platform_mod.contains("cfg(windows)") && platform_mod.contains("cfg(not(windows))"),
        "platform/mod.rs must explicitly distinguish windows vs non-windows"
    );

    // 2. Verify unsupported platform error message contract
    let err = eucli_recycle::platform::unsupported_platform_error("linux");
    assert!(
        err.message.contains("本平台尚未支持"),
        "unsupported platform error must state 本平台尚未支持, got: {}",
        err.message
    );
    let err_mac = eucli_recycle::platform::unsupported_platform_error("macos");
    assert!(err_mac.message.contains("本平台尚未支持"));
}
