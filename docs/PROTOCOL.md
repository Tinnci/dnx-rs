# DnX Protocol Reference

基于 xFSTK 源码分析的协议细节。

## 1. 通信阶段 (Stages)

通信过程严格遵循 Intel ROM -> FW -> OS 的引导链。

### Stage 1: ROM DnX (Silicon Stage)
- **设备状态**: 变砖，黑屏。CPU 仅 SRAM 可用。
- **VID/PID**: `8086:0Axx` (例如 `0A14`)
- **目标**: 将 `dnx_fwr.bin` (Firmware DnX) 加载到缓存执行。
- **握手流程**:
    1. Host 发送 `DnER` (Download Executes ROM)。
    2. Device 回复 `DFRM` (Download Firmware)。
    3. Host 发送 dnx_fwr.bin 大小和数据。

### Stage 2: FW DnX (Initialized Stage)
- **设备状态**: dnx_fwr 已运行，DRAM 初始化完成。设备可能会重新枚举。
- **VID/PID**: 变为 FW 阶段 PID。
- **目标**: 发送后续的大型镜像 (如 `ifwi.bin` 或 `droidboot.img`)。
- **握手流程**:
    1. Host 发送 `DnER`。
    2. Device 回复 `DXBL` (Download Execute Bootloader) 或其他 ACK。
    3. Host 发送 OS 镜像。

## 2. 关键魔数 (Magic Numbers)

摘录自 `crates/dnx-core/src/protocol.rs`

| 助记符 | 16进制 (Little Endian) | ASCII | 含义 |
| :--- | :--- | :--- | :--- |
| **Preamble** | | | |
| `PREAMBLE_DNER` | `0x52456E44` | `DnER` | Host -> Device: 启动握手 |
| **ACKs** | | | |
| `BULK_ACK_DFRM` | `0x4446524D` | `DFRM` | Device -> Host: 请求 DnX 固件 |
| `BULK_ACK_DXBL` | `0x4458424C` | `DXBL` | Device -> Host: 准备好接收 Bootloader |
| `BULK_ACK_RUPH` | `0x52555048` | `RUPH` | Ready for Update Profile Header |

## 3. 数据包格式
大多数传输是纯二进制流 (Bulk Transfer)。
- **Header**: 通常是 4 字节的 Preamble 或 ACK。
- **Payload**: 紧随其后的二进制文件内容。
- **Size**: 某些阶段需要先发送文件大小 (u32)。

## 4. 状态机逻辑
参考 `crates/dnx-core/src/logic.rs` 中的 `run_state_machine`。
核心是通过 `read_ack` 不断轮询设备状态，并根据返回的 ACK (如 `DFRM`) 决定下一步发送哪个文件。
