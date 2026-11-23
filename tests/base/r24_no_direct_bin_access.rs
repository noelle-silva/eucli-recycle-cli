mod support;

use std::fs;
use support::probe::{all_src_rs_files, workspace_root};

#[test]
fn static_platform_layer_never_touches_recycle_bin_directory_directly() {
    let platform_dir = workspace_root().join("src").join("platform");
    for file in all_src_rs_files() {
        let content = fs::read_to_string(&file).expect("read src file");
        let lower = content.to_ascii_lowercase();

        // 1. Platform layer must NEVER reference $Recycle.Bin at all
        if file.starts_with(&platform_dir) {
            assert!(
                !lower.contains("$recycle.bin"),
                "R24 violation: `$Recycle.Bin` referenced in platform file {}",
                file.display()
            );
        }

        // 2. No source file anywhere may craft or parse $I / $R recycle bin files
        for pat in ["\"$i", "\"$r", "\\$i", "\\$r", "format!(\"$i", "format!(\"$r"] {
            assert!(
                !lower.contains(pat),
                "R24 violation: direct `$I`/`$R` recycle bin metadata access `{}` in {}",
                pat,
                file.display()
            );
        }
    }

    // 3. rules/blacklist.rs must be pure path matching with zero filesystem I/O
    let blacklist_rs = workspace_root().join("src").join("rules").join("blacklist.rs");
    if blacklist_rs.exists() {
        let content = fs::read_to_string(&blacklist_rs).expect("read blacklist.rs");
        for io_pat in ["File::open", "File::create", "fs::read", "fs::write", "CreateFileW"] {
            assert!(
                !content.contains(io_pat),
                "R24 violation: filesystem I/O `{}` inside blacklist.rs",
                io_pat
            );
        }
    }
}
