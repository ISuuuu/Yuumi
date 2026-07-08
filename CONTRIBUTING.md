# 贡献指南 (Contributing to Yuumi)

感谢您有兴趣为 Yuumi 贡献代码！为了保持代码质量，确保您的 Pull Request (PR) 能够顺利合并，请在开始开发前阅读本指南。

## 1. 开发环境准备

本项目基于 **Tauri v2 + Vue 3 + TypeScript** 架构构建。

*   **Node.js**: 推荐 v24+
*   **pnpm**: 推荐 v8+
*   **Rust**: 安装最新的 Stable 工具链。
*   **开发命令**:
    ```bash
    pnpm install        # 安装依赖
    pnpm tauri dev      # 启动开发调试窗口
    ```

## 2. 编码指导原则

为了保持项目代码的整洁和可维护性，请务必遵循以下原则：

1.  **简洁第一**：用最少的代码解决问题。不要添加未要求的“灵活性”或“可配置性”。
2.  **外科手术式修改**：只修改必须改动的代码，不要随意重构、重排或“改进”相邻没有损坏的代码。
3.  **匹配现有风格**：无论是前端 Vue 还是后端 Rust，请遵循项目中已有的命名风格、缩进和架构模式。
4.  **消灭死代码与警告**：不要在代码中遗留无用的导入、未使用的变量或编译警告。

## 3. 提交 PR 前的自测 (重要)

项目配置了自动化的 CI 流水线。当您提交 PR 时，GitHub Actions 会在 Windows 环境下运行完整的类型检查、代码格式化检查与静态分析。

为了确保 CI 顺利通过，请在**提交 PR 之前**在本地运行以下命令进行自测：

```bash
pnpm run check-all
```

该命令相当于依次运行以下三个步骤：
1.  `pnpm run type-check`: 检查 Vue 3 / TypeScript 的类型正确性。
2.  `pnpm run fmt-check`: 检查 Rust 代码的格式是否符合 `rustfmt` 规范。（如不符合，请在本地运行 `cargo fmt` 自动修复）。
3.  `pnpm run clippy`: 运行 Rust 的 `clippy` 静态代码分析，请确保没有引入任何新的警告（Warnings）。

## 4. 单元测试

如果您修改了 `src-tauri/src/parsers/` 下的数据解析逻辑（如战绩清洗、段位信息清洗等），**强烈建议您为其编写或补充单元测试**，并在本地运行 `pnpm run test:rust`（即 `cargo test`）以确保测试通过。
