# eucli-recycle

Windows 系统回收站的命令行入口。只做三件事：`put`（送进回收站）、`list`（查看回收站）、`restore`（从回收站放回原处）。

> 完整设计规格与 24 条底层铁律见 [SPEC.md](SPEC.md)；Windows Shell COM 实测结论见 [docs/technical-findings.md](docs/technical-findings.md)。

## 核心定位

- **永不永久删除**：代码中不存在任何永久删除路径；目标进不了回收站时原样留存并报错退出。
- **永不清空回收站**：不提供 `empty` / `clear` / `purge` 等任何清空入口。
- **不管理回收站容量**：不检查满不满、不提示清理、不自动清理。
- **无弹窗、无交互**：不弹窗确认、不读取 stdin，适合脚本与自动化场景。
- **只收盘符绝对路径**：拒绝相对路径、`.`、`..`、通配符、8.3 短名、数据流、设备路径与 UNC 路径。
- **不兼容 `rm` 参数风格**：`-f`、`-rf`、`-i`、`-v` 等一律直接报错退出。

## 支持平台（v1）

- Windows 10 / 11 x64（Rust MSVC 工具链）

## 构建与测试

```powershell
# 编译发布版本
cargo build --release

# 运行全部 24 条铁律及 2 个跨切面基础约束测试
cargo test
```

## 用法示例

### 送进回收站 (`put`)

```powershell
# 送进普通文件
eucli-recycle put E:\work\temp.log

# 送进目录（必须显式声明 --dir）
eucli-recycle put --dir E:\work\build

# 仅检查不执行（dry-run）
eucli-recycle put --dry-run --format json E:\work\temp.log

# 指定自定义黑名单配置
eucli-recycle put --config E:\work\recycle.toml E:\work\temp.log
```

### 查看回收站 (`list`)

```powershell
# 查看最近 20 条
eucli-recycle list --limit 20

# 按原目录与文件名子串筛选并输出 JSON
eucli-recycle list --from E:\work --name report --format json

# 按删除时间筛选
eucli-recycle list --after 2026-11-23 --before 2026-11-23T23:59:59
```

### 恢复条目 (`restore`)

```powershell
# 按 list 输出的 16 位十六进制 trash_id 精确恢复（绝不覆盖已有文件）
eucli-recycle restore 8f3a1c2e90ab12cd

# JSON 格式输出
eucli-recycle restore --format json 8f3a1c2e90ab12cd
```

### 帮助与版本

```powershell
eucli-recycle --version
eucli-recycle --help
eucli-recycle help put
eucli-recycle help list
eucli-recycle help restore
```

## 配置文件

默认位置：`%APPDATA%\eucli-recycle\config.toml`（每次调用重新读取并校验，改完立即生效）。

```toml
schema = 1

# 只保护自身：对象本身不能扔，里面的内容可以扔
[protect.self]
paths = [
  "C:\\Users\\eucli",
]

# 保护整片：对象本身及里面所有内容都不能扔
[protect.tree]
paths = [
  "E:\\dev",
  "D:\\work\\important",
]
```

## 退出码

| 退出码 | 含义 |
|---|---|
| `0` | 全部成功（或 `--dry-run` 下全部 `planned`） |
| `1` | 用法、参数、配置错误或目标数超过 1000 |
| `2` | 被规矩拦下（未执行任何修改） |
| `3` | 执行阶段发生失败 |
| `4` | 结果无法确认（存在 `unknown` 且无 `failed`） |
