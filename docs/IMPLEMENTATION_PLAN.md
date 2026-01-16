# DnX Protocol Complete Implementation Plan

## 一、协议全景分析 (Protocol Overview)

基于 `dldrstate.cpp` 和 `medfieldmessages.h` 的逆向分析，DnX 协议包含以下核心交互：

### 1.1 状态机状态 (State Machine States)
```
DLDR_STATE_INVALID      -> 初始无效状态
DLDR_STATE_FW_NORMAL    -> 正常固件下载（Virgin Part）
DLDR_STATE_FW_MISC      -> 杂项固件下载（DnX OS 模式）
DLDR_STATE_FW_WIPE      -> 擦除 IFWI 分区
DLDR_STATE_OS_NORMAL    -> 正常 OS 下载
DLDR_STATE_OS_MISC      -> 杂项 OS 下载
```

### 1.2 完整 ACK 列表及处理逻辑

| ACK Code | ASCII | 处理动作 |
|----------|-------|---------|
| `DFRM` | 0x4446524D | Virgin Part DnX，进入 `FW_NORMAL` 状态 |
| `DxxM` | 0x4478784D | Non-virgin Part DnX，根据 gpflags 选择状态 |
| `DXBL` | 0x4458424C | 发送 DnX 固件数据（FW 或 OS 取决于当前状态） |
| `RUPHS` | 0x5255504853 (5字节) | 发送 FW Update Profile Header Size |
| `RUPH` | 0x52555048 | 发送 FW Update Profile Header |
| `DMIP` | 0x444D4950 | 发送 MIP (Module Info Pointer) |
| `LOFW` | 0x4C4F4657 | 发送第一个 128KB 固件块 |
| `HIFW` | 0x48494657 | 发送第二个 128KB 固件块 |
| `PSFW1` | 0x5053465731 (5字节) | 发送 Primary Security FW 1（分块） |
| `PSFW2` | 0x5053465732 (5字节) | 发送 Primary Security FW 2（分块） |
| `SSFW` | 0x53534657 | 发送 Secondary Security FW |
| `VEDFW` | 0x5645444657 (5字节) | 发送 Video Encoder/Decoder FW |
| `SuCP` | 0x53754350 | 发送 ROM Patch |
| `RESET` | 0x5245534554 (5字节) | FW 下载完成，设备将 GPP Reset |
| `HLT$` | 0x484C5424 | 固件更新成功完成 |
| `HLT0` | 0x484C5430 | 固件文件大小为 0 |
| `MFLD` / `CLVT` | SoC 类型标识 | 平台识别 |
| **OS Recovery** | | |
| `DORM` | 0x444F524D | OS Recovery 模式开始 |
| `OSIP Sz` | 0x4F53495020537A (7字节) | 发送 OSIP 大小 |
| `ROSIP` | 0x524F534950 (5字节) | 发送 OSIP 数据 |
| `RIMG` | 0x52494D47 | 请求 OS 镜像块 |
| `EOIU` | 0x454F4955 | 镜像更新结束 |
| `DONE` | 0x444F4E45 | 全部完成 |
| **错误码** | | |
| `ER00`-`ER25` | | 各种错误代码 |
| `ERRR` | 0x45525252 | 通用错误 |

### 1.3 数据结构

```c
// DnX Header (24 bytes = 0x18)
struct DnxHeader {
    u32 size;           // 固件大小
    u32 checksum;       // CRC32
    u32 reserved[4];    // 保留字段
};

// FW Update Profile Header
// D0 版本: 0x24 bytes
// C0 版本: 0x20 bytes
// 旧版 MFD: 0x1C bytes
struct FwUpdateProfileHeader {
    u32 magic;
    u32 version;
    u32 psfw1_size;     // offset 0x0C
    u32 psfw2_size;     // offset 0x10
    u32 ssfw_size;      // offset 0x14
    u32 rom_patch_size; // offset 0x18
    // ...
};

// OSIP Partition Table (512 bytes = 0x200)
struct OsipPartitionTable {
    u32 signature;
    u32 size;           // offset 0x04
    u32 num_pointers;   // offset 0x08
    // ...
    // OS N size at offset: (n * 0x18) + 0x30
};
```

## 二、软件架构设计 (Software Architecture)

### 2.1 分层架构

```
┌─────────────────────────────────────────────────────────────┐
│                    Presentation Layer                       │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────────────┐ │
│  │   CLI   │  │   TUI   │  │   GUI   │  │ (Future: WASM)  │ │
│  └────┬────┘  └────┬────┘  └────┬────┘  └────────┬────────┘ │
│       │            │            │                │          │
├───────┴────────────┴────────────┴────────────────┴──────────┤
│                    Observer Trait (Events)                   │
│           ProgressEvent / LogEvent / ErrorEvent              │
├─────────────────────────────────────────────────────────────┤
│                    Application Layer                         │
│  ┌──────────────────────────────────────────────────────────┐│
│  │              DnxSession (Orchestrator)                   ││
│  │  - manages state transitions                             ││
│  │  - emits events via Observer                             ││
│  └──────────────────────────────────────────────────────────┘│
├─────────────────────────────────────────────────────────────┤
│                    Domain Layer (Protocol)                   │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐│
│  │ AckHandler │ │ FwPayload  │ │ OsPayload  │ │ Chunks     ││
│  └────────────┘ └────────────┘ └────────────┘ └────────────┘│
├─────────────────────────────────────────────────────────────┤
│                    Transport Layer (USB)                     │
│  ┌──────────────────────────────────────────────────────────┐│
│  │               UsbTransport Trait                         ││
│  │  - fn write(&self, data: &[u8]) -> Result<usize>         ││
│  │  - fn read(&self, len: usize) -> Result<Vec<u8>>         ││
│  │  - fn read_ack(&self) -> Result<AckCode>                 ││
│  └──────────────────────────────────────────────────────────┘│
│  ┌─────────────────────┐  ┌─────────────────────────────────┐│
│  │ NusbTransport       │  │ MockTransport (for testing)     ││
│  │ (prod implementation│  │                                 ││
│  └─────────────────────┘  └─────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

### 2.2 核心 Traits (接口定义)

```rust
/// UI 层订阅的事件
pub enum DnxEvent {
    DeviceConnected { vid: u16, pid: u16 },
    DeviceDisconnected,
    StateChanged { from: DnxState, to: DnxState },
    Progress { phase: String, current: u64, total: u64 },
    Log { level: LogLevel, message: String },
    Error { code: u32, message: String },
    Complete,
}

/// UI 层实现此 Trait 以接收事件
pub trait DnxObserver: Send + Sync {
    fn on_event(&self, event: DnxEvent);
}

/// USB 传输层抽象
pub trait UsbTransport: Send + Sync {
    fn write(&self, data: &[u8]) -> Result<usize>;
    fn read(&self, len: usize) -> Result<Vec<u8>>;
    fn read_ack(&self) -> Result<AckCode>;
    fn is_connected(&self) -> bool;
}
```

### 2.3 模块划分

```
crates/dnx-core/
├── src/
│   ├── lib.rs                 // 公开 API
│   ├── protocol/
│   │   ├── mod.rs
│   │   ├── ack.rs             // ACK 解析与匹配
│   │   ├── constants.rs       // 魔数常量
│   │   ├── header.rs          // DnxHeader, ProfileHeader 结构
│   │   └── checksum.rs        // CRC32 计算
│   ├── transport/
│   │   ├── mod.rs
│   │   ├── traits.rs          // UsbTransport trait
│   │   ├── nusb.rs            // nusb 实现
│   │   └── mock.rs            // 测试用 Mock
│   ├── payload/
│   │   ├── mod.rs
│   │   ├── firmware.rs        // FW 镜像解析与分块
│   │   └── os.rs              // OS 镜像解析 (OSIP)
│   ├── state/
│   │   ├── mod.rs
│   │   ├── machine.rs         // 状态机核心逻辑
│   │   └── handlers.rs        // 各 ACK 处理器
│   ├── session.rs             // DnxSession 编排器
│   ├── events.rs              // DnxEvent, DnxObserver
│   └── error.rs               // 自定义错误类型
```

## 三、实施路线图 (Implementation Roadmap)

### Phase 1: 核心协议层 (2-3 天)
1. [x] 常量定义 (`protocol/constants.rs`) - 已完成基础
2. [ ] ACK 解析器优化 (`protocol/ack.rs`) - 支持变长 ACK
3. [ ] 头结构定义 (`protocol/header.rs`)
4. [ ] Transport Trait 抽象 (`transport/traits.rs`)
5. [ ] nusb 实现重构 (`transport/nusb.rs`)
6. [ ] Mock Transport (`transport/mock.rs`)

### Phase 2: 固件/镜像处理 (2 天)
1. [ ] FW 镜像解析 (`payload/firmware.rs`)
   - DnX Header 解析
   - Profile Header 解析
   - 128KB 分块逻辑
2. [ ] OS 镜像处理 (`payload/os.rs`)
   - OSIP 解析
   - 镜像分块

### Phase 3: 状态机完善 (2-3 天)
1. [ ] 完整状态定义 (`state/machine.rs`)
2. [ ] 所有 ACK Handler 实现 (`state/handlers.rs`)
3. [ ] 设备重枚举处理

### Phase 4: 事件系统与 UI 层 (1-2 天)
1. [ ] Event/Observer 系统 (`events.rs`)
2. [ ] Session 编排器 (`session.rs`)
3. [ ] CLI 重构 (`apps/cli`)

### Phase 5: 测试与文档 (持续)
1. [ ] Mock Transport 单元测试
2. [ ] 集成测试（需要真实设备）
3. [ ] API 文档完善

## 四、关键设计决策

### 4.1 为什么不用 async？
当前 nusb 的 `list_devices().wait()` 和 `std::io::Read/Write` 都是阻塞的。
在没有强制要求并发处理多设备的场景下，同步代码更简单、调试更容易。
未来如果需要 GUI 响应式或多设备并行，可以：
- 将阻塞操作包装在 `spawn_blocking` 中
- 或使用 `tokio::sync::mpsc` 将 I/O 放到后台线程

### 4.2 为什么用 Observer 而不是 channel？
Observer Trait 更灵活：
- CLI 可以直接打印日志
- TUI 可以更新进度条 Widget
- GUI 可以发送到 UI 线程
- WASM 可以调用 JS callback

### 4.3 错误处理策略
使用 `thiserror` 定义领域错误，最终在 Application 层用 `anyhow` 包装。
