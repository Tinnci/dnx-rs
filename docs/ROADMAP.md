# DnX-rs Roadmap and Feature Planning

## 当前架构回顾

```
dnx-rs/
├── crates/
│   └── dnx-core/           # 核心库
│       ├── events.rs       # 事件/观察者
│       ├── fuph.rs         # FUPH 头解析 (NEW)
│       ├── ifwi_version.rs # IFWI 版本提取
│       ├── payload.rs      # 固件载荷处理
│       ├── protocol.rs     # DnX 协议常量
│       ├── session.rs      # 会话管理
│       ├── state/          # 状态机
│       └── transport/      # USB 传输
├── apps/
│   ├── cli/                # 命令行工具
│   └── tui/                # 终端 UI
└── xtask/                  # 开发任务自动化
```

## 功能集成矩阵

| Feature | xtask | CLI | TUI | dnx-core |
|------|-------|-----|-----|----------|
| Firmware Analysis | [Done] analyze | [Done] analyze | [Planned] | [Done] FuphHeader |
| IFWI Version | [Done] ifwi-version | [Done] ifwi-version | [Planned] | [Done] ifwi_version |
| Firmware Extract | [Done] firmware extract | [No] | [No] | [No] |
| Firmware Compare | [Done] firmware compare | [No] | [No] | [No] |
| Firmware Validate | [Done] firmware validate | [No] | [No] | [No] |
| DnX Download | [Done] run | [Done] download | [Done] | [Done] Session |
| Build/Test | [Done] build/test | [No] | [No] | [No] |
| Code Quality | [Done] check | [No] | [No] | [No] |

## TUI 改进计划

### 当前 TUI 功能
- 配置管理
- DnX 会话控制
- 日志显示
- 进度条

### 建议添加的功能

#### 1. 固件信息面板
```
┌─ Firmware Info ──────────────────────────────────────┐
│ Profile: eaglespeak (Z3580)                          │
│ ┌─ dnx_fwr.bin ─────────────┐ ┌─ dnx_osr.img ──────┐ │
│ │ Size: 109,812 bytes       │ │ Size: 12.58 MB     │ │
│ │ RSA: [Success] Intel Signed   │ │ Type: Android Boot │ │
│ │ Token: $CHT (TNG A0)      │ │ Kernel: ~5 MB      │ │
│ │ Chaabi: 72 KB             │ │ Ramdisk: ~7 MB     │ │
│ └───────────────────────────┘ └────────────────────┘ │
└──────────────────────────────────────────────────────┘
```

#### 2. 版本信息显示
```
┌─ Version Info ───────────────────────────────────────┐
│ IFWI:    0094.0171   │ SCU:     00B0.0032            │
│ Chaabi:  0058.0501   │ IA32:    0003.0001            │
│ mIA:     00B0.3130   │ Hooks:   005E.002C            │
└──────────────────────────────────────────────────────┘
```

#### 3. 比较视图
```
┌─ Firmware Comparison ────────────────────────────────┐
│ eaglespeak vs blackburn                              │
│ ├─ RSA Signature: [Identical]                        │
│ ├─ Structure: [Identical]                            │
│ ├─ Token diff: 282 bytes (0.31%)                     │
│ └─ CDPH CRC: [Different]                             │
└──────────────────────────────────────────────────────┘
```

#### 4. 实时协议视图
```
┌─ Protocol Monitor ───────────────────────────────────┐
│ → DNER (handshake)                                   │
│ ← DxxM (non-virgin)                                  │
│ → DnX Header: Size=109812, GP=0, CS=0x1ACF4          │
│ ← DCFI00 (Chaabi request)                            │
│ → Chaabi: 90136 bytes                                │
│ [Running] Waiting for ACK...                          │
└──────────────────────────────────────────────────────┘
```

## 流程优化建议

### 1. 统一的固件分析 API

```rust
// dnx-core/src/firmware.rs
pub struct FirmwareAnalysis {
    pub path: PathBuf,
    pub size: u64,
    pub file_type: FirmwareType,
    pub version: Option<FirmwareVersions>,
    pub fuph: Option<FuphHeader>,
    pub markers: Vec<MarkerInfo>,
    pub rsa_signature: Option<RsaSignature>,
    pub validation: ValidationResult,
}

impl FirmwareAnalysis {
    pub fn analyze(path: &Path) -> Result<Self>;
    pub fn to_json(&self) -> String;
    pub fn to_markdown(&self) -> String;
}
```

### 2. xtask 与 TUI 共享代码

```
xtask --------------+
                    |---> dnx-core (共享分析逻辑)
TUI ----------------+
```

将固件分析逻辑移至 `dnx-core`，而非 xtask 独立实现。

### 3. 配置文件支持

```toml
# .dnx/profiles/eaglespeak.toml
[profile]
name = "eaglespeak"
description = "Asus ZenFone 2 (Z3580)"
processor = "Z3580"

[firmware]
fw_dnx = "assets/firmware/eaglespeak/dnx_fwr.bin"
os_image = "assets/firmware/eaglespeak/dnx_osr.img"

[expected]
token_marker = "$CHT"
rsa_hash = "0bda531fdad65dab..."
```

### 4. 工作流程改进

```
当前流程:
  用户 → CLI/TUI → dnx-core → 设备

改进流程:
  用户 → CLI/TUI
           ↓
        xtask (开发任务)
           ↓
        dnx-core (核心逻辑)
           ↓
        设备 / 文件分析
```

## 实现优先级

### P0 - Critical
1. [Done] FUPH Parser
2. [Done] Unified Firmware Analysis
3. [Planned] TUI Firmware Info Panel

### P1 - Important
4. [Done] Move firmware analysis to dnx-core
5. [Planned] TUI Version Info Display
6. [Done] Config File Support (.dnx/profiles/ or --config)

### P2 - Enhancement
7. [Done] TUI Real-time Protocol Monitor
8. [Planned] TUI Firmware Comparison View
9. [Done] xtask automated test workflow
10. [Planned] GitHub Actions CI/CD

## 下一步行动

1. **移动固件分析到 dnx-core**
   - 创建 `crates/dnx-core/src/firmware.rs`
   - 实现 `FirmwareAnalysis` 结构
   - xtask 和 CLI 调用此 API

2. **增强 TUI**
   - 添加固件信息面板
   - 显示版本信息
   - 协议状态监控

3. **配置文件**
   - 支持 TOML 配置
   - 自动发现 .dnx/profiles/

---

*规划文档 - 2026-01-17*
