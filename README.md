# Yuumi

Yuumi 是一款基于 **GPUI** 纯原生 Rust 构建的轻量级高性能英雄联盟客户端辅助工具，通过 LCU API 实现战绩查询、对局分析、自动化操作等功能。

> **Tags / 关键词**: `rustyuumi` `rust-yuumi` `yuumi` `lol` `league-of-legends` `lcu` `gpui` `rust`

## 功能

- **战绩查询与对局回放** — 搜索任意召唤师，查看对局历史与详情（支持 10 人数据、经典模式、海克斯大乱斗战绩及海克斯强化解析），支持一键下载对局 ROFL 回放与调用客户端启动观看
- **生涯与英雄熟练度** — 当前召唤师近期战绩概览、资产统计、英雄熟练度 2.0 追踪（里程碑评级、成就积分、等级排行）与挑战勋章自由装配配置
- **对局信息与宿命对局** — 实时显示当前对局 10 人段位、近期 KDA、阵营组队分析；支持历史多局深度交手回溯检测、多次相遇统计及徽标优先级互斥展示（曾友/曾敌）
- **路人集 (曾同局玩家)** — 自动记录每局同场相遇玩家与频次，支持自定义标签/备注标记（如黑名单、绝活哥等）、标记置顶与筛选、选人/对局聊天室自动提醒己方标记玩家，支持 JSON 导入导出与自动身份回填
- **实用工具箱** — 包含基于战绩的好友清理（智能筛选长期未开黑/未上线好友并批量删除）、OP.GG 符文与出装方案一键导入装备页、直接观战指定玩家、自定义 5v5 房间创建、在线状态与生涯背景设置、客户端缩放与黑边修复、游戏设置防篡改锁定等
- **TFT (云顶之弈)** — 云顶之弈战绩解析与详情弹窗、段位近况、OP.GG 热门阵容推荐与 4x7 大网格站位图（核心 C 位/装备优先级/代码一键复制）、海克斯强化百科（增量渲染、吸顶布局、LCU 本地 API + CDragon 兜底 + HTTP 代理/Gzip 压缩）
- **大乱斗悬浮窗** — ARAM 板凳席悬浮交互，支持 15s 保护期提醒、自动摇号与换回英雄
- **战利品管理** — 战利品库存查看（优先中文名与本地 LCU 图标）、一键智能批量开箱、碎片分解、皮肤/英雄重铸与精粹资源统筹
- **自动游戏流程 (Auto Gameflow)** — 包含自动选人/禁人/召唤师技能配置、自动接受匹配（可设延迟）、自动接受邀请、游戏结束自动荣誉点赞、延迟再来一局、ARAM 自动报边与断线自动重连
- **多杀自动截图** — 游戏内实时检测多杀事件（连杀数可配置），自动截图保存至本地并支持自定义目录与一键打开截图文件夹
- **OP.GG & 资源网络** — 内置 OP.GG 英雄/云顶数据查询，支持通用 HTTP 代理加速与资源无缝代理
- **对局上传** — 游戏结束后自动批量上传战绩（支持智能拆分、上传失败落盘暂存与触发式重试 需接口支持）

## 技术栈

| 层级 | 技术 | 说明 |
| ---- | ---- | ---- |
| UI 渲染 | GPUI (`crates/yuumi-gpui`) | 基于 Direct3D 11 / DirectWrite 硬件加速渲染，亚像素抗锯齿与低延迟交互 |
| 核心业务 | Rust (`crates/yuumi-core`) | 独立纯逻辑库，涵盖 LCU 通信、数据解析、自动化 Agent、本地存储等 |
| 异步运行时 | Tokio (multi-thread) | 多线程异步网络、WebSocket 事件流与后台任务调度 |
| 图像管线 | `image` + 异步并发预热 | 后台线程池 BGRA `RenderImage` 解码与多级内存缓存，UI 零卡顿 |
| 本地存储 | SQLite (`rusqlite bundled`) | 路人集同场相遇历史与标签数据本地持久化 |
| 安装与分发 | NSIS + 便携绿色版 | 标准安装包 (`Yuumi_<version>_x64-setup.exe`) 与绿色便携包 (`Yuumi_v<version>_portable.zip`) |

## 开发

### 环境要求

- [Rust](https://www.rust-lang.org/tools/install) >= 1.77（推荐最新 stable 工具链，包含 `cargo`、`rustfmt` 与 `clippy`）
- Windows 10 / 11（支持 Direct3D 11 硬件加速渲染）
- C++ 生成工具（MSVC 工具链）
- 可选：[Node.js](https://nodejs.org/)（仅用于运行 `scripts/capture-lcu-fixtures.mjs` 采集真实 LCU 响应测试快照）

### 启动与代码规范自测

```bash
# 开发模式运行（依赖项自动以 opt-level=2 编译，确保开发调试下 GPUI 流畅帧率）
cargo run -p yuumi-gpui
# 或
cargo run --bin yuumi

# 构建生产发布版本（生成产物位于 target/release/yuumi.exe）
cargo build --release -p yuumi-gpui

# 代码格式化与规范自测（提交前建议自测）
cargo fmt --all                        # 自动修复并对齐所有 Rust 代码格式
cargo fmt --all -- --check             # 检查代码格式是否合规
cargo clippy --workspace --all-targets # Clippy 静态代码检查
cargo test --workspace                 # 运行全工作区单元测试（含 140+ 真实/合成 fixture 解析测试）

# 抓取真实 LCU 测试快照（可选，需客户端处于运行状态）
node scripts/capture-lcu-fixtures.mjs
```

> **Git 钩子提示**: 仓库集成了 `.githooks/pre-commit`，在 `git commit` 时会自动对暂存的 `.rs` 文件执行 `rustfmt` 并运行 `cargo clippy --workspace --all-targets` 质量门禁。

## 项目结构

<details>
<summary>展开查看完整目录树</summary>

```
Yuumi/
├── Cargo.toml                    # 顶层 Workspace 配置（yuumi-core + yuumi-gpui）
├── crates/
│   ├── yuumi-core/               # 核心业务逻辑与 LCU 客户端库
│   │   ├── src/
│   │   │   ├── agents/           # 自动化 Agent（Auto BP、Auto Match、生命周期管控与 Supervisor）
│   │   │   ├── config.rs         # 配置读写与 Schema 兼容迁移（%APPDATA%/Yuumi/config.json）
│   │   │   ├── gameflow/         # 对局数据流（阵营判定、身份匹配、战绩聚合）
│   │   │   ├── lcu/              # LCU 交互层（进程探测、HTTPS 客户端、WebSocket、事件投影、资产缓存）
│   │   │   ├── parsers/          # 核心解析器（战绩、召唤师、对局信息、TFT、战利品、队友分析等）
│   │   │   ├── runtime.rs        # 运行时模式检测（便携版/安装版、全局 Tokio 句柄注册）
│   │   │   ├── storage/          # 本地存储（路人集 SQLite 持久化）
│   │   │   ├── update.rs         # 自动更新校验与便携版更新助手
│   │   │   ├── upload.rs         # 对局战绩上传队列
│   │   │   └── signalr.rs        # SignalR Hub 远程反代
│   │   └── testdata/             # 真实与合成的 LCU 响应测试快照（fixture）
│   └── yuumi-gpui/               # GPUI 原生桌面 UI 应用（主二进制产物：yuumi.exe）
│       ├── src/
│       │   ├── main.rs           # 应用入口：单实例互斥、Tokio 初始化、GPUI Window 启动
│       │   ├── app_window/       # 主窗口各路由与生命周期管理（shell、career、game_info 等）
│       │   ├── views/            # 各业务视图组件（Home、Career、Search、GameInfo、SavedPlayers、TFT、Tools、Settings）
│       │   ├── components/       # 基础 UI 组件（按钮、卡片、输入框、下拉菜单、开关、弹窗、Toast 等）
│       │   ├── services/         # 界面后端集成服务（LiveLcuService 实时数据源、多杀截图、日志系统等）
│       │   ├── opgg/             # OP.GG 独立弹窗与出装/符文/英雄梯度面板
│       │   ├── bench_overlay.rs  # 大乱斗板凳席独立置顶悬浮窗
│       │   ├── asset_loader.rs   # 图片资源异步加载引擎与 BGRA RenderImage 解码
│       │   ├── assets.rs         # 内嵌矢量图标 (SVG) 与静态资产定义
│       │   ├── theme.rs          # 颜色与主题系统（亮暗模式适配）
│       │   ├── tray.rs           # 系统托盘集成（主页/生涯/战绩/对局/TFT/工具/设置/退出）
│       │   └── platform/         # Windows 原生平台特性（Win32 窗口属性、无边框阴影、Mica 效果）
│       └── assets/               # 静态图标 (SVG) 与内置图片资产
├── .githooks/                    # 纯 Git 钩子（pre-commit 自动触发 rustfmt 与 clippy）
├── scripts/                      # 辅助工具脚本（LCU 真实 fixture 抓取等）
└── File/                         # 参考文档
```

</details>

## 配置

配置文件位于 `%APPDATA%/Yuumi/config.json`（便携版位于 `data/config.json`），支持：

| 分类            | 内容                                                                                |
| --------------- | ----------------------------------------------------------------------------------- |
| General         | 客户端路径（含 WeGame）、启动选项、HTTP 代理、日志级别、上传 API、SignalR Hub       |
| Personalization | 主题模式（浅色/深色/跟随系统）、主题色、DPI 缩放、胜/负/重赛卡片颜色、Mica 效果     |
| Functions       | 自动化流程开关（BP/匹配/重连/荣誉/邀请/上传/多杀截图等）+ 候选列表与延迟设置        |
| Other           | 公告记录、搜索历史                                                                  |

## 便携版

每次发版同时发布 `*_portable.zip`（绿色版，解压即用）：

- 配置文件/数据库/缓存数据均隔离到 `data/` 目录，可整体拷贝迁移，与安装版互不干扰
- **支持自动更新**；也可到 [GitHub Releases](https://github.com/ISuuuu/Yuumi/releases/latest) 手动下载最新版

## 致谢

- [Seraphine](https://github.com/Zzaphkiel/Seraphine) — 原版 Python 实现
- [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) — 高性能 GPU 加速 UI 框架
- [LCU API](https://www.hextechdocs.dev/lol/lcuapi) — 英雄联盟客户端 API 文档

## 许可

仅供学习交流使用。
