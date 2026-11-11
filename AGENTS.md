# Yuumi

GPUI 原生 Rust 桌面应用（`crates/yuumi-core` + `crates/yuumi-gpui`），基于原版 Tauri v2 + Vue 3 架构进行原生重构。

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

## GPUI & Rust 编码规范 (GPUI & Rust Guidelines)

### 架构边界与职责划分

- **`crates/yuumi-core`（核心领域库）**:
    - 纯无头（Headless）Rust 逻辑库，绝对禁止引入任何 GPUI、GUI 或视口相关依赖。
    - 承载 LCU 客户端通信、WebSocket 事件流、白名单投影、自动化 Agent（Auto BP、Auto Match、Supervisor 调度）、数据解析清洗、SQLite 本地持久化、配置读写与自动更新。
    - 暴露纯 Rust 异步函数与数据类型，对外提供强类型领域事件与状态管道。
- **`crates/yuumi-gpui`（原生桌面客户端）**:
    - 基于 Zed GPUI 实体组件模式（Entity / View）驱动界面。
    - 拥有主窗口 (`MainWindow`) 及独立悬浮/弹窗 (`BenchOverlayView`、`OpggWindow`)。
    - 依赖 `yuumi-core`，通过 `LiveLcuService` 编排中心订阅领域事件并拉取数据，在 UI 线程精准高效重绘。

### GPUI 渲染与 Tokio 异步隔离规范

- **Tokio 运行时句柄注册与任务投递**:
    - GPUI 的 `cx.background_executor()` 由底层平台线程池驱动，线程上下文内**没有** Tokio reactor；在其中直接调用 reqwest 网络请求或 tokio I/O 会 panic（`there is no reactor running`）。
    - 所有后台网络 I/O、LCU 接口请求及耗时任务必须通过 `yuumi_core::runtime::spawn_background(...)` 或全局 `yuumi_core::runtime::tokio_handle()` 投递至 Tokio 运行时执行。
- **UI 主线程零阻塞**:
    - 严禁在 GPUI UI 主线程中执行耗时的 CPU 计算、同步磁盘读写或同步阻塞等待。
- **异步就绪与精准重绘**:
    - 后台任务完成时，通过 `cx.spawn(...)` 配合 `this.update(cx, |this, cx| ...)` 或 `cx.notify()` 将数据回写至实体状态并驱动视图刷新。
- **共享状态与死锁防护**:
    - 在异步函数中获取状态锁时，**严禁跨 `await` 点持有同步锁 (`std::sync::MutexGuard`)**，必须先释放锁或在独立作用域块内完成读写后再 `await`，防止 Tokio 线程死锁。

### 静态资源与图片异步加载引擎

- **多级缓存与 BGRA 解码**:
    - 静态资源经 `AssetCache` 完成磁盘与内存缓存，通过 `AssetLoader` 在 Tokio 线程池并发解码为 GPUI 原生所需的 BGRA `RenderImage`，彻底消除 UI 帧率抖动。
    - 列表与详情视图加载前，优先通过 `prewarm_assets` 在后台批量预热图片，解码就绪后触发单次 `cx.notify()` 重绘。
- **内嵌矢量资产与主题着色**:
    - 所有内置线性图标（SVG）经 `assets::Assets` 实现 `gpui::AssetSource` 编译进二进制，经 `svg()` 元素自动转化为 alpha 蒙版并按当前主题语义色着色。
    - 具象彩色徽标（如 OP.GG Logo）内嵌预解码位图，避免被全局单色蒙版覆写。

### 主题与样式规范

- **纯白水晶极光 (Crystal Aurora)** 风格。
- 统一通过 `crate::theme::Theme` 访问语义色常量与尺寸配置，支持浅色/深色主题动态适配。
- 动态自定义颜色（主题色、胜/负/重赛卡片颜色）由 `RUNTIME_COLORS` 覆盖表维护，保证用户修改设置后即时响应。

### 类型安全与错误治理

- 所有与外部交互的数据模型严格实现 `serde::Serialize` 和 `serde::Deserialize`，合理使用 camelCase / PascalCase 别名与 Default 兜底。
- 生产代码严禁随意 `unwrap()` 或 `panic!`，统一通过 `Result<T, ...>` 传播，内部异常记录详细日志（`log::error!`），面向用户的错误通过 `Toast` 浮层友好呈现。

## 技术栈

- **UI 渲染**: GPUI (`crates/yuumi-gpui`)
- **核心逻辑**: Rust (`crates/yuumi-core`)
- **异步运行时**: Tokio (multi-thread)
- **本地存储**: SQLite (`rusqlite bundled`)
- **包管理**: Cargo (Workspace)

## 常用命令

```bash
# 开发与运行（依赖项自动开启 opt-level=2 优化，保障 GPUI 渲染性能）
cargo run -p yuumi-gpui
# 或
cargo run --bin yuumi

# 构建生产版本（输出 target/release/yuumi.exe）
cargo build --release -p yuumi-gpui

# 代码质量校验与测试（修改代码后进行验证）
cargo fmt --all                        # 自动修复全工作区代码格式
cargo fmt --all -- --check             # 检查代码格式是否合规
cargo clippy --workspace --all-targets # 全工作区 Clippy 静态检查
cargo test --workspace                 # 运行全部单元测试（含解析层回归测试）

# 抓取真实 LCU 测试快照（可选，需客户端处于运行状态）
node scripts/capture-lcu-fixtures.mjs
```

> **Git 钩子约定**: 仓库集成 `.githooks/pre-commit`，在提交时自动对暂存的 `.rs` 文件执行 `rustfmt` 并运行 `cargo clippy --workspace --all-targets`。

### 解析层测试约定（fixture 驱动）

`crates/yuumi-core/testdata/match/` 存放 LCU 对局响应快照，`manifest.json` 记录每份样本的覆盖场景：

- `real/` —— 从真实客户端抓包（`node scripts/capture-lcu-fixtures.mjs`），覆盖排位/匹配各分路组合、三分钟重开、自定义队列 3100。
- `synthetic/` —— 按 schema 合成，仅用于真实抓包缺失的场景（大乱斗、斗魂竞技场、420 队列名映射、稀疏载荷边界）。合成样本必须在此目录，不得与真实抓包混淆。

样本保存的是**完整外层响应**（含 `participantIdentities`、`teams`、`timeline`、`gameMode`、`platformId` 及 `stats` 中 118 个字段里未建模的部分），用途是防止结构体过度收紧——内联最小 JSON 的测试永远发现不了「真实载荷多一个字段就解析失败」这类问题。

- 新增解析路径的测试优先写 fixture，而非在测试里内联 `json!({...})`。
- `all_fixtures_on_disk_deserialize` 会遍历 `real/` 与 `synthetic/` 两个目录逐一反序列化，**新增样本无需改测试**（它不扫根目录，因为那里有 `manifest.json` 与 `_capture/`）。
- 需要真实数据时执行 `node scripts/capture-lcu-fixtures.mjs`（默认对 puuid / summonerId / 召唤师名做脱敏，解析层不消费这些字段，加 `--no-redact` 可保留原值）。线索：部分 WeGame 渠道客户端不写 `lockfile`（0 字节），脚本会自动退化为读取 `LeagueClientUx.exe` 的进程命令行。
- **脚本只写 `crates/yuumi-core/testdata/match/_capture/`（暂存区，已 gitignore）**，不写 `real/`、不覆盖 `manifest.json`：抓包结果是一次 20 局的整体快照，且含 gameflow / champ-select 等非 matches 结构的会话快照（放在 `_capture/sessions/`），自动覆盖会让「精选样本 + 手写元数据」同时失效。迁入流程见脚本结束时的提示：挑选 → 重命名进 `real/` → 补 `manifest.json` 的 `real[]` → 在 `match_parser.rs` 的 `REAL` 登记并用 `dump_real_fixture_results` 生成 `REAL_EXPECT`。
- 参照 `manifest.json` 的 `observedInRealPayload`，以下几点是实测得出的、容易被误判的载荷事实：`roleBoundItem` 经常不是分路任务道具（实测出现 3006、2055），只能作候选；存在 `playerAugment6` 但只建模到 5；`stats` 里**没有** `augments` 数组字段。

## 项目结构

<details>
<summary>展开查看完整目录树（Cargo Workspace）</summary>

```
Yuumi/
├── Cargo.toml                         # Workspace 配置（crates/yuumi-core + crates/yuumi-gpui + vendor/gpui patch）
├── crates/
│   ├── yuumi-core/                    # 纯核心领域库（无 UI 依赖）
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs                 # 核心模块导出
│   │   │   ├── config.rs              # 配置读写与 Schema 自动迁移（%APPDATA%/Yuumi/config.json）
│   │   │   ├── runtime.rs             # 运行时模式检测（便携版/安装版）、全局 Tokio 运行时句柄注册
│   │   │   ├── opgg.rs                # OP.GG API 客户端、数据解析与内存缓存
│   │   │   ├── update.rs              # 自动更新校验（minisign）、GitHub Release 检索与便携版替换 helper
│   │   │   ├── upload.rs              # 对局上传队列（去重/落盘 .tmp 暂存/重试）
│   │   │   ├── signalr.rs             # SignalR Hub 远程反代
│   │   │   ├── lcu/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── monitor.rs         # LCU 进程轮询（sysinfo + lockfile + 命令行/WMIC 三重探测）
│   │   │   │   ├── client.rs          # HTTPS 客户端（忽略自签证书 + Basic Auth + 并发信号量限流 + CDragon 代理）
│   │   │   │   ├── ws.rs              # WebSocket 事件订阅（带取消机制：新连接自动终止旧循环）
│   │   │   │   ├── event_projection.rs# LCU 事件字段白名单投影与同 URI 内容去重
│   │   │   │   ├── game_data.rs       # 游戏资源预加载（物品/技能/符文/Cherry海克斯/英雄映射）
│   │   │   │   ├── asset_cache.rs     # 静态资源内存+磁盘二级缓存
│   │   │   │   └── write.rs           # LCU 写操作（符文页、装备页、房间创建、生涯背景、状态签名等）
│   │   │   ├── gameflow/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── team_builder.rs    # 阵营判定（含脱敏推断）、gameflow → 队伍映射与组队识别
│   │   │   │   ├── player_identity.rs # 玩家身份兼容判定与槽位英雄继承
│   │   │   │   └── match_stats.rs     # 胜率/KDA/连胜/位置英雄聚合统计
│   │   │   ├── parsers/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── summoner.rs        # 召唤师信息清洗
│   │   │   │   ├── mastery.rs         # 英雄熟练度 2.0 与里程碑解析
│   │   │   │   ├── match_parser.rs    # 战绩列表清洗（经典/海克斯大乱斗/SGP）
│   │   │   │   ├── match_detail.rs    # 单局对局详情解析
│   │   │   │   ├── teammates.rs       # 近期同队队友分析
│   │   │   │   ├── game_info.rs       # 对局信息（10 人段位 + 近期战绩 + 宿命交手统计）
│   │   │   │   ├── tft.rs             # 云顶之弈（段位/战绩/CDragon资源字典/OP.GG阵容）
│   │   │   │   ├── loot.rs            # 战利品库存与精粹解析
│   │   │   │   ├── loot_actions.rs    # 智能开箱/分解/重铸/解锁动作编排
│   │   │   │   ├── replay.rs          # 对局 ROFL 回放下载/状态/观看
│   │   │   │   └── saved_player.rs    # 路人集身份与快照模型
│   │   │   ├── storage/
│   │   │   │   ├── mod.rs
│   │   │   │   └── saved_players.rs   # 路人集 SQLite 持久化（标记/备注/相遇历史/JSON 导入导出/身份回填）
│   │   │   └── agents/
│   │   │       ├── mod.rs
│   │   │       ├── config.rs          # 自动化开关与分路候选配置模型
│   │   │       ├── lifecycle.rs       # 游戏阶段枚举与生命周期代际守卫（GenerationGuard）
│   │   │       ├── supervisor.rs      # 任务独占并发调度器（AgentSupervisor）
│   │   │       ├── auto_bp.rs         # 自动选人/禁人/召唤师技能/板凳席处理
│   │   │       └── auto_match.rs      # 自动接受匹配/接受邀请/自动点赞/再来一局/自动重连/标记提醒
│   │   └── testdata/                  # 真实与合成的 LCU/TFT/Session 测试快照（fixture）
│   └── yuumi-gpui/                    # GPUI 原生桌面应用（主二进制产物：yuumi.exe）
│       ├── Cargo.toml
│       ├── build.rs                   # Windows 资源清单与 exe 图标内嵌
│       ├── assets/                    # 静态矢量图标 (SVG) 与内置位图资产
│       └── src/
│           ├── main.rs                # 程序入口：便携升级判定、Tokio 运行时初始化、单实例互斥、GPUI 窗口启动
│           ├── app_window/            # 主窗口状态、路由视图渲染与事件响应
│           │   ├── mod.rs             # MainWindow 定义与资源预热（prewarm_assets）
│           │   ├── shell.rs           # 外壳布局（标题栏/侧栏/路由分发/回到顶部/弹窗）
│           │   ├── lifecycle.rs       # LiveLcuEvent 分派、配置同步、独立子窗生命周期管理
│           │   └── ...                # career / search / game_info / saved_players / tft / tools / settings / f_updates
│           ├── views/                 # 各业务页面视图组件
│           │   ├── mod.rs             # Route 枚举与侧栏导航定义
│           │   ├── home.rs            # 首页（连接状态/客户端启动/快捷入口）
│           │   ├── career.rs          # 生涯（战绩列表/英雄熟练度/挑战勋章/战利品弹窗）
│           │   ├── search.rs          # 战绩查询（玩家搜索/战绩详情）
│           │   ├── game_info.rs       # 对局信息（10 人段位/KDA/队伍与全员视图切换）
│           │   ├── saved_players.rs   # 路人集（标记管理/相遇对局历史/导入导出）
│           │   ├── tft.rs             # 云顶之弈（段位/战绩/OP.GG 热门阵容/海克斯百科）
│           │   ├── tools.rs & tools/  # 实用工具箱（好友清理/房间/符文/装备/观战/客户端修补）
│           │   └── settings.rs        # 设置页（个性化/路径/代理/截图/云端/更新/关于）
│           ├── components/            # 通用 UI 基础组件（titlebar/sidebar/card/dropdown/switch/toast/lcu_image 等）
│           ├── opgg/                  # OP.GG 独立窗口（OpggWindow）与梯度榜/出装/符文/技能组件
│           ├── services/              # 应用后台服务集成
│           │   ├── live_lcu.rs        # 中心实时服务（LCU 连接生命周期、WebSocket 分发、防抖拉取与事件广播）
│           │   ├── agent_runtime.rs   # Agent 事件执行与后台任务桥接
│           │   ├── cloud_tasks.rs     # 对局上传触发与 SignalR Hub 任务管理
│           │   ├── screenshot.rs      # 游戏内多杀事件监听与自动截图（xcap）
│           │   ├── logging.rs         # 日志系统（自研 log::Log 实现 + 按天/2MB 轮转）
│           │   └── ...                # challenges / champion_alias / friend_cleaner / profile_background / spectate / system_tools
│           ├── bench_overlay.rs       # 大乱斗板凳席独立置顶悬浮窗（BenchOverlayView）
│           ├── asset_loader.rs        # 图片异步加载与 Tokio BGRA RenderImage 解码引擎
│           ├── assets.rs              # 内嵌 SVG/位图资产数据源（AssetSource）
│           ├── theme.rs               # Crystal Aurora 主题色板与亮暗模式运行时覆盖
│           ├── tray.rs                # 系统托盘管理器（TrayManager）
│           ├── i18n.rs                # 国际化文案
│           └── platform/              # Windows 平台支持（单实例互斥、Win32 窗口控制、Mica 云母特效）
├── vendor/gpui/                       # 本地打补丁的 gpui 0.2.2（避免 WM_MOVE 无效重绘）
├── .githooks/                         # 纯 Git 钩子（pre-commit 自动 rustfmt 与 clippy）
├── .github/                           # CI 工作流、Release 构建脚本与 NSIS 安装包模板
├── scripts/                           # 辅助工具脚本（capture-lcu-fixtures.mjs）
├── File/                              # 重构参考文档（不入库）
├── AGENTS.md
└── README.md
```

</details>

## 架构数据流

```
LeagueClientUx.exe
  ↓ (sysinfo 轮询 port/token，支持 lockfile / 命令行 / WMIC 三种检测)
monitor.rs → LcuClient + GameDataAssets（预加载英雄/物品/技能/符文）
  ↓
ws.rs → LCU WebSocket 事件（带取消机制：新连接自动终止旧循环）
  ├─→ event_projection 字段投影 + 去重 → LiveLcuService (broadcast / watch)
  │     ├─→ MainWindow (UI 主线程接收 LiveLcuEvent 驱动视图刷新与资产预热)
  │     ├─→ BenchOverlayView (大乱斗板凳席悬浮窗实时同步)
  │     └─→ CloudTasks / SignalR (对局上传与远程反代)
  ├─→ BP Agent: auto_bp.rs (自动选人/禁人/召唤师技能)
  └─→ Match Agent: auto_match.rs (自动接受/重连/自动点赞/再来一局/标记提醒)
```

### LCU 事件投影契约

`crates/yuumi-core/src/lcu/event_projection.rs` 在分发会话快照前会做**字段白名单投影**与**同 URI 内容去重**：
选人阶段 session 每秒最多推送 20 次，LCU 原始载荷中大量冗余字段（`obfuscatedPuuid`、`selectedSkinId`、
`spell1Id/2Id`、`playerType`、`chatDetails` 等）业务从不消费，投影与去重可显著降低内存拷贝与反序列化开销。

- 新增读取 champ-select 或 gameflow session 中的字段时，必须同步扩展 `event_projection.rs` 的对应白名单（如 `CHAMP_SELECT_PLAYER_KEYS`、`GAMEFLOW_PARTICIPANT_KEYS`），否则该字段会被过滤掉。
- 只有 `champ-select session` 与 `gameflow session` 做投影；`gameflow-phase`、`current-champion`、`ready-check` 原样透传。
- 去重状态按 WS 连接实例创建（重连自动清空）。

## 窗口与 UI

- **主窗口 (`MainWindow`)**:
    - 无边框窗口 (`appears_transparent: true`)，默认 `1200×910` 居中（最小 `880×600`），自定义标题栏含拖拽/最小化/最大化/关闭、返回导航、当前游戏阶段与阵营指示器。
    - 支持 Windows 11 Mica（云母）半透明背景效果，支持设置页 DPI 缩放覆盖。
    - 侧栏支持展开/折叠（`64px ↔ 210px`），通过 `Route` 枚举管理 8 个核心路由视图；独立视口采用局部滚动或外层统一滚动，附带回到顶部悬浮按钮。
- **独立窗口**:
    - **大乱斗板凳席悬浮窗 (`BenchOverlayView`)**: 独立置顶半透明无边框小窗，支持自由拖拽并记忆坐标、15 秒摇号保护期倒计时、快速交换与自动摇号换回，支持用户手动关闭防重复弹出。
    - **OP.GG 独立窗口 (`OpggWindow`)**: 跟随主窗所在屏幕定位，提供英雄梯度榜、分路对位出装、符文与技能加点参考。
- **系统托盘 (`TrayManager`)**:
    - 完整托盘菜单（主页/生涯/战绩查询/对局信息/TFT/工具/设置/退出），支持关闭到托盘、左键单击切换显示主窗口。
- **主题与渲染性能**:
    - "纯白水晶极光 (Crystal Aurora)" 风格，亮/暗色跟随系统或手动切换。
    - `Cargo.toml` 中通过 `[profile.dev.package."*"] opt-level = 2` 为 `gpui` / `taffy` 依赖开启优化，确保开发模式下布局与绘制依然流畅；`vendor/gpui` 修补了 `WM_MOVE` 引起的冗余全量重绘。

## 核心能力与服务模块

原生重构消除了前后端跨进程 IPC 通信开销，`yuumi-gpui` 直接调用 `yuumi-core` 的 Rust 异步接口：

| 领域             | `yuumi-core` 核心模块                                  | `yuumi-gpui` 集成与视图                                               | 说明                                                          |
| :--------------- | :----------------------------------------------------- | :-------------------------------------------------------------------- | :------------------------------------------------------------ |
| LCU 通信与资源   | `lcu/{monitor,client,ws,game_data,asset_cache,write}`  | `services/live_lcu.rs`, `asset_loader.rs`                             | 进程探测、HTTPS/WS 订阅、游戏资源预加载、BGRA 图片解码        |
| 召唤师与生涯     | `parsers/{summoner,mastery,match_parser,match_detail}` | `views/career.rs`, `career_view.rs`, `app_window/career.rs`           | 召唤师信息、战绩列表（LCU/SGP）、英雄熟练度 2.0、挑战勋章     |
| 战绩查询与回放   | `parsers/{match_parser,match_detail,replay}`           | `views/search.rs`, `replay_store.rs`, `app_window/search.rs`          | 任意玩家查询、详情面板、ROFL 回放下载与播放                   |
| 对局信息与宿命   | `parsers/game_info.rs`, `gameflow/*`                   | `views/game_info.rs`, `game_info_view.rs`, `app_window/game_info.rs`  | 10 人实时对局分析、段位拉取、组队识别与宿命相遇统计           |
| 路人集系统       | `storage/saved_players.rs`, `parsers/saved_player.rs`  | `views/saved_players.rs`, `app_window/saved_players.rs`               | SQLite 持久化标记、相遇历史查询、JSON 导入导出、身份回填      |
| 云顶之弈 (TFT)   | `parsers/tft.rs`                                       | `views/tft.rs`, `app_window/tft.rs`                                   | 段位与战绩、CDragon 资源字典、海克斯强化、OP.GG 热门阵容      |
| 战利品管理       | `parsers/{loot,loot_actions}`                          | `views/career.rs`（战利品弹窗）, `app_window/career.rs`               | 库存查询、精粹统计、智能批量开箱/分解/重铸/解锁               |
| 实用工具与 OP.GG | `lcu/write.rs`, `opgg.rs`                              | `views/tools.rs`, `opgg/*`, `services/{friend_cleaner,spectate,...}`  | 好友清理、5v5 自定义房间、符文/装备应用、直接观战、OP.GG 窗口 |
| 自动化与截图     | `agents/*`, `upload.rs`, `signalr.rs`                  | `services/{agent_runtime,cloud_tasks,screenshot}`                     | Auto BP/Match、对局上传队列、SignalR 反代、多杀自动截图       |
| 配置与更新       | `config.rs`, `runtime.rs`, `update.rs`                 | `views/settings.rs`, `services/logging.rs`, `app_window/f_updates.rs` | 配置持久化、便携模式检测、minisign 验签自动更新、日志轮转     |

## Agent 后台任务

| Agent        | 触发事件                         | 功能                                                                                 |
| :----------- | :------------------------------- | :----------------------------------------------------------------------------------- |
| `auto_bp`    | `/lol-champ-select/v1/session`   | 自动选人/禁人/设置召唤师技能/自动同意换人换楼/板凳席处理                             |
| `auto_match` | `gameflow-phase` + `ready-check` | 自动接受匹配/接受邀请/自动点赞/延时再来一局/自动重连/标记玩家提醒 + 对局结束触发上传 |

## 对局数据层 (GameInfo)

对局信息的数据管线按「核心纯逻辑 / 实时编排 / GPUI 视图」分层：

| 模块                                                                   | 职责                                                              |
| :--------------------------------------------------------------------- | :---------------------------------------------------------------- |
| `crates/yuumi-core/src/gameflow/team_builder.rs`                       | 阵营判定（含脱敏推断）、gameflow → 队伍成员映射、占位与组队配色   |
| `crates/yuumi-core/src/gameflow/player_identity.rs`                    | 身份门禁 `is_identity_compatible`、槽位英雄继承、组队标识提取     |
| `crates/yuumi-core/src/gameflow/match_stats.rs`                        | 胜率 / KDA / 连胜 / 位置英雄聚合                                  |
| `crates/yuumi-core/src/parsers/game_info.rs`                           | 10 人段位并发查询、宿命对局计算与 `LiveGameInfo` 组装             |
| `crates/yuumi-gpui/src/services/live_lcu.rs`                           | 编排层：阶段联动防抖拉取、LiveClient 阵营探测、状态快照与事件广播 |
| `crates/yuumi-gpui/src/views/game_info.rs` & `app_window/game_info.rs` | GPUI 视图渲染、队伍/全员模式切换与交互事件分派                    |

## 云顶之弈模块 (`crates/yuumi-core/src/parsers/tft.rs` & `crates/yuumi-gpui/src/views/tft.rs`)

- **数据源与缓存**: 段位与战绩走 LCU 本地接口；英雄 / 羁绊 / 装备 / 海克斯强化字典统一从 CDragon 镜像下载并在本地落盘缓存（`tft_catalog.json`）；热门阵容对接 OP.GG MCP（`tft_list_meta_decks`），解析失败统一返回结构化 `TftError`。
- **视图特性**: 支持段位近况、历史对局详情弹窗、OP.GG 热门阵容 4x7 站位棋盘与一键复制阵容代码、海克斯百科检索。

## 战利品系统 (`crates/yuumi-core/src/parsers/loot.rs` & `loot_actions.rs`)

- **库存与精粹**: 实时读取蓝色/橙色/神话精粹余额及各分类战利品碎片列表。
- **批量智能操作**: 支持一键智能批量开箱（遍历传送门、法球、宝箱）、碎片分解、三合一重铸与解锁，单条进度通过 `LiveLcuEvent::LootActionProgress` 实时反馈至界面进度条。

## 上传系统 (`crates/yuumi-core/src/upload.rs`)

- **UploadQueue**: 去重异步队列，后台 Worker 串行上传，单局 30 秒超时。
- **UploadTrigger**: 由 `services/cloud_tasks.rs` 监听阶段转换（`InProgress` → 结束阶段）后自动触发上传。
- **Smart Split**: 当前玩家数据放入 `matchInfo.participants`，其余 9 人放入外层 `participants`。
- **失败落盘与重试**: 网络异常或服务端报错时自动落盘为 `.tmp` 暂存文件，后续在连接就绪或对局结束时自动触发重试。

## 配置文件

位于 `%APPDATA%/Yuumi/config.json`（便携版位于 `data/config.json`），结构（PascalCase）：

- `General` — 客户端路径、启动选项、HTTP 代理 (含旧版 Schema 自动迁移)、日志等级、上传 API 地址、SignalR Hub 配置
- `Personalization` — Mica 效果、DPI 缩放、语言、主题模式、主题色、胜/负/重赛卡片颜色与死亡数高亮色
- `Functions` — 自动化流程开关与分路候选列表 (自动选禁/匹配/重连/荣誉/邀请/再来一局/ARAM报边/标记提醒/多杀截图/隐藏侧栏页签)
- `Other` — 公告记录、搜索历史

## 开发工具与调试日志

- **调试输出**: Debug 构建下，`services/logging.rs` 会将 `Info` 及以上日志同步输出到 `stderr`；GPUI 渲染性能与界面布局异常可通过日志和开发期断点排查。
- **日志系统**: 由 `crates/yuumi-gpui/src/services/logging.rs` 的自研 `log::Log` 实现接管，按天轮转、单文件 2MB 上限、保留 30 天。等级由设置页「日志等级」控制：0=Debug / 1=Info（默认）/ 2=Error。
    - 日志文件输出路径：可执行文件同级 `log/` 目录（便携版即 `Yuumi-portable/log/`）；单文件超限后为 `yuumi_YYYY-MM-DD-N.log`。
    - 排查 LCU 通信或后台 Task 异常时，请检索最新 `.log` 文件中的 `[LCU]`, `[WS]`, `[LiveLcu]`, `[Error]` 标记。
