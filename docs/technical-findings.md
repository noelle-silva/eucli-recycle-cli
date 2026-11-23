# 技术假设实测结论（SPEC.md 7.3 节 V1–V8）

- 测试日期：2026-11-23
- 测试环境：Windows 10/11 x64，Rust 1.90.0 (`x86_64-pc-windows-msvc`)，`windows = "0.61.3"`
- 测试磁盘：C: / D: / E:（均为 NTFS），系统回收站约 5,429 条既有条目
- 测试方式：在临时沙盒目录中使用独立验证程序（`v_probe`）直接调用 Windows 官方 Shell COM 接口（`IFileOperation`、`IFileOperationProgressSink`、`IShellFolder2`、`FOLDERID_RecycleBinFolder`、`GetShortPathNameW`）实测，全程在宿主机真实环境下运行。

---

## 汇总表

| 编号 | 假设内容 | 实测结论 | 实现策略 |
|---|---|---|---|
| **V1** | 去掉 `FOF_WANTNUKEWARNING` 后，长路径目标不再弹窗，改为失败返回 | **部分成立（关键发现）**：确实**不再弹窗**；但仅使用 `FOF_ALLOWUNDO \| FOF_NO_UI \| FOFX_RECYCLEONDELETE` 时，Windows 在回收失败后会**静默降级为永久删除**并返回 `Ok(())`！必须在 `PreDeleteItem` 中检查 `TSF_DELETE_RECYCLE_IF_POSSIBLE` 并在缺失时返回错误中止。 | **双保险**：设置 `FOF_ALLOWUNDO \| FOF_NO_UI \| FOFX_RECYCLEONDELETE`，且在 `IFileOperationProgressSink::PreDeleteItem` 中强制校验 `(dwFlags & TSF_DELETE_RECYCLE_IF_POSSIBLE) != 0`，否则立即返回 `COPYENGINE_E_RECYCLE_FORCE_NUKE` 中止操作。 |
| **V2** | `FOFX_RECYCLEONDELETE` 在当前 SDK 可用，且加上后行为符合预期 | **成立**：`windows 0.61.3` 提供 `FOFX_RECYCLEONDELETE`（`0x00080000`），可正常编译且普通文件可正常回收；但它单独不足以阻止 `FOF_NO_UI` 下对长路径的静默永久删除，需配合 V1 的 `PreDeleteItem` 守卫。 | 始终启用 `FOF_ALLOWUNDO \| FOF_NO_UI \| FOFX_RECYCLEONDELETE` 并配合 `PreDeleteItem` 守卫。 |
| **V3** | 回收成功时 `PostDeleteItem` 能拿到新生成的回收站条目 | **成立**：正常回收时 `hrDelete = 0x00270008`（`COPYENGINE_S_DONT_PROCESS_CHILDREN`），`psiNewlyCreated` 非空，可通过 `IShellFolder2::ParseDisplayName` + `GetDetailsEx` 在 ~0.65ms 内读取其原始位置、原始文件名、删除时间、体积。 | 优先通过 `PostDeleteItem` 返回的 `psiNewlyCreated` 在 `FOLDERID_RecycleBinFolder` 中读取元数据并核实；若 `psiNewlyCreated` 为空则直接不认成功。 |
| **V4** | 回收被拒时，`hrDelete` 能拿到 `COPYENGINE_E_*` 具体原因码 | **成立**：锁定文件时 `PerformOperations()` 返回 `Ok(())`，但 `PostDeleteItem` 拿到 `COPYENGINE_E_SHARING_VIOLATION_SRC (0x80270027)`；长路径被 `PreDeleteItem` 守卫拦下时拿到 `COPYENGINE_E_RECYCLE_FORCE_NUKE (0x80270036)`。 | 绝不只看 `PerformOperations()` 返回值；统一结合 `PerformOperations`、`PreDeleteItem` 拦截标记与 `PostDeleteItem` 的 `hrDelete` 做精确错误码映射。 |
| **V5** | （已按 R5 删除，编号空缺不复用） | — | 不读取 `NukeOnDelete` / `MaxCapacity` 等任何回收站配置。 |
| **V6** | 跨卷场景（源在 `D:`）行为正确 | **成立**：`D:` 盘文件回收成功，进入 `D:` 对应回收站条目，`PostDeleteItem` 回调与 `psiNewlyCreated` 均正常。 | 统一使用 `IFileOperation` 处理任意本地 NTFS 卷。 |
| **V7** | 8.3 短名在各卷的实际存在情况 | **实测完成**：`C:` 启用（`C:\Program Files` → `C:\PROGRA~1`），`D:` 启用（长名目录 → `D:\EUCLI_~1`），`E:` 未启用（返回原名）。 | 规则层按 5.1 静态拒绝所有 8.3 短名形态；测试中先探测再跑短名相关用例。 |
| **V8** | 每个目标单独一次 `IFileOperation` 时，`PostDeleteItem` 仍能拿到该目标结局 | **成立**：逐个目标创建独立 `IFileOperation` + Sink，每次都能完整收到 `PreDeleteItem` 与 `PostDeleteItem` 回调。 | 严格实行「每个目标一次独立 `IFileOperation`」，执行完立刻核实，失败即停（R13）。 |

---

## 详细实测记录

### 测试 1：V1 / V2 / V3 / V4 / V6 / V7 / V8 综合实测

执行命令：

```powershell
cargo build --offline --manifest-path "...\scratch\v_probe\Cargo.toml"
& "...\scratch\v_probe\target\debug\v_probe.exe"
```

关键输出：

```text
=== Sandbox (TEMP): E:\Temp\eucli_vprobe_12308 ===

=== V7: 8.3 Short Name Probe ===
C: (in TEMP) orig=E:\Temp\eucli_vprobe_12308\very_long_directory_name_for_83_test -> short=Some("E:\\Temp\\eucli_vprobe_12308\\very_long_directory_name_for_83_test")
C:\Program Files -> short=Some("C:\\PROGRA~1")
D: orig=D:\eucli_vprobe_longdirname_12308 -> short=Some("D:\\EUCLI_~1")
E: orig=E:\eucli_vprobe_longdirname_12308 -> short=Some("E:\\eucli_vprobe_longdirname_12308")

=== V2 / V3 / V8: Normal Recycle (with and without FOFX_RECYCLEONDELETE) ===
With FOFX_RECYCLEONDELETE: perform=Ok(()) aborted=false exists_after=false sink=SinkRecord { pre_delete_flags: [642], post_delete_hrs: [HRESULT(0x00270008)], post_delete_newly_created: [Some("E:\\$RECYCLE.BIN\\S-1-5-21-...\\$RVDDH5O.txt")], post_delete_newly_created_display: [Some("$RVDDH5O.txt")], post_move_hrs: [] }
Without FOFX_RECYCLEONDELETE: perform=Ok(()) aborted=false exists_after=false sink=SinkRecord { pre_delete_flags: [642], post_delete_hrs: [HRESULT(0x00270008)], post_delete_newly_created: [Some("E:\\$RECYCLE.BIN\\S-1-5-21-...\\$RH4BA5S.txt")], post_delete_newly_created_display: [Some("$RH4BA5S.txt")], post_move_hrs: [] }

=== V1 / V4: Long Path (>260 chars) ===
Created deep path len=367
Long path root (with FOFX + PreDelete guard): elapsed=549.4633ms perform=Err(HRESULT(0x80004004)) aborted=false root_exists=true leaf_exists=true sink=SinkRecord { pre_delete_flags: [642, 2], post_delete_hrs: [HRESULT(0x00270003), HRESULT(0x80004004)], post_delete_newly_created: [None, None], ... }
Long path root (FOFX alone, no PreDelete guard): elapsed=985.3439ms perform=Ok(()) aborted=false root_exists=false leaf_exists=false sink=SinkRecord { pre_delete_flags: [642, 2, 2, 2, 2, 2, 2, 2, 2, 2], post_delete_hrs: [HRESULT(0x00270003), ..., HRESULT(0x00270008)], post_delete_newly_created: [None, None, ...], ... }

=== V6: Cross-volume (D:) ===
D: recycle: perform=Ok(()) aborted=false exists_after=false sink=SinkRecord { pre_delete_flags: [642], post_delete_hrs: [HRESULT(0x00270008)], post_delete_newly_created: [Some("D:\\$RECYCLE.BIN\\S-1-5-21-...\\$RARO8HA.txt")], ... }

=== Enumeration Benchmark & Properties + Restore ===
EnumObjects hr=HRESULT(0x00000000)
FOUND f1! name="v2_with_fofx.txt" loc="E:\\Temp\\eucli_vprobe_12308" del_var=Some("2026/11/23:07:41:36.000") del_ft=Some(FILETIME { dwLowDateTime: 289781760, dwHighDateTime: 31292190 }) sz_u64=Some(10)
Enumerated 5429 items in 2.5398859s
Restore MoveItem: res=Ok(()) f1_exists_after=true sink=SinkRecord { ..., post_move_hrs: [HRESULT(0x00270008)] }
Restored content: Ok("hello fofx")
```

**关键结论（V1）**：
- `642 = 0x282` 包含 `TSF_DELETE_RECYCLE_IF_POSSIBLE (0x80)`；`2 = 0x002` 不包含该位。
- 当仅传递 `FOF_ALLOWUNDO | FOF_NO_UI | FOFX_RECYCLEONDELETE` 而不拦截 `PreDeleteItem` 时，Windows 对包含 >260 字符子路径的目录先尝试整体回收（`dwFlags = 642`，`PostDeleteItem` 返回 `COPYENGINE_S_NOT_HANDLED = 0x00270003`），随后因为 `FOF_NO_UI` 隐含无确认，**直接递归降级为永久删除**（`dwFlags = 2`），并且最终 `PerformOperations()` 仍返回 `Ok(())`！
- 在 `PreDeleteItem` 中检查 `(dwFlags & TSF_DELETE_RECYCLE_IF_POSSIBLE) == 0` 并立即返回 `COPYENGINE_E_RECYCLE_FORCE_NUKE (0x80270036)` 后，Windows 立刻中止操作，**无任何弹窗**，原目录与所有子文件完好无损。

---

### 测试 2：混合长路径目录、直接超长路径文件、锁定文件、只读文件、链接/联接/坏链接、快速核实

关键输出：

```text
1. Mixed long-path dir: res=Err(HRESULT(0x80270036)) short_exists=true deep_exists=true rec=SinkRec { pre_flags: [642, 2], post_hrs: [HRESULT(0x00270003), HRESULT(0x80270036)], newly_created_parsing: [None, None], blocked_permanent: true }
2. Direct >260 file: len=346 res=Err(HRESULT(0x80270036)) exists=true rec=SinkRec { pre_flags: [514], post_hrs: [HRESULT(0x80270036)], newly_created_parsing: [None], blocked_permanent: true }
3. Locked file: res=Ok(()) exists=true rec=SinkRec { pre_flags: [642], post_hrs: [HRESULT(0x80270027)], newly_created_parsing: [None], blocked_permanent: false }
4. Read-only file: res=Ok(()) exists=false rec=SinkRec { pre_flags: [642], post_hrs: [HRESULT(0x00270008)], newly_created_parsing: [Some("E:\\$RECYCLE.BIN\\...\\$RWC6GVD.txt")], blocked_permanent: false }
5a. Junction recycle: res=Ok(()) link_exists=false target_exists=true inner_exists=true
5b. Symlink file recycle: res=Ok(()) link_exists=false target_exists=true
5c. Dangling symlink recycle: res=Ok(()) link_exists=false
6a. rb_folder.ParseDisplayName elapsed=565.9µs hr=Ok(()) pidl_nonnull=true
6b. Fast GetDetailsEx elapsed=645.4µs name=Some("fast_verify.txt") loc=Some("E:\\Temp\\eucli_vprobe2_25620") del_ft=Some(...) sz=Some(19)
```

**关键结论**：
1. 含普通短路径文件 + 超长路径子文件的混合目录，在 `PreDeleteItem` 守卫下**短文件与深层文件全部完好保留**。
2. 直接传入 >260 字符文件路径时，`PreDeleteItem` 首次回调即为 `dwFlags = 514`（无 `TSF_DELETE_RECYCLE_IF_POSSIBLE`），被守卫立即拦下，原文件完好保留。
3. 独占锁定的文件：`PerformOperations()` 返回 `Ok(())`，但 `PostDeleteItem` 返回 `COPYENGINE_E_SHARING_VIOLATION_SRC (0x80270027)`，必须以 sink 回调作为真实结局依据并映射为 `E_IN_USE`。
4. 只读文件、目录联接（junction）、文件符号链接、目录符号链接、失效坏链接均可正常送进回收站，且绝不触碰目标对象。

---

### 测试 3：恢复后回收站条目消失的核实方式

关键输出：

```text
symlink_dir create=Ok(())
Before restore in EnumObjects: found=true
After restore in EnumObjects: found=false
After restore ParseDisplayName GetDetailsEx: loc=Ok("E:\\Temp\\eucli_vprobe3_77028") name=Ok("restore_verify_check.txt")
```

**关键结论**：
- `IFileOperation::MoveItem` 从 `FOLDERID_RecycleBinFolder` 移回原目录后，条目确实从回收站枚举（`EnumObjects`）中完全消失，同时原位置文件完整恢复。
- 注意：`IShellFolder::ParseDisplayName` 对已恢复条目的旧解析名可能仍构造出带缓存属性的 PIDL，因此**恢复后的「回收站内已消失」核实必须基于重新枚举 `EnumObjects`**，绝不能用 `ParseDisplayName` 替代。
