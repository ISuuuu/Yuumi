# Yuumi

Tauri v2 + Vue 3 + TypeScript 桌面应用。
原版 Python (Seraphine) 项目的 Rust 重构。

## 技术栈

- **前端**: Vue 3 + TypeScript + Vite + Zustand + vue-router
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
│   ├── App.vue                     # 根组件（导航栏 + 路由切换）
│   ├── main.ts                     # 入口
│   ├── api/
│   │   └── lcu.ts                  # LCU API 封装 + Rust 命令调用
│   ├── store/
│   │   └── lcuStore.ts             # Zustand 全局状态（LCU 事件映射）
│   ├── views/
│   │   ├── Home.vue                # 首页（LCU 状态 + 快捷导航）
│   │   ├── Career.vue              # 生涯战绩（召唤师 + 对局历史）
│   │   ├── Settings.vue            # 设置（头像/签名/在线状态）
│   │   └── Tools.vue               # 工具箱（创建房间/ARAM 摇号）
│   └── components/
│       └── Greet.vue               # 示例组件
├── src-tauri/                      # Tauri/Rust 后端
│   ├── src/
│   │   ├── main.rs                 # Rust 入口
│   │   ├── lib.rs                  # AppState、命令注册、agent 启动
│   │   ├── config.rs               # 配置读写（%APPDATA%/Yuumi/config.json）
│   │   ├── tools.rs                # 杂项工具（创建房间/ARAM 摇号）
│   │   ├── lcu/
│   │   │   ├── mod.rs
│   │   │   ├── monitor.rs          # LCU 进程轮询（sysinfo 提取 port/token）
│   │   │   ├── client.rs           # HTTPS 代理（忽略 SSL + Basic Auth）
│   │   │   └── ws.rs               # WebSocket 事件订阅 → 广播前端 + 分发 agents
│   │   ├── parsers/
│   │   │   ├── mod.rs
│   │   │   ├── summoner.rs         # 召唤师数据清洗
│   │   │   └── match_parser.rs     # 战绩数据清洗（parseGameData）
│   │   └── agents/
│   │       ├── mod.rs
│   │       ├── auto_bp.rs          # 自动选人/禁人/召唤师技能
│   │       └── auto_match.rs       # 自动接受匹配/自动重连
│   ├── tauri.conf.json
│   ├── capabilities/
│   └── Cargo.toml
├── File/                           # 重构参考文档
│   ├── tauri_reconstruction_spec.md
│   └── tauri_reconstruction_context.md
├── CLAUDE.md
├── package.json
├── vite.config.ts
└── tsconfig.json
```

## 架构数据流

```
LeagueClientUx.exe
  ↓ (sysinfo 轮询 port/token)
monitor.rs → AppState.lcu_client
  ↓
ws.rs → LCU WebSocket 事件
  ├─→ 前端: Tauri emit → lcuStore.ts → Vue 组件
  ├─→ BP Agent: mpsc → auto_bp.rs (自动选人/禁人/技能)
  └─→ Match Agent: mpsc → auto_match.rs (自动接受/重连)
```

## Rust Tauri 命令清单

| 命令 | 来源 | 说明 |
|:---|:---|:---|
| `call_lcu_api` | lcu/client.rs | 通用 LCU API 转发 |
| `get_current_summoner` | parsers/summoner.rs | 获取召唤师信息 |
| `get_match_history` | parsers/match_parser.rs | 获取战绩列表 |
| `create_5v5_practice_lobby` | tools.rs | 创建自定义房间 |
| `aram_reroll_and_swap_back` | tools.rs | 大乱斗摇号换回 |

新命令需在 `lib.rs` 的 `invoke_handler` 中注册。

## Agent 后台任务

| Agent | 触发事件 | 功能 |
|:---|:---|:---|
| auto_bp | `/lol-champ-select/v1/session` | 自动选人/禁人/设置召唤师技能 |
| auto_match | gameflow-phase + ready-check | 自动接受匹配/自动重连 |

## 配置文件

位于 `%APPDATA%/Yuumi/config.json`，结构：
- `General` — 启动、代理、日志
- `Personalization` — 主题、语言、颜色
- `Functions` — 所有自动化功能开关和候选列表
- `Other` — 其他杂项

## 开发工具

开发模式下自动打开 DevTools（见 `lib.rs` 的 `setup` 闭包）。
