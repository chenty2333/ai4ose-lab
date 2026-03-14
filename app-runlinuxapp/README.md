# arceos-runlinuxapp

A standalone monolithic kernel application running on [ArceOS](https://github.com/arceos-org/arceos), capable of loading and executing **real Linux ELF binaries** (compiled with [musl libc](https://musl.libc.org/)) in user space. All kernel dependencies are sourced from [crates.io](https://crates.io). Demonstrates ELF parsing, user-space address space management, privilege mode switching, and Linux syscall handling across multiple architectures.

## What It Does

This application builds a minimal monolithic kernel that runs a statically-linked Linux C program (`hello.c`) in user mode:

1. **ELF loading** (`loader.rs`): Parses the ELF64 header and program headers of `/sbin/hello` from a FAT32 virtual disk, loads all `PT_LOAD` segments into a newly created user address space.
2. **User stack initialization** (`main.rs`): Allocates a 64 KiB user stack and sets up the initial stack frame with `argc`, `argv`, `envp`, and `auxv` following the Linux ELF ABI — required by musl libc's `_start` routine.
3. **User-mode execution** (`task.rs`): Spawns a kernel task that enters user mode via `UserContext::run()`, then loops to handle traps (syscalls, interrupts, exceptions) and re-enter user mode.
4. **Syscall handling** (`syscall.rs`): Intercepts Linux syscalls from the user binary and handles them in the kernel, including:
   - `SYS_SET_TID_ADDRESS` — thread ID registration (musl startup)
   - `SYS_IOCTL` — terminal query stub (musl stdio init)
   - `SYS_WRITEV` — write output to the kernel console (used by `puts()`)
   - `SYS_EXIT` / `SYS_EXIT_GROUP` — process termination
   - `SYS_ARCH_PRCTL` — x86_64 TLS setup (`ARCH_SET_FS`)

### The User-Space Payload

The payload is a minimal C program compiled with musl libc and statically linked:

```c
#include <stdio.h>

int main()
{
    puts("Hello, UserApp!\n");
    return 0;
}
```

The `xtask` tool automatically compiles it with the appropriate musl cross-compiler for each target architecture, strips the binary, and packages it into a FAT32 disk image as `/sbin/hello`.

## Supported Architectures

| Architecture | Rust Target | QEMU Machine | Platform | Musl Prefix |
|---|---|---|---|---|
| riscv64 | `riscv64gc-unknown-none-elf` | `qemu-system-riscv64 -machine virt` | riscv64-qemu-virt | `riscv64-linux-musl` |
| aarch64 | `aarch64-unknown-none-softfloat` | `qemu-system-aarch64 -machine virt` | aarch64-qemu-virt | `aarch64-linux-musl` |
| x86_64 | `x86_64-unknown-none` | `qemu-system-x86_64 -machine q35` | x86-pc | `x86_64-linux-musl` |
| loongarch64 | `loongarch64-unknown-none` | `qemu-system-loongarch64 -machine virt` | loongarch64-qemu-virt | `loongarch64-linux-musl` |

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

### 5. Musl cross-compilation toolchains

The user-space payload (`hello.c`) must be compiled with a musl-based cross-compiler for each target architecture. The `xtask` tool searches for `<arch>-linux-musl-gcc` in your `$PATH`.

**Option A: Download prebuilt toolchains** (recommended)

Download from <https://musl.cc/> or build with [musl-cross-make](https://github.com/richfelker/musl-cross-make):

```bash
# Example: download and install riscv64 musl cross-compiler
wget https://musl.cc/riscv64-linux-musl-cross.tgz
tar xzf riscv64-linux-musl-cross.tgz -C /opt/
export PATH="/opt/riscv64-linux-musl-cross/bin:$PATH"

# Repeat for other architectures as needed:
# aarch64-linux-musl-cross.tgz
# x86_64-linux-musl-cross.tgz
# loongarch64-linux-musl-cross.tgz
```

**Option B: Install via system package manager (Ubuntu 24.04, x86_64 and aarch64 only)**

```bash
sudo apt install musl-tools           # provides musl-gcc (x86_64 host only)
sudo apt install gcc-aarch64-linux-gnu  # for aarch64 (glibc, not musl)
```

> Note: System packages typically only cover x86_64 musl. For riscv64 and loongarch64, prebuilt toolchains from musl.cc are required.

**Verify installation:**

```bash
# Check that the cross-compiler is available
riscv64-linux-musl-gcc --version
aarch64-linux-musl-gcc --version
x86_64-linux-musl-gcc --version
loongarch64-linux-musl-gcc --version
```

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

# Musl cross-compilers (download from https://musl.cc/)
# Extract to /opt/ or ~/toolchains/ and add to PATH
```

## Quick Start

```bash
# Install cargo-clone sub-command
cargo install cargo-clone
# Get source code of arceos-runlinuxapp crate from crates.io
cargo clone arceos-runlinuxapp
# Enter crate directory
cd arceos-runlinuxapp

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

1. **Install config** — copies `configs/<arch>.toml` → `.axconfig.toml`
2. **Build payload** — compiles `payload/hello.c` with `<arch>-linux-musl-gcc -static`, strips the binary
3. **Create disk image** — builds a 64 MB FAT32 image containing `/sbin/hello`
4. **Build kernel** — `cargo build --release --target <target> --features axstd`
5. **Objcopy** — converts kernel ELF to raw binary (non-x86_64 only)
6. **Run QEMU** — launches the emulator with VirtIO block device attached

### Expected output (riscv64)

```
handle_syscall [96] ...
handle_syscall [29] ...
Unimplemented syscall: SYS_IOCTL
handle_syscall [66] ...
Hello, UserApp!
handle_syscall [66] ...

handle_syscall [94] ...
[SYS_EXIT_GROUP]: system is exiting ..
monolithic kernel exit [Some(0)] normally!
```

On x86_64, the syscall numbers differ (218, 158, 16, 20, 231) but the behavior is identical.

QEMU will automatically exit after the kernel prints the final message.

## Project Structure

```
app-runlinuxapp/
├── .cargo/
│   └── config.toml          # cargo xtask alias & AX_CONFIG_PATH
├── xtask/
│   └── src/
│       └── main.rs           # Build/run tool: musl compilation, disk image, QEMU
├── configs/
│   ├── riscv64.toml          # Platform config (MMIO, memory layout, etc.)
│   ├── aarch64.toml
│   ├── x86_64.toml
│   └── loongarch64.toml
├── payload/
│   └── hello.c               # User-space C program (musl libc)
├── src/
│   ├── main.rs               # Kernel entry: load ELF, init user stack, spawn task
│   ├── loader.rs             # ELF64 parser & PT_LOAD segment loader
│   ├── syscall.rs            # Linux syscall handler (writev, exit, etc.)
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
| `axhal` | Hardware Abstraction Layer — `UserContext`, trap handling, page tables |
| `axmm` | Memory management — user address spaces, page mapping with `SharedPages` backend |
| `axtask` | Task scheduler — kernel task spawning, CFS scheduling, context switching |
| `axfs` / `axfeat` | Filesystem — FAT32 virtual disk access for loading the ELF binary |
| `axio` | I/O traits (`Read`, `Seek`) for file operations |
| `axsync` | Synchronization primitives |
| `axlog` | Kernel logging (`ax_println!`, `ax_print!`) |
| `axerrno` | Linux error codes for syscall return values |
| `memory_addr` | Virtual/physical address types and alignment utilities |
| `kernel-elf-parser` | Constructs the initial user stack (argc/argv/envp/auxv) per Linux ABI |

## Architecture-Specific Notes

### x86_64

- **`SYS_ARCH_PRCTL`** (syscall 158): Musl libc uses `arch_prctl(ARCH_SET_FS, addr)` to set up Thread-Local Storage (TLS). The handler writes the address to `UserContext.fs_base`.
- **16-byte stack alignment**: The CPU enforces 16-byte RSP alignment when delivering interrupts from ring 3 → ring 0. `AlignedUserContext` (`#[repr(C, align(16))]`) prevents triple faults.
- **QEMU uses ELF directly**: Unlike other architectures, `qemu-system-x86_64` boots the kernel ELF directly (no `rust-objcopy` needed).

### aarch64

- **FP/SIMD access**: Musl-compiled binaries use floating-point instructions. `CPACR_EL1.FPEN` must be set to allow FP/SIMD access from EL0 before entering user mode.

### Syscall number mapping

| Syscall | riscv64/aarch64/loongarch64 | x86_64 |
|---|---|---|
| `set_tid_address` | 96 | 218 |
| `ioctl` | 29 | 16 |
| `writev` | 66 | 20 |
| `exit` | 93 | 60 |
| `exit_group` | 94 | 231 |
| `arch_prctl` | — | 158 |


## Exercise
### Requirements
Based on the `arceos-runlinuxapp` kernel component and the reference code under the `exercise` directory, implement a kernel component named `arceos-runlinux-with-sys-map` that implements the `sys_mmap` system call (the system call number varies across different processors; for riscv64, the sys_mmap system call number is 222 in decimal and 0xde in hexadecimal). This component should support file mapping to make the test cases pass.

Additional Requirements:
1. Implement the `sys_mmap` system call to ensure the test cases pass.

### Expectation
Taking riscv64 as an example:
```
handle_syscall [222] ...
paddr: PA:0x....
...
MapFile ok!
...
monolithic kernel exit [Some(0)] normally!
```

### Tips
You can refer to the code segment for loading applications into the user address space in `arceos-userprivilege` kernel component.

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
| 9 | [arceos-userprivilege](https://crates.io/crates/arceos-userprivilege) | User-privilege mode switching: loading a user-space program, switching to unprivileged mode, and handling syscalls |
| 10 | [arceos-lazymapping](https://crates.io/crates/arceos-lazymapping) | Lazy page mapping (demand paging): user-space program triggers page faults, and the kernel maps physical pages on demand |
| 11 | **arceos-runlinuxapp** (this crate) | Loading and running real Linux ELF applications (musl libc) on ArceOS, with ELF parsing and Linux syscall handling |
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
