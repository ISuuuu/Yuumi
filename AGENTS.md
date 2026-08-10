# Yuumi

Tauri v2 + Vue 3 + TypeScript 桌面应用。
原版 Python (Seraphine) 项目的 Rust 重构。

## 编码指导原则 (Coding Guidelines)

**权衡：** 这些原则倾向于谨慎而非速度。对于微不足道的任务，请自行判断。

### 1. 编码前思考 (Think Before Coding)

**不要假设。不要隐瞒困惑。展现权衡。**

- **明确陈述你的假设**：如果不确定，请进行询问。
- **展现多种解释**：如果存在多种解释，请展示它们 —— 不要默默选择其中一种。
- **提倡更简单的方法**：如果有更简单的方法，请指出。在必要时进行反驳/建议。
- **停止并提问**：如果有不清楚的地方，请停下来，指出令人困惑的点，并进行询问。

### 2. 简洁第一 (Simplicity First)

**用最少的代码解决问题。不做任何投机性的编写。**

- **不要添加超出需求的功能**。
- **不要为单次使用的代码进行抽象**。
- **不要添加未要求的“灵活性”或“可配置性”**。
- **不要为不可能发生的情景编写错误处理**。
- **如果写了 200 行代码，而 50 行就能搞定，请重写**。
- **思考**：问问自己：“资深工程师会觉得这太复杂了吗？”如果是，请简化。

### 3. 外科手术式修改 (Surgical Changes)

**只改必须改动的。只清理你自己的烂摊子。**

- **不要“改进”相邻的代码、注释或格式**。
- **不要重构没有损坏/正常工作的代码**。
- **匹配现有的代码风格**，即使你有不同的习惯。
- **对于不相关的死代码，请提及 —— 不要直接删除它**。
- **清理引入的无用代码**：移除因**你的**修改而不再使用的 imports、变量或函数。
- **不要移除预先存在的死代码**，除非被明确要求。
- **检测标准**：修改的每一行都应该能直接追溯到用户的需求。

### 4. 目标驱动执行 (Goal-Driven Execution)

**定义成功标准。循环直到验证通过。**

将任务转化为可验证的目标：

- “添加校验” → “为无效输入编写测试，然后使其通过”
- “修复 Bug” → “编写复现该 Bug 的测试，然后使其通过”
- “重构 X” → “确保重构前后测试均能通过”
- **多步骤任务需陈述简短计划**：
  ```
  1. [步骤] → 验证: [检查项]
  2. [步骤] → 验证: [检查项]
  3. [步骤] → 验证: [检查项]
  ```
- **成功标准**：强大的成功标准能让你独立循环。弱标准（“使其工作”）需要不断澄清。

## Tauri v2 & Vue 3 编码规范 (Tauri & Vue Guidelines)

### Rust 后端 (Tauri v2 / Rust)

- **类型安全边界**：所有与前端交互的 Struct/Enum 必须实现 `serde::Serialize` 和 `serde::Deserialize`。
- **错误传播与序列化**：
  - `#[tauri::command]` 如果可能失败，必须返回 `Result<T, String>`。
  - 严禁随意使用 `unwrap()` 或 `panic!`，应使用 `map_err(|e| e.to_string())` 或 `thiserror` 将 Error 转化为前端友好的 String，并使用系统 `logging.rs` 的 logger 记录完整堆栈。
- **共享状态管理与死锁防护**：
  - 只能通过 `tauri::State<'_, AppState>` 访问全局状态，不得使用不安全的全局静态变量。
  - 在异步 Command 或后台 Task 中获取状态锁时，**严禁跨 `await` 点持有同步锁 (`std::sync::MutexGuard`)**，必须先释放锁或在作用域块分离后再 `await`，防止 Tokio 线程池死锁。
- **异步与非阻塞**：
  - 严禁在 Command 的主线程中执行耗时的 CPU 计算或 I/O 操作。
  - 使用 `tokio::spawn` 投递后台任务，并在执行完毕后通过 `tauri::Emitter::emit`（Tauri v2 API，禁止使用 v1 的 `emit_all`）异步通知前端。
- **命令注册 Checklist**：
  - 新增 `#[tauri::command]` 时，**必须同步在 `lib.rs` 的 `invoke_handler!` 宏列表中注册**，否则前端调用会提示 command not found。

### Vue 3 前端 (Vue 3 / TypeScript)

- **类型约束**：
  - 必须对所有 `invoke` 的入参及返回结果定义明确的 TypeScript Interface，绝对禁止使用 `any`。
  - Rust 返回的 Result 应该在前端有合理的错误捕获（`try-catch` 或 `.catch()`），并通过 `useToast` 或 `message` 呈现给用户。
- **事件监听生命周期管理**：
  - 使用 `@tauri-apps/api/event` 的 `listen` 订阅 Rust 事件时，必须在组件销毁时（`onUnmounted`）调用返回的 `unlisten()` 函数，以防闭包内存泄漏。
- **LCU API 隔离原则**：
  - 前端绝不应直接建立与 LCU 端口的 HTTP/WebSocket 连接。
  - 所有 LCU 接口的调用，必须经由 Rust 端的 `call_lcu_api` 转发，以规避 Token 泄漏并统一错误捕获。
- **组件及路由状态保留**：
  - 页面路由切换基于 Vue 的 `currentPage` 控制。
  - Search 页和 GameInfo 页由于数据量较大且需要保留搜索/对比状态，必须使用 `v-show` 保持组件挂载，避免重新渲染销毁状态。
- **主题与样式**：
  - 遵循 "纯白水晶极光" 风格，背景使用毛玻璃模糊（`backdrop-filter: blur`），配色统一采用动态 CSS 变量，不可随意硬编码色值。

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

# 代码质量校验与测试（修改代码后进行验证）
pnpm type-check      # Vue / TS 类型检查 (vue-tsc --noEmit)
pnpm clippy          # Rust 代码静态检查 (cargo clippy)
pnpm format          # Rust 代码格式化 (cargo fmt)
pnpm test:rust       # 运行 Rust 单元测试 (cargo test)
pnpm check-all       # 一键检查全部 (Format + Type-Check + Clippy)
```

## 项目结构

```
Yuumi/
├── src/                            # Vue 前端
│   ├── App.vue                     # 根组件（自定义标题栏 + 导航栏 + 路由切换）
│   ├── main.ts                     # 主窗口入口
│   ├── opgg.ts                     # OP.GG 独立窗口入口
│   ├── i18n.ts                     # 多语言配置
│   ├── api/
│   │   └── lcu.ts                  # LCU API 封装 + Rust 命令调用
│   ├── store/
│   │   └── lcuStore.ts             # Pinia 全局状态（LCU 事件映射）
│   ├── utils/
│   │   └── theme.ts                # 主题色动态更新
│   ├── composables/
│   │   ├── useLcuAsset.ts          # LCU 资源路径 → data URL（缓存 + 去重）
│   │   ├── useToast.ts             # Naive UI 消息提示 Hook (多窗口安全降级)
│   │   ├── usePlayerSearch.ts      # 召唤师名称点击跳转搜索
│   │   ├── useTftData.ts           # 云顶之弈 API & 数据处理
│   │   ├── useTftMetaDecks.ts      # 云顶 OP.GG 热门阵容解析
│   │   ├── useLoot.ts              # 战利品智能开箱/分解/重铸
│   │   ├── useMatchHistory.ts      # 战绩历史 Hook
│   │   ├── usePremadeGroup.ts      # 组队分析 Hook
│   │   ├── useGamePlayerData.ts   # 对局玩家数据集中管理
│   │   └── useAutoSaveConfig.ts    # 配置项自动保存 Hook
│   ├── assets/                     # 静态资源（图片等）
│   ├── views/
│   │   ├── Home.vue                # 首页（LCU 状态 + 快捷导航）
│   │   ├── Career.vue              # 生涯战绩（召唤师 + 对局历史）
│   │   ├── Search.vue              # 战绩查询（玩家搜索 + 对局列表）
│   │   ├── GameInfo.vue            # 对局信息（10 人段位 + KDA）
│   │   ├── SavedPlayers.vue        # 路人集（曾同局玩家/标记玩家管理）
│   │   ├── TFT.vue                 # 云顶之弈（段位/战绩/阵容推荐/海克斯）
│   │   ├── Settings.vue            # 设置（头像/签名/在线状态/配置/HTTP代理）
│   │   ├── Tools.vue               # 工具箱（创建房间/ARAM 摇号/自动流程/战利品/符文/皮肤等）
│   │   └── BenchOverlay.vue        # 大乱斗板凳席悬浮窗
│   └── components/
│       ├── tft/                    # 云顶之弈子组件（TftRankHeader/TftMatchCard/TftMatchDetailModal/TftMetaCompsTab/TftAugmentsTab）
│       ├── gameinfo/               # 对局信息子组件（PlayerCard/PlayerMatchColumn/TeamComposition/PhaseBadge 等）
│       ├── career/                 # 生涯子组件（SummonerHeader/MatchHistoryTab）
│       ├── tools/                  # 工具箱子组件（AutoGameflowCard/LootTab 等）
│       ├── layout/                 # 布局与自定义标题栏
│       ├── OpggModal.vue           # OP.GG 数据弹窗
│       ├── OpggWindow.vue          # OP.GG 独立窗口组件
│       ├── NoticePopup.vue         # 更新日志弹窗组件
│       ├── LcuOfflineState.vue     # 统一 LCU 离线未连接状态
│       ├── LcuImage.vue            # LCU 资源图片组件（loading/error/fallback 状态）
│       ├── ChampionPicker.vue      # 英雄选择器（v-model: number[]）
│       ├── SpellPicker.vue         # 召唤师技能选择器（v-model: number[]）
│       └── NaiveUIBridge.vue       # Naive UI 全局 API 桥接组件
├── src-tauri/                      # Tauri/Rust 后端
│   ├── src/
│   │   ├── main.rs                 # Rust 入口
│   │   ├── lib.rs                  # AppState、命令注册、agent 启动、系统托盘
│   │   ├── config.rs               # 配置读写与 Schema 自动迁移（%APPDATA%/Yuumi/config.json）
│   │   ├── saved_players.rs        # 路人集系统（SQLite 持久化/标记管理/相遇历史/导入导出）
│   │   ├── commands/               # Tauri 拆分命令 (config.rs, lcu.rs, tools.rs)
│   │   ├── loot.rs                 # 战利品管理系统 (开箱/分解/重铸/精粹查询)
│   │   ├── updater.rs              # 自动更新机制与更新日志本地缓存
│   │   ├── tools.rs                # 杂项工具（创建房间/ARAM 摇号/符文/皮肤/OP.GG抓取）
│   │   ├── logging.rs              # 日志系统（flexi_logger，日志写入 exe 同级 log/ 目录）
│   │   ├── signalr.rs              # SignalR Hub 远程反代（条件启动）
│   │   ├── upload.rs               # 对局上传队列（包含落盘暂存与重试逻辑）
│   │   ├── lcu/
│   │   │   ├── mod.rs
│   │   │   ├── monitor.rs          # LCU 进程轮询（sysinfo + lockfile + WMIC 兜底）
│   │   │   ├── client.rs           # HTTPS 代理（忽略 SSL + Basic Auth + CDragon 代理请求 + 全局超时）
│   │   │   ├── opgg.rs             # OP.GG API 隔离层（HTTP 代理 + 内存缓存）
│   │   │   ├── sgp.rs              # SGP 远程 API 工具（信号量限流控制）
│   │   │   ├── ws.rs               # WebSocket 事件订阅 → 广播前端 + 分发 agents（带取消机制）
│   │   │   └── game_data.rs        # 游戏资源预加载（物品/技能/符文/Cherry海克斯/英雄 ID→名称/iconPath）
│   │   ├── parsers/
│   │   │   ├── mod.rs
│   │   │   ├── summoner.rs         # 召唤师数据清洗
│   │   │   ├── match_parser.rs     # 战绩数据清洗（parseGameData/get_recent_teammates/海克斯大乱斗/经典模式）
│   │   │   ├── game_info.rs        # 对局信息（10 人段位 + 近期 KDA + 宿命对局分析）
│   │   │   └── tft.rs              # TFT 云顶之弈数据解析 (战绩/段位/海克斯/阵容)
│   │   └── agents/
│   │       ├── mod.rs
│   │       ├── auto_bp.rs          # 自动选人/禁人/召唤师技能/板凳席推送
│   │       └── auto_match.rs       # 自动接受匹配/接受邀请/自动点赞/延时再来一局/自动重连/标记提醒/对局结束触发上传
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

| 命令                         | 来源                    | 说明                                      |
| :--------------------------- | :---------------------- | :---------------------------------------- |
| `greet`                      | lib.rs                  | 测试命令                                  |
| `get_config`                 | commands/config.rs      | 读取完整 AppConfig                        |
| `update_config`              | commands/config.rs      | 写入完整 AppConfig                        |
| `get_config_load_error`      | commands/config.rs      | 读取配置文件加载错误信息                  |
| `get_close_to_tray`          | commands/config.rs      | 读取关闭到托盘设置                        |
| `get_lcu_connection_info`    | commands/lcu.rs         | 获取 LCU PID/port/token                   |
| `get_game_data_assets`       | commands/lcu.rs         | 获取预加载的游戏资源映射                  |
| `get_map_side`               | commands/lcu.rs         | 获取当前对局的游戏阵营 (蓝方/红方)        |
| `detect_lol_path`            | commands/tools.rs       | 自动检测 LOL 客户端路径                   |
| `select_lol_folder`          | commands/tools.rs       | 打开 LOL 客户端文件夹选择框               |
| `select_folder`              | commands/tools.rs       | 打开通用原生文件夹选择对话框              |
| `open_screenshot_folder`     | commands/tools.rs       | 打开游戏截图目录                          |
| `set_mica_effect`            | commands/tools.rs       | 设置窗口 Mica (云母) 毛玻璃特效           |
| `launch_lol_client`          | commands/tools.rs       | 自动启动 LOL 客户端                       |
| `show_bench_overlay_window`  | commands/tools.rs       | 控制并显示大乱斗板凳席悬浮窗              |
| `call_lcu_api`               | lcu/client.rs           | 通用 LCU API 转发                         |
| `get_lcu_asset`              | lcu/client.rs           | 获取单个 LCU 图片资源                     |
| `get_current_summoner`       | parsers/summoner.rs     | 获取召唤师信息                            |
| `get_match_history`          | parsers/match_parser.rs | 获取战绩列表 (LCU 本地接口)               |
| `get_match_history_sgp`      | parsers/match_parser.rs | 获取战绩列表 (SGP 远程接口)               |
| `get_recent_teammates`       | parsers/match_parser.rs | 获取近期组队队友数据分析                  |
| `get_game_player_summaries`  | parsers/game_info.rs    | 获取对局 10 人段位 + KDA                  |
| `get_player_fate_info`       | parsers/game_info.rs    | 获取同场玩家历史交手/宿命战绩统计        |
| `get_tft_data`               | parsers/tft.rs          | 获取云顶之弈基础数据                      |
| `get_tft_ranked_stats`       | parsers/tft.rs          | 获取云顶之弈段位与近期胜率统计            |
| `get_tft_match_history`      | parsers/tft.rs          | 获取云顶之弈对局历史与详情                |
| `get_tft_augments`           | parsers/tft.rs          | 获取海克斯强化百科（LCU优先，CDragon兜底）|
| `create_5v5_practice_lobby`  | tools.rs                | 创建自定义房间                            |
| `aram_reroll_and_swap_back`  | tools.rs                | 大乱斗摇号换回                            |
| `apply_rune_page`            | tools.rs                | 应用符文页                                |
| `get_lcu_zoom`               | tools.rs                | 获取 LCU 窗口缩放                         |
| `fix_lcu_window`             | tools.rs                | 修复 LCU 窗口位置                         |
| `clear_game_cache`           | tools.rs                | 清除游戏缓存                              |
| `open_log_folder`            | tools.rs                | 打开日志目录                              |
| `fetch_opgg_data`            | tools.rs                | 获取 OP.GG 召唤师/英雄数据                |
| `fetch_tft_meta_decks`       | tools.rs                | 获取 OP.GG 云顶之弈热门阵容数据           |
| `get_champion_skins`         | tools.rs                | 获取英雄皮肤列表                          |
| `get_game_settings_readonly` | tools.rs                | 读取游戏设置                              |
| `set_game_settings_readonly` | tools.rs                | 写入游戏设置                              |
| `spectate_directly`          | tools.rs                | 直接观战指定玩家                          |
| `get_openable_loots`         | loot.rs                 | 获取可打开的战利品列表                    |
| `batch_open_loots`           | loot.rs                 | 批量打开指定战利品                        |
| `smart_open_all_loots`       | loot.rs                 | 一键智能全自动批量开箱                    |
| `get_loot_inventory`         | loot.rs                 | 获取当前全部战利品库存                    |
| `disenchant_loot`            | loot.rs                 | 分解指定战利品（蓝色/橙色精粹）          |
| `reroll_loot`                | loot.rs                 | 重铸战利品（三合一英雄/皮肤）             |
| `upgrade_loot`               | loot.rs                 | 升级/解锁战利品                            |
| `get_essence_balances`       | loot.rs                 | 查询精粹余额                              |
| `upload_single_match`        | upload.rs               | 单场对局上传                              |
| `batch_upload_matches`       | upload.rs               | 批量对局上传                              |
| `get_signalr_status`         | signalr.rs              | 获取 SignalR 远程连接状态                 |
| `check_update`               | updater.rs              | 检查软件更新                              |
| `install_update`             | updater.rs              | 开始安装最新软件更新                      |
| `install_pending_update`     | updater.rs              | 安装已下载且挂起的更新                    |
| `fetch_github_text`          | commands/tools.rs       | 代理抓取 GitHub 资源                      |
| `get_release_changelog`      | commands/tools.rs       | 获取版本更新日志                          |
| `save_saved_player`          | saved_players.rs        | 保存/标记同局玩家信息                    |
| `query_all_saved_players`    | saved_players.rs        | 查询路人集列表（支持标签过滤/置顶）       |
| `query_encountered_games`    | saved_players.rs        | 查询指定玩家相遇对局历史                  |
| `get_saved_players_map`      | saved_players.rs        | 获取已标记玩家 HashMap                    |
| `delete_saved_player`        | saved_players.rs        | 删除指定已保存玩家                        |
| `export_tagged_players_to_json_file` | saved_players.rs| 导出标记玩家到 JSON                       |
| `import_tagged_players_from_json_file` | saved_players.rs| 从 JSON 导入标记玩家                      |
| `backfill_saved_player_identity`     | saved_players.rs| 异步补全已记录玩家的召唤师身份            |

新命令需在 `lib.rs` 的 `invoke_handler` 中注册。

## Agent 后台任务

| Agent      | 触发事件                       | 功能                                     |
| :--------- | :----------------------------- | :--------------------------------------- |
| auto_bp    | `/lol-champ-select/v1/session` | 自动选人/禁人/设置召唤师技能/推送板凳席  |
| auto_match | gameflow-phase + ready-check   | 自动接受匹配/自动接受邀请/自动点赞/延时再来一局/自动重连/标记提醒 + 游戏结束触发上传 |

## 云顶之弈模块 (parsers/tft.rs & src/components/tft)

- **数据源回落 (Fallback)**: 强化符文百科优先请求 LCU 本地 API，不可用时自动降级拉取 CDragon 镜像；支持 `force_refresh` 强刷新与 Gzip 压缩代理传输。
- **阵容推荐**: 集成 OP.GG 热门阵容，支持前中期打工过渡、激活羁绊解析、一键复制阵容代码以及 4x7 大网格站位图（含核心 C 位、星级与装备优先级图标）。

## 战利品系统 (loot.rs)

- **批量与智能开箱**: 支持遍历并自动解包传送门、法球、杰作宝箱与法球，后台线程并发安全处理。
- **精粹统筹与重铸**: 提供战利品碎片一键分解为精粹、合成重铸以及永久解锁功能。

## 上传系统 (upload.rs)

- **UploadQueue**: 去重异步队列，后台 Worker 串行上传，每局 30 秒超时。
- **UploadTrigger**: 监听游戏阶段转换（InProgress → EndOfGame/Lobby），延迟 2 秒后自动上传。
- **Smart Split**: 当前玩家数据放入 `matchInfo.participants`，其余 9 人放入外层 `participants`。
- **失败落盘与重试**: 当网络异常或服务端报错时，数据自动落盘为 `.tmp` 暂存文件，后续在对局结束或状态更新时触发重试。

## 配置文件

位于 `%APPDATA%/Yuumi/config.json`，结构：

- `General` — 启动、代理 (HTTP Proxy 与 Schema 自动迁移)、日志、上传 API 地址、SignalR Hub 配置、客户端路径
- `Personalization` — 主题、语言、颜色、侧边栏显示/隐藏控制 (TFT/SavedPlayers)
- `Functions` — 所有自动化功能开关和候选列表 (自动接受/邀请/点赞/再来一局/ARAM报边/标记提醒)
- `Other` — 其他杂项

## 开发工具与调试日志

- **前端调试**: 开发模式下自动打开 Chromium DevTools（见 `lib.rs` 的 `setup` 闭包），按 `F12` 可审查组件与网络请求。
- **日志系统**: 日志由 `flexi_logger` 接管，按天轮转保存 30 天。
  - 日志文件输出路径：用户目录 `%APPDATA%/Yuumi/log/` 或可执行文件同级 `log/` 目录。
  - 排查 LCU 通信或后台 Task 异常时，请检索最新 `.log` 文件中的 `[LCU]`, `[WS]`, `[Error]` 标记。
