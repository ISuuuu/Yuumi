# Yuumi

Tauri v2 + Vue 3 + TypeScript 桌面应用。
原版 Python (Seraphine) 项目的 Rust 重构。

## 技术栈

- **前端**: Vue 3 + TypeScript + Vite + Pinia + Naive UI
- **后端**: Tauri v2 (Rust)
- **包管理**: pnpm

## 常用命令

```bash
pnpm tauri dev      # 开发（Vite + Tauri 窗口）
pnpm tauri build    # 构建生产包
pnpm dev            # 仅前端开发
pnpm build          # 仅前端构建
```

## 项目结构

```
Yuumi/
├── src/                            # Vue 前端
│   ├── App.vue                     # 根组件（自定义标题栏 + 导航栏 + 路由切换）
│   ├── main.ts                     # 入口
│   ├── api/
│   │   └── lcu.ts                  # LCU API 封装 + Rust 命令调用
│   ├── store/
│   │   └── lcuStore.ts             # Pinia 全局状态（LCU 事件映射）
│   ├── opgg.ts                     # OP.GG 弹窗/独立窗口逻辑
│   ├── opgg.ts                     # OP.GG 辅助逻辑
│   ├── utils/
│   │   └── theme.ts                # 主题色动态更新
│   ├── composables/
│   │   ├── useLcuAsset.ts          # LCU 资源路径 → data URL（缓存 + 去重）
│   │   └── useToast.ts             # Naive UI 消息提示 Hook (多窗口安全降级)
│   ├── assets/                     # 静态资源（图片等）
│   ├── views/
│   │   ├── Home.vue                # 首页（LCU 状态 + 快捷导航）
│   │   ├── Career.vue              # 生涯战绩（召唤师 + 对局历史）
│   │   ├── Search.vue              # 战绩查询（玩家搜索 + 对局列表）
│   │   ├── GameInfo.vue            # 对局信息（10 人段位 + KDA）
│   │   ├── TFT.vue                 # TFT 数据
│   │   ├── Settings.vue            # 设置（头像/签名/在线状态/配置）
│   │   └── Tools.vue               # 工具箱（创建房间/ARAM 摇号/符文/皮肤等）
│   └── components/
│       ├── OpggModal.vue           # OP.GG 数据弹窗
│       ├── OpggWindow.vue          # OP.GG 独立窗口组件
│       ├── LcuImage.vue            # LCU 资源图片组件（loading/error 状态）
│       ├── ChampionPicker.vue      # 英雄选择器（v-model: number[]）
│       ├── SpellPicker.vue         # 召唤师技能选择器（v-model: number[]）
│       └── NaiveUIBridge.vue       # Naive UI 全局 API 桥接组件
├── src-tauri/                      # Tauri/Rust 后端
│   ├── src/
│   │   ├── main.rs                 # Rust 入口
│   │   ├── lib.rs                  # AppState、命令注册、agent 启动、系统托盘
│   │   ├── config.rs               # 配置读写（%APPDATA%/Yuumi/config.json）
│   │   ├── tools.rs                # 杂项工具（创建房间/ARAM 摇号/符文/皮肤等）
│   │   ├── logging.rs              # 日志系统（flexi_logger，日志写入 exe 同级 log/ 目录）
│   │   ├── signalr.rs              # SignalR Hub 远程反代（条件启动）
│   │   ├── upload.rs               # 对局上传队列（单场/批量，Smart Split payload）
│   │   ├── lcu/
│   │   │   ├── mod.rs
│   │   │   ├── monitor.rs          # LCU 进程轮询（sysinfo + lockfile + WMIC 兜底）
│   │   │   ├── client.rs           # HTTPS 代理（忽略 SSL + Basic Auth）
│   │   │   ├── ws.rs               # WebSocket 事件订阅 → 广播前端 + 分发 agents（带取消机制）
│   │   │   └── game_data.rs        # 游戏资源预加载（物品/技能/符文/英雄 ID→名称/iconPath）
│   │   ├── parsers/
│   │   │   ├── mod.rs
│   │   │   ├── summoner.rs         # 召唤师数据清洗
│   │   │   ├── match_parser.rs     # 战绩数据清洗（parseGameData）
│   │   │   ├── game_info.rs        # 对局信息（10 人段位 + 近期 KDA）
│   │   │   └── tft.rs              # TFT 数据解析
│   │   └── agents/
│   │       ├── mod.rs
│   │       ├── auto_bp.rs          # 自动选人/禁人/召唤师技能
│   │       └── auto_match.rs       # 自动接受匹配/自动重连/对局结束触发上传
│   ├── tauri.conf.json
│   ├── capabilities/
│   └── Cargo.toml
├── File/                           # 重构参考文档（不入库）
│   ├── tauri_reconstruction_spec.md
│   └── tauri_reconstruction_context.md
├── AGENTS.md
├── package.json
├── vite.config.ts
└── tsconfig.json
```

## 架构数据流

```
LeagueClientUx.exe
  ↓ (sysinfo 轮询 port/token，支持 lockfile / 命令行 / WMIC 三种检测)
monitor.rs → AppState.lcu_client + AppState.game_data（预加载英雄/物品/技能/符文）
  ↓
ws.rs → LCU WebSocket 事件（带取消机制：新连接自动终止旧循环）
  ├─→ 前端: Tauri emit → lcuStore.ts → Vue 组件
  ├─→ BP Agent: mpsc → auto_bp.rs (自动选人/禁人/技能)
  ├─→ Match Agent: mpsc → auto_match.rs (自动接受/重连)
  └─→ Upload Trigger: 游戏结束 → UploadQueue → 外部 API
```

## 窗口与 UI

- **自定义标题栏**: 去掉原生装饰 (`decorations: false`)，自定义标题栏含最小化/最大化/关闭 + 返回导航 + 游戏阶段显示
- **系统托盘**: 完整托盘菜单（主页/生涯/战绩查询/对局信息/TFT/其他功能/设置/退出），点击托盘图标显示窗口，支持关闭到托盘
- **主题**: "纯白水晶极光" 风格，CSS 变量体系，毛玻璃效果，自定义滚动条
- **页面路由**: 手动 `currentPage` ref 状态切换，Search 和 GameInfo 用 `v-show` 保持状态
- **窗口启动居中**: `tauri.conf.json` 中 `"center": true`

## Rust Tauri 命令清单

| 命令 | 来源 | 说明 |
|:---|:---|:---|
| `greet` | lib.rs | 测试命令 |
| `get_config` | lib.rs | 读取完整 AppConfig |
| `update_config` | lib.rs | 写入完整 AppConfig |
| `get_close_to_tray` | lib.rs | 读取关闭到托盘设置 |
| `get_lcu_connection_info` | lib.rs | 获取 LCU PID/port/token |
| `get_game_data_assets` | lib.rs | 获取预加载的游戏资源映射 |
| `detect_lol_path` | lib.rs | 自动检测 LOL 客户端路径 |
| `select_lol_folder` | lib.rs | 打开原生文件夹选择对话框 |
| `call_lcu_api` | lcu/client.rs | 通用 LCU API 转发 |
| `get_lcu_asset` | lcu/client.rs | 获取单个 LCU 图片资源 |
| `get_current_summoner` | parsers/summoner.rs | 获取召唤师信息 |
| `get_match_history` | parsers/match_parser.rs | 获取战绩列表 |
| `get_game_player_summaries` | parsers/game_info.rs | 获取对局 10 人段位 + KDA |
| `get_tft_data` | parsers/tft.rs | 获取 TFT 数据 |
| `create_5v5_practice_lobby` | tools.rs | 创建自定义房间 |
| `aram_reroll_and_swap_back` | tools.rs | 大乱斗摇号换回 |
| `apply_rune_page` | tools.rs | 应用符文页 |
| `get_lcu_zoom` | tools.rs | 获取 LCU 窗口缩放 |
| `fix_lcu_window` | tools.rs | 修复 LCU 窗口位置 |
| `clear_game_cache` | tools.rs | 清除游戏缓存 |
| `open_log_folder` | tools.rs | 打开日志目录 |
| `fetch_opgg_data` | tools.rs | 获取 OP.GG 数据 |
| `get_champion_skins` | tools.rs | 获取英雄皮肤列表 |
| `get_game_settings_readonly` | tools.rs | 读取游戏设置 |
| `set_game_settings_readonly` | tools.rs | 写入游戏设置 |
| `upload_single_match` | upload.rs | 单场对局上传 |
| `batch_upload_matches` | upload.rs | 批量对局上传 |

新命令需在 `lib.rs` 的 `invoke_handler` 中注册。

## Agent 后台任务

| Agent | 触发事件 | 功能 |
|:---|:---|:---|
| auto_bp | `/lol-champ-select/v1/session` | 自动选人/禁人/设置召唤师技能 |
| auto_match | gameflow-phase + ready-check | 自动接受匹配/自动重连 + 游戏结束触发上传 |

## 上传系统 (upload.rs)

- **UploadQueue**: 去重异步队列，后台 Worker 串行上传，每局 30 秒超时
- **UploadTrigger**: 监听游戏阶段转换（InProgress → EndOfGame/Lobby），延迟 2 秒后自动上传
- **Smart Split**: 当前玩家数据放入 `matchInfo.participants`，其余 9 人放入外层 `participants`
- **批量上传**: 每批 10 场，连接失败时停止后续批次

## 配置文件

位于 `%APPDATA%/Yuumi/config.json`，结构：
- `General` — 启动、代理、日志、上传 API 地址、SignalR Hub 配置、客户端路径
- `Personalization` — 主题、语言、颜色
- `Functions` — 所有自动化功能开关和候选列表
- `Other` — 其他杂项

## 开发工具

开发模式下自动打开 DevTools（见 `lib.rs` 的 `setup` 闭包）。
日志写入 `<exe_dir>/log/` 目录，每天轮转，保留 30 天。
