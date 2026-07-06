# Yuumi

> 英雄联盟辅助工具 · Tauri v2 + Vue 3 + Rust

Yuumi 是一款轻量的英雄联盟客户端辅助工具，通过 LCU API 实现战绩查询、对局分析、自动化操作等功能。

## 功能

- **战绩查询** — 搜索任意召唤师，查看对局历史与详情（10 人数据）
- **生涯** — 当前召唤师的近期战绩概览
- **对局信息** — 实时显示当前对局 10 人段位与近期 KDA
- **TFT** — 云顶之弈数据查看（暂无）
- **自动选人/禁人** — 支持按分路配置候选英雄与召唤师技能
- **自动接受匹配** — 可设置延迟
- **自动重连** — 游戏断线自动重连
- **自动创建大厅** — 空闲时自动创建指定模式房间
- **OP.GG** — 内置 OP.GG 数据查询
- **对局上传** — 游戏结束后自动批量上传战绩（需接口支持）

## 技术栈

| 层级 | 技术 |
|---|---|
| 前端 | Vue 3 + TypeScript + Vite + Pinia + Naive UI |
| 后端 | Tauri v2 (Rust) |
| 通信 | LCU HTTPS + WebSocket (tokio-tungstenite) |
| 打包 | Tauri bundler |

## 开发

### 环境要求

- [Node.js](https://nodejs.org/) >= 18
- [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/tools/install) >= 1.77
- Tauri v2 系统依赖（见 [Tauri 官方文档](https://v2.tauri.app/start/prerequisites/)）

### 启动

```bash
pnpm install        # 安装依赖
pnpm tauri dev      # 开发模式（Vite + Tauri 窗口）
pnpm tauri build    # 构建生产包
```

### 常用命令

```bash
pnpm dev            # 仅前端开发（浏览器）
pnpm build          # 仅前端构建
pnpm tauri dev      # 完整开发（前端 + Rust 后端）
pnpm tauri build    # 生产构建
```

## 项目结构

```
Yuumi/
├── src/                  # Vue 前端
│   ├── views/            # 页面组件（Home/Career/Search/GameInfo/TFT/Tools/Settings/Notice/BenchOverlay）
│   ├── components/       # 通用组件（LcuImage/OpggModal/OpggWindow/ChampionPicker/SpellPicker/NaiveUIBridge）
│   ├── composables/      # 组合式 hooks（useLcuAsset/useToast）
│   ├── api/              # LCU API 封装 + Tauri IPC 调用
│   ├── store/            # Pinia 全局状态
│   ├── utils/            # 工具函数（主题色等）
│   ├── i18n.ts           # 多语言配置
│   ├── main.ts           # 主窗口入口
│   └── opgg.ts           # OP.GG 独立窗口入口
├── src-tauri/            # Rust 后端
│   ├── src/
│   │   ├── lib.rs        # 入口：AppState、命令注册、系统托盘
│   │   ├── config.rs     # 配置读写
│   │   ├── lcu/          # LCU 连接（进程检测、HTTPS 代理、WebSocket、资源预加载）
│   │   ├── parsers/      # 数据清洗（召唤师/战绩/对局信息/TFT）
│   │   ├── agents/       # 自动化任务（BP/匹配/重连）
│   │   ├── upload.rs     # 对局上传队列
│   │   ├── tools.rs      # 杂项工具
│   │   └── signalr.rs    # SignalR Hub 远程反代
│   └── tauri.conf.json   # Tauri 配置
└── File/                 # 重构参考文档（不入库）
```

## 配置

配置文件位于 `%APPDATA%/Yuumi/config.json`，支持：

| 分类 | 内容 |
|---|---|
| General | 客户端路径、启动选项、代理、日志、上传 API、SignalR |
| Personalization | 主题色、语言、胜/败/重赛卡片颜色 |
| Functions | 自动化功能开关（BP/匹配/重连/上传等）+ 候选英雄/技能列表 |
| Other | 公告记录、搜索历史 |

## 致谢

- [Seraphine](https://github.com/Zzaphkiel/Seraphine) — 原版 Python 实现
- [LCU API](https://www.hextechdocs.dev/lol/lcuapi) — 英雄联盟客户端 API 文档

## 许可

仅供学习交流使用。
