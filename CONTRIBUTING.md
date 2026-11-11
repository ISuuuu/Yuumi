# 贡献指南 (Contributing to Yuumi)

感谢您有兴趣为 Yuumi 贡献代码！为了保持代码质量，确保您的 Pull Request (PR) 能够顺利合并，请在开始开发前阅读本指南。

## 1. 开发环境准备

本项目基于 **GPUI + 纯 Rust Cargo Workspace** 架构构建（包含 `crates/yuumi-core` 核心库与 `crates/yuumi-gpui` 原生桌面应用）。

*   **Rust**: 1.77+ 或最新 Stable 工具链（需包含 `rustfmt` 与 `clippy`）
*   **操作系统与工具链**: Windows 10 / 11（支持 Direct3D 11），MSVC 构建工具链
*   **可选 Node.js**: 仅在需要运行 `scripts/capture-lcu-fixtures.mjs` 采集真实 LCU 测试快照时需要
*   **开发命令**:
    ```bash
    cargo run -p yuumi-gpui               # 启动 GPUI 开发调试窗口
    cargo build --release -p yuumi-gpui   # 构建 Release 产物 (target/release/yuumi.exe)
    ```

## 2. 编码指导原则

为了保持项目代码的整洁和可维护性，请务必遵循以下原则：

1.  **简洁第一**：用最少的代码解决问题。不要添加未要求的“灵活性”或“可配置性”。
2.  **外科手术式修改**：只修改必须改动的代码，不要随意重构、重排或“改进”相邻没有损坏的代码。
3.  **匹配现有风格**：请遵循项目中已有的 Rust 命名风格、模块边界（`yuumi-core` 保持无 UI 依赖）以及 GPUI 组件组织规范。
4.  **消灭死代码与警告**：不要在代码中遗留无用的导入、未使用的变量或编译警告。

## 3. 提交 PR 前的自测 (重要)

项目配置了自动化的 CI 流水线。当您提交 PR 时，GitHub Actions 会在 Windows 环境下运行格式检查、静态分析、单元测试与 Release 编译验证。

为了确保 CI 顺利通过，请在**提交 PR 之前**在本地运行以下命令进行自测：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo test --workspace
```

各步骤说明：
1.  `cargo fmt --all -- --check`: 检查 Rust 代码格式是否符合 `rustfmt` 规范（如不符合，请运行 `cargo fmt --all` 自动修复）。
2.  `cargo clippy --workspace --all-targets`: 运行 Rust 的 `clippy` 静态代码分析，请确保没有引入任何新的警告（Warnings）。
3.  `cargo test --workspace`: 运行全工作区单元测试。

> 仓库还提供了 `.githooks/pre-commit` Git 钩子，可在提交时自动对暂存的 `.rs` 文件执行格式化并触发 `clippy` 检查。

## 4. 单元测试与测试快照

如果您修改了 `crates/yuumi-core/src/parsers/` 下的数据解析逻辑（如战绩清洗、段位信息清洗、TFT 解析等），**强烈建议您为其编写或补充单元测试**（优先参考 `crates/yuumi-core/testdata/` 下的 fixture 约定，需要重新采集真实 LCU 响应时可运行 `node scripts/capture-lcu-fixtures.mjs`），并在本地运行 `cargo test --workspace` 确保测试通过。
