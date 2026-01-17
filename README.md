# DnX-rs

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)](https://www.rust-lang.org)

**DnX-rs** 是一个使用 Rust 编写的高性能协议工具，旨在替代过时的 Intel [xFSTK](https://github.com/xfstk/xfstk) 工具链。它通过纯 Rust 实现的 USB DnX 协议，为 Intel Medfield/Merrifield/Moorefield 平台（如 ASUS ZenFone 系列）提供可靠的底层固件刷写与恢复功能。

---

## 项目特性

- **纯 Rust 实现**: 消除对旧版 Qt 和 libusb 的依赖，提供跨平台的原生性能。
- **现代化 UI**: 同时支持极简的命令行界面 (CLI) 和交互式的终端 UI (TUI)。
- **深度固件分析**: 集成 FUPH/DnX 解析逻辑，支持 RSA 签名校验、标记扫描及 Chaabi 固件提取。
- **观察者模式架构**: 核心逻辑与 UI 层完全解耦，支持实时进度反馈和详细的协议监控。
- **自动化开发流**: 通过自定义 `xtask` 提供构建、测试、镜像分析及模板生成等完整的开发自动化支持。

## 文档指南

项目文档遵循明确的分层结构，以便于不同需求的读者查阅：

1.  **[ROADMAP.md](docs/ROADMAP.md)**: 项目进度、特性规划及优先级安排。
2.  **[ARCHITECTURE.md](docs/ARCHITECTURE.md)**: 系统分层架构、Trait 定义及状态机实现细节。
3.  **[PROTOCOL.md](docs/PROTOCOL.md)**: 基于逆向工程还原的 DnX 协议技术规格与魔数参考。
4.  **[固件分析报告](assets/firmware/README.md)**: 针对特定硬件平台的固件深度剖析报告。

## 快速开始

### 开发环境准备

确保已安装最新的 Rust 稳定版工具链：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 构建与运行

本项目使用 `cargo xtask` 代理常规任务：

```bash
# 启动交互式 TUI
cargo xtask run --tui

# 使用 CLI 执行固件下载
cargo xtask run --cli -- --fw-dnx path/to/dnx_fwr.bin --os-image path/to/dnx_osr.img

# 执行固件深度分析
cargo xtask analyze path/to/firmware.bin
```

## 项目结构

- `crates/dnx-core`: 核心协议栈，包含状态机、USB 传输抽象及固件解析逻辑。
- `apps/cli`: 轻量化命令行入口。
- `apps/tui`: 基于 Ratatui 的全功能图形化终端界面。
- `xtask`: 统一的任务自动化中心。
- `assets/`: 固件资产与历史分析报告。

---

*本项目仅供研究与学习使用。刷写固件存在风险，请确保您了解相关操作的影响。*
