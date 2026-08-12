# Yuumi

Yuumi 是一款轻量的英雄联盟客户端辅助工具，通过 LCU API 实现战绩查询、对局分析、自动化操作等功能。

## 功能

- **战绩查询** — 搜索任意召唤师，查看对局历史与详情（支持 10 人数据、经典模式、海克斯大乱斗/经典海斗战绩及海克斯强化解析）
- **生涯** — 当前召唤师的近期战绩概览与资源信息
- **对局信息** — 实时显示当前对局 10 人段位、近期 KDA、阵营组队分析及历史同场玩家标记徽章
- **路人集 (曾同局玩家)** — 自动记录每局同场相遇玩家与频次，支持自定义标签/备注标记（如黑名单、大神等）、标记置顶与筛选、选人/对局聊天室自动提醒己方标记玩家，支持 JSON 导入导出与自动身份回填
- **TFT (云顶之弈)** — 云顶之弈战绩解析与详情弹窗、段位近况、OP.GG 热门阵容推荐与 4x7 大网格站位图（核心 C 位/装备优先级/代码一键复制）、海克斯强化百科（增量渲染、吸顶布局、LCU 本地 API + CDragon 兜底 + HTTP 代理/Gzip 压缩）
- **大乱斗悬浮窗** — ARAM 板凳席悬浮交互，支持 15s 保护期提醒、自动摇号与换回英雄
- **战利品管理** — 战利品库存查看（优先中文名与本地 LCU 图标）、一键智能批量开箱、碎片分解、皮肤/英雄重铸与精粹资源统筹
- **自动游戏流程 (Auto Gameflow)** — 包含自动选人/禁人/召唤师技能配置、自动接受匹配（可设延迟）、自动接受邀请、游戏结束自动荣誉点赞、延迟再来一局、ARAM 自动报边与断线自动重连
- **自动创建大厅** — 空闲时自动创建指定模式房间
- **多杀自动截图** — 游戏内实时检测多杀事件（连杀数可配置），自动截图保存至本地并支持自定义目录与一键打开截图文件夹
- **OP.GG & 资源网络** — 内置 OP.GG 英雄/云顶数据查询，支持通用 HTTP 代理加速与资源无缝代理
- **对局上传** — 游戏结束后自动批量上传战绩（支持智能拆分、上传失败落盘暂存与触发式重试 需接口支持）

## 技术栈

| 层级 | 技术                                         |
| ---- | -------------------------------------------- |
| 前端 | Vue 3 + TypeScript + Vite + Pinia + Naive UI |
| 后端 | Tauri v2 (Rust)                              |
| 通信 | LCU HTTPS + WebSocket (tokio-tungstenite)    |
| 打包 | Tauri bundler                                |

## 开发

### 环境要求

- [Node.js](https://nodejs.org/) >= 18
- [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/tools/install) >= 1.77
- Tauri v2 系统依赖（见 [Tauri 官方文档](https://v2.tauri.app/start/prerequisites/)）

### 启动与代码规范自测

```bash
pnpm install        # 安装依赖
pnpm tauri dev      # 开发模式（Vite + Tauri 窗口）
pnpm tauri build    # 构建生产包

# 代码格式化与规范自测（提交前建议自测）
pnpm run format     # 自动修复并对齐 Rust 后端代码格式
pnpm run check-all  # 执行完整自测（包含类型检查、格式校验及 clippy 静态分析，集成于 git pre-commit 钩子）
```

## 项目结构

```
Yuumi/
├── src/                  # Vue 前端
│   ├── views/            # 页面组件（Home/Career/Search/GameInfo/SavedPlayers/TFT/Tools/Settings/BenchOverlay）
│   ├── components/       # 通用与功能子组件
│   │   ├── tft/          # 云顶之弈模块（段位卡片/战绩卡片/详情弹窗/OP.GG阵容推荐/海克斯百科）
│   │   ├── gameinfo/     # 对局信息模块（玩家卡片/战绩列/对局阶段）
│   │   ├── career/       # 生涯战绩模块（召唤师信息/战绩页签）
│   │   ├── tools/        # 工具箱模块（自定义房间/符文配置/自动流程卡片/观战/战利品批量处理）
│   │   ├── layout/       # 布局与自定义标题栏
│   │   ├── NoticePopup.vue  # 更新日志弹窗
│   │   ├── UpdateDialog.vue # 自动更新弹窗（下载/安装新版本）
│   │   └── LcuOfflineState.vue # 统一 LCU 离线未连接状态
│   ├── composables/      # 组合式 hooks（useLcuAsset/useToast/usePlayerSearch/useTftData/useLoot/useAutoSaveConfig 等）
│   ├── api/              # LCU API 封装 + Tauri IPC 调用
│   ├── store/            # Pinia 全局状态
│   ├── utils/            # 工具函数（主题色等）
│   ├── i18n.ts           # 多语言配置
│   ├── main.ts           # 主窗口入口
│   └── opgg.ts           # OP.GG 独立窗口入口
├── src-tauri/            # Rust 后端
│   ├── src/
│   │   ├── main.rs       # 程序入口：日志初始化、rustls CryptoProvider 配置
│   │   ├── lib.rs        # 入口：AppState、命令注册、系统托盘
│   │   ├── logging.rs    # 日志初始化
│   │   ├── config.rs     # 配置读写与 Schema 无缝迁移
│   │   ├── saved_players.rs # 路人集 (曾同局玩家 SQLite 持久化与标记)
│   │   ├── lcu/          # LCU 连接（进程监控、HTTPS 代理、WebSocket、OP.GG Client、SGP并发限流、游戏数据资源映射）
│   │   ├── parsers/      # 数据清洗（召唤师/战绩/对局信息/TFT云顶之弈）
│   │   ├── agents/       # 自动化任务（BP/自动接受/自动重连/自动荣誉/接受邀请/多杀截图）
│   │   ├── commands/     # Tauri 命令实现（配置、LCU、工具等）
│   │   ├── loot.rs       # 战利品系统（智能批量开箱、分解、重铸、解锁）
│   │   ├── upload.rs     # 对局上传队列（包含失败暂存与对局触发式重试）
│   │   ├── updater.rs    # 自动更新与更新日志缓存模块
│   │   ├── tools.rs      # 杂项工具（自定义房间/符文/OP.GG数据抓取/观战）
│   │   └── signalr.rs    # SignalR Hub 远程反代
│   └── tauri.conf.json   # Tauri 配置
└── File/                 # 重构参考文档（不入库）
```

## 配置

配置文件位于 `%APPDATA%/Yuumi/config.json`，支持：

| 分类            | 内容                                                                                |
| --------------- | ----------------------------------------------------------------------------------- |
| General         | 客户端路径（含 WeGame）、启动选项、HTTP 代理、日志、截图保存目录、上传 API、SignalR |
| Personalization | 主题色、语言、胜/败/重赛卡片颜色、侧边栏菜单显示/隐藏                               |
| Functions       | 自动化流程开关（BP/匹配/重连/荣誉/邀请/上传/多杀截图等）+ 候选列表                  |
| Other           | 公告记录、搜索历史                                                                  |

## 致谢

- [Seraphine](https://github.com/Zzaphkiel/Seraphine) — 原版 Python 实现
- [LCU API](https://www.hextechdocs.dev/lol/lcuapi) — 英雄联盟客户端 API 文档

## 许可

仅供学习交流使用。
