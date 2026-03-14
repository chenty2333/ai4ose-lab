# arceos-fairsched

A standalone preemptive scheduling application running on [ArceOS](https://github.com/arceos-org/arceos) unikernel, with all dependencies sourced from [crates.io](https://crates.io). Demonstrates **preemptive CFS (Completely Fair Scheduler)** with timer-interrupt-driven task switching across multiple architectures.

## What It Does

This application demonstrates preemptive scheduling and inter-task communication:

1. **CFS scheduling**: The `sched-cfs` feature enables the Completely Fair Scheduler, which assigns CPU time slices based on virtual runtime fairness.
2. **Timer interrupts**: The `sched-cfs` feature implies `irq`, enabling hardware timer interrupts that trigger preemption — tasks are switched automatically without explicit `yield_now()` calls.
3. **Producer task** (worker1): Pushes 257 messages (0..=256) into a shared queue **without yielding**. The CFS scheduler preempts it via timer interrupts.
4. **Consumer task** (worker2): Pops messages from the queue. Only yields when the queue is empty.
5. **SpinNoIrq lock**: The shared `VecDeque` is protected by an interrupt-safe spinlock to prevent deadlocks during preemption.

### Cooperative vs Preemptive

| | `arceos-msgqueue` (cooperative) | `arceos-fairsched` (preemptive) |
|---|---|---|
| Scheduler | FIFO | CFS |
| Producer yields? | Yes (`yield_now()`) | No (preempted by timer) |
| Task switching | Voluntary | Automatic (timer IRQ) |
| Fairness | Manual | Automatic (vruntime) |

## Supported Architectures

| Architecture | Rust Target | QEMU Machine | Platform |
|---|---|---|---|
| riscv64 | `riscv64gc-unknown-none-elf` | `qemu-system-riscv64 -machine virt` | riscv64-qemu-virt |
| aarch64 | `aarch64-unknown-none-softfloat` | `qemu-system-aarch64 -machine virt` | aarch64-qemu-virt |
| x86_64 | `x86_64-unknown-none` | `qemu-system-x86_64 -machine q35` | x86-pc |
| loongarch64 | `loongarch64-unknown-none` | `qemu-system-loongarch64 -machine virt` | loongarch64-qemu-virt |

## Prerequisites

- **Rust nightly toolchain** (edition 2024)

  ```bash
  rustup install nightly
  rustup default nightly
  ```

- **Bare-metal targets** (install the ones you need)

  ```bash
  rustup target add riscv64gc-unknown-none-elf
  rustup target add aarch64-unknown-none-softfloat
  rustup target add x86_64-unknown-none
  rustup target add loongarch64-unknown-none
  ```

- **QEMU** (install the emulators for your target architectures)

  ```bash
  # Ubuntu/Debian
  sudo apt install qemu-system-riscv64 qemu-system-aarch64 \
                   qemu-system-x86 qemu-system-loongarch64  # OR qemu-system-misc

  # macOS (Homebrew)
  brew install qemu
  ```

- **rust-objcopy** (from `cargo-binutils`, required for non-x86_64 targets)

  ```bash
  cargo install cargo-binutils
  rustup component add llvm-tools
  ```

## Quick Start

```bash
# install cargo-clone sub-command
cargo install cargo-clone
# get source code of arceos-fairsched crate from crates.io
cargo clone arceos-fairsched
# into crate dir
cd arceos-fairsched
# Build and run on RISC-V 64 QEMU (default)
cargo xtask run

# Build and run on other architectures
cargo xtask run --arch aarch64
cargo xtask run --arch x86_64
cargo xtask run --arch loongarch64

# Build only (no QEMU)
cargo xtask build --arch riscv64
cargo xtask build --arch aarch64
```

Expected output (abbreviated, interleaving depends on timer preemption):

```
Multi-task(Preemptible) is starting ...
Wait for workers to exit ...
worker1 ... ThreadId(4)
worker1 [0]
worker1 [1]
...
worker1 [42]
worker2 ... ThreadId(5)
worker2 [0]
worker2 [1]
...
worker1 ok!
worker2 [256]
worker2 ok!
Multi-task(Preemptible) ok!
```

Note: The exact interleaving varies across runs and architectures because it depends on timer interrupt timing.

QEMU will automatically exit after printing the message.

## Project Structure

```
app-fairsched/
├── .cargo/
│   └── config.toml       # cargo xtask alias & AX_CONFIG_PATH
├── xtask/
│   └── src/
│       └── main.rs       # build/run tool (QEMU launch, no pflash)
├── configs/
│   ├── riscv64.toml      # Platform config
│   ├── aarch64.toml
│   ├── x86_64.toml
│   └── loongarch64.toml
├── src/
│   └── main.rs           # Preemptive producer-consumer with CFS
├── build.rs              # Linker script path setup (auto-detects arch)
├── Cargo.toml            # Dependencies (axstd with sched-cfs feature)
└── README.md
```

## Key Components

| Component | Role |
|---|---|
| `axstd` | ArceOS standard library (replaces Rust's `std` in `no_std` environment) |
| `axtask` | Task scheduler with CFS algorithm, run queues, and preemption support |
| `axsync` | Synchronization primitives, provides `SpinNoIrq` for the message queue |
| `sched-cfs` feature | Enables CFS (Completely Fair Scheduler) with timer-interrupt-driven preemption |
| `multitask` feature | Enables multi-task support with `thread::spawn` / `thread::join` |
| `paging` feature | Enables page table management for MMIO region mapping |

## ArceOS Tutorial Crates

This crate is part of a series of tutorial crates for learning OS development with [ArceOS](https://github.com/arceos-org/arceos). The crates are organized by functionality and complexity progression:

| # | Crate Name | Description |
|:---:|---|---|
| 1 | [arceos-helloworld](https://crates.io/crates/arceos-helloworld) | Minimal ArceOS unikernel application that prints Hello World, demonstrating the basic boot flow |
| 2 | [arceos-collections](https://crates.io/crates/arceos-collections) | Dynamic memory allocation on a unikernel, demonstrating the use of String, Vec, and other collection types |
| 3 | [arceos-readpflash](https://crates.io/crates/arceos-readpflash) | MMIO device access via page table remapping, reading data from QEMU's PFlash device |
| 4 | [arceos-childtask](https://crates.io/crates/arceos-childtask) | Multi-tasking basics: spawning a child task (thread) that accesses a PFlash MMIO device |
| 5 | [arceos-msgqueue](https://crates.io/crates/arceos-msgqueue) | Cooperative multi-task scheduling with a producer-consumer message queue, demonstrating inter-task communication |
| 6 | **arceos-fairsched** (this crate) | Preemptive CFS scheduling with timer-interrupt-driven task switching, demonstrating automatic task preemption |
| 7 | [arceos-readblk](https://crates.io/crates/arceos-readblk) | VirtIO block device driver discovery and disk I/O, demonstrating device probing and block read operations |
| 8 | [arceos-loadapp](https://crates.io/crates/arceos-loadapp) | FAT filesystem initialization and file I/O, demonstrating the full I/O stack from VirtIO block device to filesystem |
| 9 | [arceos-userprivilege](https://crates.io/crates/arceos-userprivilege) | User-privilege mode switching: loading a user-space program, switching to unprivileged mode, and handling syscalls |
| 10 | [arceos-lazymapping](https://crates.io/crates/arceos-lazymapping) | Lazy page mapping (demand paging): user-space program triggers page faults, and the kernel maps physical pages on demand |
| 11 | [arceos-runlinuxapp](https://crates.io/crates/arceos-runlinuxapp) | Loading and running real Linux ELF applications (musl libc) on ArceOS, with ELF parsing and Linux syscall handling |
| 12 | [arceos-guestmode](https://crates.io/crates/arceos-guestmode) | Minimal hypervisor: creating a guest address space, entering guest mode, and handling a single VM exit (shutdown) |
| 13 | [arceos-guestaspace](https://crates.io/crates/arceos-guestaspace) | Hypervisor address space management: loop-based VM exit handling with nested page fault (NPF) on-demand mapping |
| 14 | [arceos-guestvdev](https://crates.io/crates/arceos-guestvdev) | Hypervisor virtual device support: timer virtualization, console I/O forwarding, and NPF passthrough; guest runs preemptive multi-tasking |
| 15 | [arceos-guestmonolithickernel](https://crates.io/crates/arceos-guestmonolithickernel) | Full hypervisor + guest monolithic kernel: the guest kernel supports user-space process management, syscall handling, and preemptive scheduling |

**Progression Logic:**

- **#1–#8 (Unikernel Stage)**: Starting from the simplest output, these crates progressively introduce memory allocation, device access (MMIO / VirtIO), multi-task scheduling (both cooperative and preemptive), and filesystem support, building up the core capabilities of a unikernel.
- **#9–#11 (Monolithic Kernel Stage)**: Building on the unikernel foundation, these crates add user/kernel privilege separation, page fault handling, and ELF loading, progressively evolving toward a monolithic kernel.
- **#12–#15 (Hypervisor Stage)**: Starting from minimal VM lifecycle management, these crates progressively add address space management, virtual devices, timer injection, and ultimately run a full monolithic kernel inside a virtual machine.

## License

GPL-3.0-or-later OR Apache-2.0 OR MulanPSL-2.0
