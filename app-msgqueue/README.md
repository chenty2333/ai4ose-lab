# arceos-msgqueue

A standalone message-queue application running on [ArceOS](https://github.com/arceos-org/arceos) unikernel, with all dependencies sourced from [crates.io](https://crates.io). Demonstrates **cooperative multi-task scheduling** with a producer-consumer message queue and PFlash MMIO access across multiple architectures.

## What It Does

This application demonstrates cooperative scheduling and inter-task communication:

1. **Message queue**: A `VecDeque` protected by `SpinNoIrq` (interrupt-safe spinlock) serves as the shared message queue between tasks.
2. **Producer task** (worker1): Pushes numbered messages (0..64) into the queue, calling `thread::yield_now()` after each push to cooperatively hand over the CPU.
3. **Consumer task** (worker2): Pops messages from the queue, printing each one. When the queue is empty, it yields the CPU back to the producer.
4. **Cooperative scheduling**: Tasks voluntarily yield via `thread::yield_now()`, demonstrating the basic cooperative scheduling algorithm — without yielding, a task runs until completion.
5. **PFlash MMIO**: Before starting the message queue, the main task verifies PFlash access via page-table-mapped MMIO.

## PFlash Address Map

| Architecture | PFlash Unit | Physical Address | QEMU Option |
|---|---|---|---|
| riscv64 | pflash1 | `0x22000000` | `-drive if=pflash,unit=1` |
| aarch64 | pflash1 | `0x04000000` | `-drive if=pflash,unit=1` |
| x86_64 | pflash0 | `0xFFC00000` | `-drive if=pflash,unit=0` (with embedded SeaBIOS) |
| loongarch64 | pflash1 | `0x1D000000` | `-drive if=pflash,unit=1` |

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

- **SeaBIOS** (required for x86_64 only)

  ```bash
  # Ubuntu/Debian
  sudo apt install seabios
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
# get source code of arceos-msgqueue crate from crates.io
cargo clone arceos-msgqueue
# into crate dir
cd arceos-msgqueue
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

Expected output (abbreviated):

```
Multi-task message queue is starting ...
PFlash check: [0xFFFFFFC022000000] -> PFLA
Wait for workers to exit ...
worker1 (producer) ...
worker1 [0]
worker2 (consumer) ...
worker2 [0]
worker1 [1]
worker2 [1]
...
worker1 [64]
worker1 ok!
worker2 [64]
worker2 ok!
Multi-task message queue OK!
```

QEMU will automatically exit after printing the message.

## Project Structure

```
app-msgqueue/
├── .cargo/
│   └── config.toml       # cargo xtask alias & AX_CONFIG_PATH
├── xtask/
│   └── src/
│       └── main.rs       # build/run tool (pflash image creation + QEMU launch)
├── configs/
│   ├── riscv64.toml      # Platform config with PFlash MMIO range
│   ├── aarch64.toml
│   ├── x86_64.toml
│   └── loongarch64.toml
├── src/
│   └── main.rs           # Producer-consumer message queue with cooperative scheduling
├── build.rs              # Linker script path setup (auto-detects arch)
├── Cargo.toml            # Dependencies (axstd with paging + multitask features)
└── README.md
```

## How It Works

The `cargo xtask` pattern uses a host-native helper crate (`xtask/`) to orchestrate
cross-compilation and QEMU execution:

1. **`cargo xtask build --arch <ARCH>`**
   - Copies `configs/<ARCH>.toml` to `.axconfig.toml`
   - Runs `cargo build --release --target <TARGET> --features axstd`

2. **`cargo xtask run --arch <ARCH>`**
   - Performs the build step above
   - Creates a PFlash image with magic string `"PFLA"` at offset 0
   - For x86_64: embeds SeaBIOS at the end of the pflash image
   - Converts ELF to raw binary via `rust-objcopy` (except x86_64)
   - Launches QEMU with the PFlash image attached

## Key Components

| Component | Role |
|---|---|
| `axstd` | ArceOS standard library (replaces Rust's `std` in `no_std` environment) |
| `axhal` | Hardware abstraction layer, provides `phys_to_virt` for address translation |
| `axtask` | Task scheduler with run queues, enables `thread::spawn` / `thread::join` / `thread::yield_now` |
| `axsync` | Synchronization primitives, provides `SpinNoIrq` for the message queue |
| `paging` feature | Enables page table management; maps MMIO regions listed in config |
| `multitask` feature | Enables multi-task scheduler with cooperative scheduling support |

## Exercise
### Requirements
Based on the `arceos-msgqueue` kernel component and the reference code under the `exercise` directory, implement a new kernel functional component for the memory allocator named `bump_allocator` (based on the `bump` memory allocation algorithm), as well as the corresponding kernel component `arceos-msgqueue-alt-alloc`. You are required to modify the implementation of the `exercise/modules/bump_allocator` component as much as possible to support the `bump` memory allocation algorithm, and modify other parts as little as possible.

### Expectation
```
Running bump tests...
Bump tests run OK!
```

### Tips
1. You can refer to the existing page allocator and byte allocator to implement the corresponding Traits.
2. This `bump_allocator` acts as both a byte allocator and a page allocator. Therefore, it must implement three Traits: `BaseAllocator`, `ByteAllocator`, and `PageAllocator` simultaneously. This is different from the existing references.


## ArceOS Tutorial Crates

This crate is part of a series of tutorial crates for learning OS development with [ArceOS](https://github.com/arceos-org/arceos). The crates are organized by functionality and complexity progression:

| # | Crate Name | Description |
|:---:|---|---|
| 1 | [arceos-helloworld](https://crates.io/crates/arceos-helloworld) | Minimal ArceOS unikernel application that prints Hello World, demonstrating the basic boot flow |
| 2 | [arceos-collections](https://crates.io/crates/arceos-collections) | Dynamic memory allocation on a unikernel, demonstrating the use of String, Vec, and other collection types |
| 3 | [arceos-readpflash](https://crates.io/crates/arceos-readpflash) | MMIO device access via page table remapping, reading data from QEMU's PFlash device |
| 4 | [arceos-childtask](https://crates.io/crates/arceos-childtask) | Multi-tasking basics: spawning a child task (thread) that accesses a PFlash MMIO device |
| 5 | **arceos-msgqueue** (this crate) | Cooperative multi-task scheduling with a producer-consumer message queue, demonstrating inter-task communication |
| 6 | [arceos-fairsched](https://crates.io/crates/arceos-fairsched) | Preemptive CFS scheduling with timer-interrupt-driven task switching, demonstrating automatic task preemption |
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
- **#8–#10 (Monolithic Kernel Stage)**: Building on the unikernel foundation, these crates add user/kernel privilege separation, page fault handling, and ELF loading, progressively evolving toward a monolithic kernel.
- **#11–#14 (Hypervisor Stage)**: Starting from minimal VM lifecycle management, these crates progressively add address space management, virtual devices, timer injection, and ultimately run a full monolithic kernel inside a virtual machine.

## License

GPL-3.0-or-later OR Apache-2.0 OR MulanPSL-2.0
