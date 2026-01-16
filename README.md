# DnX-rs: Intel DnX Protocol Implementation in Rust

## 项目简介
`dnx-rs` 是一个用 Rust 编写的开源工具，旨在重写和替代过时的 [xFSTK](http://sourceforge.net/projects/xfstk/) 工具链。它专注于通过 USB DnX 协议与 Intel Medfield/Merrifield/Moorefield 平台（如 ASUS ZenFone 系列）进行底层通信。

## 与 xFSTK 的关系
本项目**不是** xFSTK 的封装，而是对其通信协议的全新**实现**。
- **协议来源**: 分析 xFSTK 源码 (`medfield` 平台部分) 还原出的魔数和状态机。
- **架构**: 纯 Rust 异步/同步 IO，移除对 Qt 和过时 GUI 库的依赖。
- **目标**: 提供一个跨平台、无依赖、易于调试的 CLI/TUI 工具。

## 为什么不将 xFSTK 作为 Submodule？
在这个项目中，我们**不推荐**将原始 xFSTK 仓库作为 git submodule 引入，原因如下：
1.  **代码解耦**: 我们只需要参考其协议逻辑（已提取到 `docs/PROTOCOL.md`），不需要构建或链接其 C++ 代码。
2.  **依赖臃肿**: xFSTK 依赖旧版 Qt 和 libusb，作为 submodule 会污染我们的纯 Rust 开发环境。
3.  **遗留资产**: xFSTK 包含大量非核心平台的代码（如 Clovertrail/Moorefield 的旧驱动），对于专注于现代 Rust 实现不仅无用甚至是干扰。

## 目录结构
- `crates/dnx-core`: 核心协议栈（无 UI，纯逻辑）。
- `apps/cli`: 命令行入口。
- `xtask`: 项目自动化构建脚本。
- `docs/`: 协议文档与逆向工程笔记。

## 快速开始
```bash
# 构建
cargo xtask build

# 运行 (需要 USB 权限)
sudo ./target/debug/dnx --fw-dnx path/to/dnx_fwr.bin
```
