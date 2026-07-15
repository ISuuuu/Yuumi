// ─── Tauri 命令薄封装层 ───
// 本模块仅包含对 Tauri invoke 暴露的轻量命令（文件选择、系统操作等）。
// 业务逻辑较重的命令请放在顶层模块（tools.rs、loot.rs 等），
// 命名上 commands::tools 负责系统级工具，顶层 tools 负责 LCU 业务逻辑。
pub mod config;
pub mod lcu;
pub mod tools;
