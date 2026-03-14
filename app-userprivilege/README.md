# arceos-userprivilege

A standalone monolithic kernel application running on [ArceOS](https://github.com/arceos-org/arceos), demonstrating **user-privilege mode execution**: loading a minimal user-space binary, switching the CPU to unprivileged mode, and handling syscalls when the user program requests kernel services. All dependencies are sourced from [crates.io](https://crates.io). Supports four processor architectures.

## What It Does

This application demonstrates the fundamental OS mechanism of **privilege separation** -- running code in unprivileged (user) mode and trapping back to the kernel on syscalls:

1. **Address space creation** (`main.rs`): Creates an isolated user address space with `AddrSpace::new_empty()`, then copies the kernel page table entries so kernel code remains accessible during traps.
2. **Binary loading** (`loader.rs`): Reads a raw binary (`/sbin/origin`) from a FAT32 virtual disk and copies it to a fixed user-space address (`0x1000`).
3. **User stack allocation** (`main.rs`): Allocates a 64 KiB user stack at the top of the user address space with `SharedPages` backend.
4. **User-mode execution** (`task.rs`): Spawns a kernel task that creates a `UserContext`, switches to the user page table, and enters user mode via `UserContext::run()`. A trap dispatch loop handles `ReturnReason::Syscall` and other events.
5. **Syscall handling** (`syscall.rs`): Intercepts `SYS_EXIT` (syscall 93) from user space, prints a message, and terminates the task with the provided exit code.

### The User-Space Payload

The payload is a minimal `no_std` Rust binary that simply invokes the `SYS_EXIT` syscall with exit code 0:

```rust
#[unsafe(no_mangle)]
unsafe extern "C" fn _start() -> ! {
    // riscv64 example:
    core::arch::asm!(
        "li a7, 93",   // SYS_EXIT
        "ecall",
        options(noreturn)
    );
}
```

Each architecture uses its own inline assembly to issue the syscall (`ecall` on riscv64, `svc #0` on aarch64, `syscall` on x86_64, `syscall 0` on loongarch64). The payload is compiled for the target bare-metal architecture, converted to a raw binary with `rust-objcopy`, and packaged into a FAT32 disk image as `/sbin/origin`.


### Relationship to other crates in this series

| Crate | Key Feature | Builds On |
|---|---|---|
| **`arceos-userprivilege`** (this) | User/kernel privilege separation, basic syscall | -- |
| `arceos-lazymapping` | Demand paging (lazy page fault handling) | userprivilege |
| `arceos-runlinuxapp` | Run real Linux ELF binaries (musl libc) | lazymapping |

## Supported Architectures

| Architecture | Rust Target | QEMU Machine | Platform |
|---|---|---|---|
| riscv64 | `riscv64gc-unknown-none-elf` | `qemu-system-riscv64 -machine virt` | riscv64-qemu-virt |
| aarch64 | `aarch64-unknown-none-softfloat` | `qemu-system-aarch64 -machine virt` | aarch64-qemu-virt |
| x86_64 | `x86_64-unknown-none` | `qemu-system-x86_64 -machine q35` | x86-pc |
| loongarch64 | `loongarch64-unknown-none` | `qemu-system-loongarch64 -machine virt` | loongarch64-qemu-virt |

## Prerequisites

### 1. Rust nightly toolchain (edition 2024)

```bash
rustup install nightly
rustup default nightly
```

### 2. Bare-metal targets (install the ones you need)

```bash
rustup target add riscv64gc-unknown-none-elf
rustup target add aarch64-unknown-none-softfloat
rustup target add x86_64-unknown-none
rustup target add loongarch64-unknown-none
```

### 3. rust-objcopy (from `cargo-binutils`, required for non-x86_64 targets)

```bash
cargo install cargo-binutils
rustup component add llvm-tools
```

### 4. QEMU (install the emulators for your target architectures)

```bash
# Ubuntu 24.04
sudo apt update
sudo apt install qemu-system-riscv64 qemu-system-arm \
                 qemu-system-x86 qemu-system-misc

# macOS (Homebrew)
brew install qemu
```

> Note: On Ubuntu, `qemu-system-aarch64` is provided by `qemu-system-arm`, and `qemu-system-loongarch64` is provided by `qemu-system-misc`.

### Summary of required packages (Ubuntu 24.04)

```bash
# All-in-one install for Ubuntu 24.04
sudo apt update
sudo apt install -y \
    build-essential \
    qemu-system-riscv64 \
    qemu-system-arm \
    qemu-system-x86 \
    qemu-system-misc

# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup install nightly && rustup default nightly
rustup target add riscv64gc-unknown-none-elf aarch64-unknown-none-softfloat \
                  x86_64-unknown-none loongarch64-unknown-none
cargo install cargo-binutils
rustup component add llvm-tools
```

## Quick Start

```bash
# Install cargo-clone sub-command
cargo install cargo-clone
# Get source code of arceos-userprivilege crate from crates.io
cargo clone arceos-userprivilege
# Enter crate directory
cd arceos-userprivilege

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

### What `cargo xtask run` does

The `xtask` command automates the full workflow:

1. **Install config** -- copies `configs/<arch>.toml` to `.axconfig.toml`
2. **Build payload** -- compiles `payload/` Rust crate for the bare-metal target, then `rust-objcopy` converts the ELF to a raw binary
3. **Create disk image** -- builds a 64 MB FAT32 image containing `/sbin/origin`
4. **Build kernel** -- `cargo build --release --target <target> --features axstd`
5. **Objcopy** -- converts kernel ELF to raw binary (non-x86_64 only)
6. **Run QEMU** -- launches the emulator with VirtIO block device attached

### Expected output

```
handle_syscall ...
[SYS_EXIT]: process is exiting ..
monolithic kernel exit [0] normally!
```

QEMU will automatically exit after the kernel prints the final message.

## Project Structure

```
app-userprivilege/
├── .cargo/
│   └── config.toml          # cargo xtask alias & AX_CONFIG_PATH
├── xtask/
│   └── src/
│       └── main.rs           # Build/run tool: payload compilation, disk image, QEMU
├── configs/
│   ├── riscv64.toml          # Platform config (MMIO, memory layout, etc.)
│   ├── aarch64.toml
│   ├── x86_64.toml
│   └── loongarch64.toml
├── payload/
│   ├── Cargo.toml            # Minimal no_std binary crate
│   ├── linker.ld             # Linker script (entry at 0x1000)
│   └── src/
│       └── main.rs           # User-space: SYS_EXIT(0) via inline assembly
├── src/
│   ├── main.rs               # Kernel entry: create address space, load app, spawn task
│   ├── loader.rs             # Raw binary loader (read from FAT32, copy to 0x1000)
│   ├── syscall.rs            # Syscall handler (SYS_EXIT only)
│   └── task.rs               # User task spawning & trap dispatch loop
├── build.rs                  # Linker script path setup (auto-detects arch)
├── Cargo.toml                # Dependencies from crates.io
├── rust-toolchain.toml       # Nightly toolchain & bare-metal targets
└── README.md
```

## Key Components

| Component | Role |
|---|---|
| `axstd` | ArceOS standard library (replaces Rust's `std` in `no_std` environment) |
| `axhal` | Hardware Abstraction Layer -- `UserContext`, `ReturnReason`, trap handling |
| `axmm` | Memory management -- user address spaces, page mapping with `SharedPages` backend |
| `axtask` | Task scheduler -- kernel task spawning, CFS scheduling, context switching |
| `axfs` / `axfeat` | Filesystem -- FAT32 virtual disk access for loading the user binary |
| `axio` | I/O traits (`Read`) for file operations |
| `axlog` | Kernel logging (`ax_println!`) |
| `memory_addr` | Virtual/physical address types and alignment utilities |

## How the Privilege Transition Works

```
Kernel (supervisor/ring 0)

 1. Create AddrSpace::new_empty()
 2. copy_mappings_from(kernel_aspace)
 3. Load /sbin/origin at VA 0x1000
 4. Map user stack
 5. UserContext::new(entry=0x1000, sp=stack_top, 0)
 6. uctx.run()  ─────────────────────────────┐
                                              │
 8. ReturnReason::Syscall             ◄───────┤
 9. handle_syscall -> SYS_EXIT(0)             │
10. axtask::exit(0)                           │
                                              ▼
                              ┌─────────────────────────┐
                              │  User mode (ring 3)     │
                              │                         │
                              │  7. ecall / svc /       │
                              │     syscall             │
                              └─────────────────────────┘
```

## ArceOS Tutorial Crates

This crate is part of a series of tutorial crates for learning OS development with [ArceOS](https://github.com/arceos-org/arceos). The crates are organized by functionality and complexity progression:

| # | Crate Name | Description |
|:---:|---|---|
| 1 | [arceos-helloworld](https://crates.io/crates/arceos-helloworld) | Minimal ArceOS unikernel application that prints Hello World, demonstrating the basic boot flow |
| 2 | [arceos-collections](https://crates.io/crates/arceos-collections) | Dynamic memory allocation on a unikernel, demonstrating the use of String, Vec, and other collection types |
| 3 | [arceos-readpflash](https://crates.io/crates/arceos-readpflash) | MMIO device access via page table remapping, reading data from QEMU's PFlash device |
| 4 | [arceos-childtask](https://crates.io/crates/arceos-childtask) | Multi-tasking basics: spawning a child task (thread) that accesses a PFlash MMIO device |
| 5 | [arceos-msgqueue](https://crates.io/crates/arceos-msgqueue) | Cooperative multi-task scheduling with a producer-consumer message queue, demonstrating inter-task communication |
| 6 | [arceos-fairsched](https://crates.io/crates/arceos-fairsched) | Preemptive CFS scheduling with timer-interrupt-driven task switching, demonstrating automatic task preemption |
| 7 | [arceos-readblk](https://crates.io/crates/arceos-readblk) | VirtIO block device driver discovery and disk I/O, demonstrating device probing and block read operations |
| 8 | [arceos-loadapp](https://crates.io/crates/arceos-loadapp) | FAT filesystem initialization and file I/O, demonstrating the full I/O stack from VirtIO block device to filesystem |
| 9 | **arceos-userprivilege** (this crate) | User-privilege mode switching: loading a user-space program, switching to unprivileged mode, and handling syscalls |
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
