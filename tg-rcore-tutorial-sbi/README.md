# tg-rcore-tutorial-sbi

[![Crates.io](https://img.shields.io/crates/v/tg-rcore-tutorial-sbi.svg)](https://crates.io/crates/tg-rcore-tutorial-sbi)
[![Documentation](https://docs.rs/tg-rcore-tutorial-sbi/badge.svg)](https://docs.rs/tg-rcore-tutorial-sbi)
[![License](https://img.shields.io/crates/l/tg-rcore-tutorial-sbi.svg)](LICENSE)

SBI (Supervisor Binary Interface) 调用封装模块，为 rCore 教学操作系统提供 S 态到 M 态/固件的统一调用接口。

## 设计目标

- 为内核提供最小且稳定的 SBI API：输出、定时器、关机。
- 屏蔽“外部 BIOS”与“内置 SBI”两种启动形态差异。
- 在 `no_std` 场景下保持简单可读，方便教学实验逐步扩展。

## 总体架构

- `src/lib.rs`：对 SBI `ecall` 的 Rust 封装与高层函数导出。
- `src/msbi.rs`：`nobios` 模式下的最小 M 态 SBI 实现（软固件）。
- `src/m_entry.asm`：M 态入口与 trap 处理相关汇编。
- `feature = "nobios"`：启用内置 M 态支持，适配 `qemu -bios none`。

## 主要特征

- 支持 Legacy 控制台 I/O（EID `0x01`, `0x02`）。
- 支持 Timer 扩展（EID `0x54494D45`）。
- 支持 System Reset 扩展（EID `0x53525354`）。
- 支持 `no_std`，适合裸机内核。
- `nobios` 下可在无外部 SBI 固件时运行。

## 功能实现要点

- 通过内联汇编触发 `ecall`，遵循 RISC-V SBI 寄存器约定。
- `shutdown(failure)` 统一正常关机与异常关机路径。
- `set_timer(time)` 由 S 态设置下一次时钟中断触发点。
- `nobios` 模式固定面向 QEMU virt 的 MMIO 布局（UART/CLINT/test device）。

## 对外接口

- 函数：
  - `set_timer(u64)`
  - `console_putchar(u8)`
  - `console_getchar() -> usize`
  - `shutdown(bool) -> !`
- 模块（按 feature）：
  - `msbi`（`nobios`）

## 使用示例

```rust
use tg_sbi::{console_putchar, set_timer, shutdown};

console_putchar(b'H');
set_timer(1_000_000);
shutdown(false);
```

- 章节内真实用法：
  - `tg-rcore-tutorial-ch1/src/main.rs` 使用 `console_putchar` 和 `shutdown`。
  - `tg-rcore-tutorial-ch3/src/main.rs` 使用 `set_timer` 做时间片中断。

## 本地修改组件后进行测试验证
建议流程：

1. 在内核功能组件的目录先验证组件本身：
   ```bash
   cd tg-rcore-tutorial-sbi
   cargo check 
   ```
2. 测试依赖该组件的OS，做集成验证：
   ```bash
   cd tg-rcore-tutorial-sbi
   ./systest.sh -l  #测试依赖本地修改后的组件的各个内核是否能通过它们自身测试
   # 如果测试在crates.io上的组件是否能通过测试，则执行  ./systest.sh
   # 可通过修改 systest.txt 和sysdeps.txt 来灵活调整测试对象和测试本地修改的组件
   ```

## 与 tg-rcore-tutorial-ch1~tg-rcore-tutorial-ch8 的关系

- 直接依赖章节：`tg-rcore-tutorial-ch1` 到 `tg-rcore-tutorial-ch8` 全部依赖 `tg-rcore-tutorial-sbi`。
- 关键职责：提供最底层运行时交互（控制台输出、时钟中断、关机）。
- 关键引用文件：
  - `tg-rcore-tutorial-ch1/Cargo.toml`
  - `tg-rcore-tutorial-ch2/src/main.rs`
  - `tg-rcore-tutorial-ch3/src/main.rs`
  - `tg-rcore-tutorial-ch8/src/main.rs`

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
