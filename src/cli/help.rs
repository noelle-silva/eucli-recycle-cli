pub fn main_help() -> String {
    format!(
        "\
eucli-recycle {version} — Windows 系统回收站命令行入口

【能做什么】
  put      把文件或目录安全送进系统回收站
  list     只读查看系统回收站里的条目与条目标识
  restore  按条目标识将回收站里的条目放回原处
  help     查看主帮助或各子命令帮助

用法:
  eucli-recycle <命令> [选项] [目标...]
  eucli-recycle --help
  eucli-recycle --version
  eucli-recycle help <put|list|restore>

【永远不会做什么】
  - 永不永久删除任何文件或目录（代码中不存在永久删除路径）
  - 永不清空回收站（不存在 empty/clear/purge 等任何清空入口）
  - 不管理、不检查、不提示回收站容量
  - 不弹窗、不提问、不读取标准输入，适合脚本与无人值守场景
  - 不兼容 rm 的参数风格（-f、-rf、-i、-v 等一律直接报错）

【示例】
  eucli-recycle put E:\\work\\temp.log
  eucli-recycle put --dir E:\\work\\build
  eucli-recycle put --dry-run --format json E:\\work\\temp.log
  eucli-recycle list --from E:\\work --limit 20
  eucli-recycle restore 8f3a1c2e90ab12cd

【可能遇到的错误与该怎么做】
  E_NOT_ABSOLUTE      目标不是盘符绝对路径 → 请传入完整绝对路径（如 E:\\work\\a.txt）
  E_NEEDS_DIR_FLAG    目标是目录但未声明 → 确认要回收目录时显式加上 --dir
  E_RECYCLE_REFUSED   回收站拒绝接收（如超长路径）→ 文件仍在原处，请缩短路径或人工处理
  E_BLACKLIST_*       命中系统或配置保护规则 → 目标受保护；如需调整请检查配置文件
  E_IN_USE            文件被其他程序占用 → 关闭占用该文件的程序后重试
  E_ORIGIN_OCCUPIED   恢复时原位置已有同名对象 → 请先移走原位置对象后再执行 restore
",
        version = env!("CARGO_PKG_VERSION")
    )
}

pub fn put_help() -> String {
    "\
eucli-recycle put — 把文件或目录送进系统回收站

【能做什么】
  校验全部绝对路径与保护规则后，将目标逐个送进系统回收站并核实结果。

用法:
  eucli-recycle put [选项] <绝对路径> [<绝对路径> ...]

选项:
  --dir                      显式声明目标是目录（删除普通目录时必填）
  --format <human|json>      输出格式，默认 human
  --config <路径>            指定黑名单配置文件（默认 %APPDATA%\\eucli-recycle\\config.toml）
  --dry-run                  只做检查不执行，输出 planned / blocked
  --help                     显示本命令帮助

【永远不会做什么】
  - 永不永久删除，目标进不了回收站时原样留存并报错退出
  - 永不清空回收站，不弹窗确认，不接受相对路径与 rm 风格参数

【示例】
  eucli-recycle put E:\\work\\old.txt
  eucli-recycle put --dir E:\\work\\old_dir
  eucli-recycle put --dry-run --format json E:\\work\\old.txt

【可能遇到的错误与该怎么做】
  E_NEEDS_DIR_FLAG     目标是目录但未加 --dir → 加上 --dir 重试
  E_DIR_FLAG_ON_FILE   目标是文件或链接却加了 --dir → 去掉 --dir 重试
  E_CWD_ITSELF         目标是当前工作目录 → 先切换到其他目录
  E_RECYCLE_REFUSED    回收站拒绝接收 → 文件留在原处，请排查路径长度或权限
"
    .to_string()
}

pub fn list_help() -> String {
    "\
eucli-recycle list — 查看系统回收站里的条目

【能做什么】
  只读枚举系统回收站，输出条目标识（trash_id）、文件名、原路径、删除时间与类型。

用法:
  eucli-recycle list [选项]

选项:
  --from <绝对路径>          只显示原路径位于该目录下的条目
  --name <子串>              按文件名子串筛选（区分大小写，不做通配）
  --after <时间>             只显示该时间及之后删除的条目（YYYY-MM-DD 或 YYYY-MM-DDTHH:MM:SS）
  --before <时间>            只显示该时间及之前删除的条目（YYYY-MM-DD 或 YYYY-MM-DDTHH:MM:SS）
  --limit <N>                最多返回 N 条
  --offset <N>               跳过前 N 条
  --format <human|json>      输出格式，默认 human
  --help                     显示本命令帮助

【永远不会做什么】
  - 永不永久删除，永不清空回收站，不修改回收站内容，不判断回收站容量

【示例】
  eucli-recycle list --limit 20
  eucli-recycle list --from E:\\work --name report --format json

【可能遇到的错误与该怎么做】
  E_NOT_ABSOLUTE      --from 不是绝对路径 → 传入盘符绝对路径（如 E:\\work）
  E_UNKNOWN_ARG       参数拼写错误或值非法 → 检查时间格式与数值参数
"
    .to_string()
}

pub fn restore_help() -> String {
    "\
eucli-recycle restore — 把回收站里的条目放回原处

【能做什么】
  按 list 给出的条目标识（trash_id）唯一匹配并搬回原路径，随后核实原位出现且回收站内已移除。

用法:
  eucli-recycle restore [选项] <条目标识> [<条目标识> ...]

选项:
  --format <human|json>      输出格式，默认 human
  --config <路径>            指定黑名单配置文件（用于校验原路径保护规则）
  --help                     显示本命令帮助

【永远不会做什么】
  - 永不永久删除，永不清空回收站
  - 不按文件名猜测，不自动重建已删除的父目录，绝不覆盖原位置已有对象

【示例】
  eucli-recycle restore 8f3a1c2e90ab12cd
  eucli-recycle restore --format json 8f3a1c2e90ab12cd

【可能遇到的错误与该怎么做】
  E_TRASH_ID_NOT_FOUND   回收站中找不到该标识 → 运行 list 获取最新条目标识
  E_ORIGIN_DIR_MISSING   原父目录已不存在 → 先手动创建该目录再恢复
  E_ORIGIN_OCCUPIED      原位置已有同名文件或目录 → 先移走占位对象再恢复
"
    .to_string()
}
