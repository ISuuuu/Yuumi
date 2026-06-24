# Yuumi

Tauri v2 + Vue 3 + TypeScript 桌面应用。

## 技术栈

- **前端**: Vue 3 + TypeScript + Vite
- **后端**: Tauri v2 (Rust)
- **包管理**: pnpm

## 常用命令

```bash
# 开发（同时启动 Vite dev server + Tauri 窗口）
pnpm tauri dev

# 构建生产包
pnpm tauri build

# 仅前端开发
pnpm dev

# 仅前端构建
pnpm build

# 生成图标（从源图片）
pnpm tauri icon <source.png>
```

## 项目结构

```
Yuumi/
├── src/                    # Vue 前端
│   ├── App.vue             # 根组件
│   ├── main.ts             # 入口
│   └── components/         # Vue 组件
├── src-tauri/              # Tauri/Rust 后端
│   ├── src/
│   │   ├── main.rs         # Rust 入口
│   │   └── lib.rs          # Tauri 命令 & 应用配置
│   ├── tauri.conf.json     # Tauri 配置
│   ├── capabilities/       # 权限声明
│   └── Cargo.toml          # Rust 依赖
├── public/                 # 静态资源
├── package.json
├── vite.config.ts
└── tsconfig.json
```

## 调用 Rust 命令

在 Vue 中通过 `@tauri-apps/api/core` 的 `invoke` 调用 Rust 函数：

```ts
// 前端
import { invoke } from "@tauri-apps/api/core";
const result = await invoke<string>("greet", { name: "World" });
```

```rust
// Rust (src-tauri/src/lib.rs)
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}
```

新命令需要在 `lib.rs` 的 `invoke_handler` 中注册：`.invoke_handler(tauri::generate_handler![greet, my_cmd])`

## 窗口 & 开发工具

开发模式下自动打开 DevTools（见 `lib.rs` 的 `setup` 闭包）。
