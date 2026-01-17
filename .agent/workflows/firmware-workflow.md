---
description: Complete workflow for firmware analysis and development
---

# dnx-rs 开发和固件分析工作流程

本文档描述了使用 xtask 进行固件分析和开发的完整工作流程。

## 常用命令速查

```bash
# 构建项目
cargo xtask build
cargo xtask build --release
cargo xtask build --target cli

# 运行测试
cargo xtask test
cargo xtask test --unit
cargo xtask test --integration

# 代码质量检查
cargo xtask check
cargo xtask check --fix

# 生成文档
cargo xtask doc
cargo xtask doc --open
```

## 固件分析工作流程

### 1. 列出可用固件

```bash
// turbo
cargo xtask firmware list
```

### 2. 验证固件完整性

```bash
// turbo
cargo xtask firmware validate eaglespeak
cargo xtask firmware validate blackburn
cargo xtask firmware validate /path/to/firmware.bin
```

### 3. 分析固件结构

```bash
// turbo
cargo xtask analyze assets/firmware/eaglespeak/dnx_fwr.bin
cargo run -p dnx-cli -- analyze <file>
```

### 4. 比较两个固件

```bash
// turbo
cargo xtask firmware compare file1.bin file2.bin
cargo xtask firmware compare file1.bin file2.bin --detailed
```

### 5. 提取固件组件

```bash
// turbo
cargo xtask firmware extract <source.bin> -o <output_dir>
cargo xtask firmware extract <source.bin> -c token    # 只提取 token
cargo xtask firmware extract <source.bin> -c chaabi   # 只提取 chaabi
cargo xtask firmware extract <source.bin> -c ifwi     # 只提取 ifwi
cargo xtask firmware extract <source.bin> -c all      # 提取所有组件
```

### 6. 提取 IFWI 版本信息

```bash
// turbo
cargo xtask ifwi-version <ifwi_image.bin>
cargo xtask ifwi-version <file> --format json
cargo xtask ifwi-version <file> --format markdown
```

## 开发工作流程

### 新功能开发

1. 设置开发环境：
```bash
// turbo
cargo xtask setup
```

2. 开发并检查代码：
```bash
// turbo
cargo xtask check --fix
```

3. 运行测试：
```bash
// turbo
cargo xtask test
```

4. 构建发布版本：
```bash
// turbo
cargo xtask build --release
```

### 运行 CLI

```bash
// turbo
cargo xtask run -p eaglespeak
cargo xtask run -- --fw-dnx path/to/dnx.bin --os-image path/to/os.img
```

## DnX 设备交互工作流程

### 完整刷机流程

1. 确保固件就绪：
```bash
cargo xtask firmware validate eaglespeak
```

2. 将设备进入 DnX 模式（按住音量下 + 电源键）

3. 运行下载：
```bash
cargo run -p dnx-cli -- -p eaglespeak -v
```

### 调试模式

```bash
RUST_LOG=debug cargo run -p dnx-cli -- -p eaglespeak 2>&1 | tee dnx.log
```

## 固件结构参考

### dnx_fwr.bin 布局

```
0x00000 - 0x00080: 文件头 (128 bytes)
0x00080 - 0x00088: $DnX 标记 (8 bytes)
0x00088 - 0x00188: RSA-2048 签名 (256 bytes)
0x00188 - Token起始: VRL/SCU 代码 (~19KB)
Token起始 - CH00-0x80: Token 区域 (16KB)
CH00-0x80 - CDPH: Chaabi FW (~72KB)
CDPH - 文件末尾: CDPH 区域 + 填充 + Footer
```

### 关键魔术字符串

| 标记 | 描述 |
|------|------|
| `$DnX` | DnX 签名标记 |
| `$CHT` | TNG A0 Token 标记 |
| `CH00` | Chaabi FW 开始 |
| `CDPH` | Chaabi FW 结束 |
| `$FIP` | FIP 版本块（完整 IFWI 镜像）|

## 清理

```bash
// turbo
cargo xtask clean
cargo xtask clean --all  # 包括日志文件
```
