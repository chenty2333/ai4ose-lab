# arceos-lazymapping

A standalone monolithic kernel application running on [ArceOS](https://github.com/arceos-org/arceos), demonstrating **lazy page mapping (demand paging)**: physical memory pages for the user stack are pre-allocated but not mapped into the page table until the user program actually accesses them. On first access, a page fault is triggered, and the kernel's fault handler maps the page on-demand. All dependencies are sourced from [crates.io](https://crates.io). Supports four processor architectures.

## What It Does

This application demonstrates **demand paging** — a core OS memory management technique where page table entries are not populated until the corresponding memory is actually accessed:

1. **Address space creation** (`main.rs`): Creates an isolated user address space, copies the kernel page table entries, and loads a minimal user binary.
2. **Lazy stack initialization** (`main.rs` + `task.rs`):
   - Pre-allocates physical pages for the user stack via `SharedPages`.
   - Maps the stack area in the address space with `Backend::new_shared` (which creates page table entries).
   - **Unmaps all page table entries** for the stack area inside the task closure — the physical pages remain allocated but are now invisible to the MMU.
3. **User-mode execution** (`task.rs`): The user binary runs and writes to the stack. Since the page table entries were removed, the CPU raises a **page fault**.
4. **Page fault handling** (`task.rs`): The kernel catches `ReturnReason::PageFault`, verifies the faulting address is within the stack region, looks up the corresponding pre-allocated physical page from `SharedPages`, maps it back into the page table, and resumes execution — all transparently to the user program.
5. **Syscall handling** (`syscall.rs`): After the stack access succeeds, the user binary issues `SYS_EXIT(0)` and the kernel terminates the task.

### The User-Space Payload

The payload is a minimal `no_std` Rust binary that **explicitly touches the stack** (triggering the page fault) before calling `SYS_EXIT`:

```rust
#[unsafe(no_mangle)]
unsafe extern "C" fn _start() -> ! {
    // riscv64 example: write to stack, then exit
    core::arch::asm!(
        "addi sp, sp, -4",   // touch the stack → page fault!
        "sw a0, (sp)",
        "li a7, 93",         // SYS_EXIT
        "ecall",
        options(noreturn)
    );
}
```

Each architecture has its own stack-touching instruction (`push rax` on x86_64, `str x0, [sp]` on aarch64, `st.d $a0, $sp, 0` on loongarch64).


### Relationship to other crates in this series

| Crate | Key Feature | Builds On |
|---|---|---|
| `arceos-userprivilege` | User/kernel privilege separation, basic syscall | — |
| **`arceos-lazymapping`** (this) | Demand paging (lazy page fault handling) | userprivilege |
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
# Get source code of arceos-lazymapping crate from crates.io
cargo clone arceos-lazymapping
# Enter crate directory
cd arceos-lazymapping

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
2. **Build payload** — compiles `payload/` Rust crate for the bare-metal target, then `rust-objcopy` converts the ELF to a raw binary
3. **Create disk image** — builds a 64 MB FAT32 image containing `/sbin/origin`
4. **Build kernel** — `cargo build --release --target <target> --features axstd`
5. **Objcopy** — converts kernel ELF to raw binary (non-x86_64 only)
6. **Run QEMU** — launches the emulator with VirtIO block device attached

### Expected output

```
handle page fault OK!
handle_syscall ...
[SYS_EXIT]: system is exiting ..
monolithic kernel exit [0] normally!
```

The key line is **`handle page fault OK!`** — this confirms that the user stack was lazily mapped on first access. QEMU will automatically exit after the kernel prints the final message.

## Project Structure

```
app-lazymapping/
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
│       └── main.rs           # User-space: touch stack + SYS_EXIT(0)
├── src/
│   ├── main.rs               # Kernel entry: create address space, lazy stack init
│   ├── loader.rs             # Raw binary loader (read from FAT32, copy to 0x1000)
│   ├── syscall.rs            # Syscall handler (SYS_EXIT)
│   └── task.rs               # Task spawning, stack unmap, page fault handler
├── build.rs                  # Linker script path setup (auto-detects arch)
├── Cargo.toml                # Dependencies from crates.io
├── rust-toolchain.toml       # Nightly toolchain & bare-metal targets
└── README.md
```

## Key Components

| Component | Role |
|---|---|
| `axstd` | ArceOS standard library (replaces Rust's `std` in `no_std` environment) |
| `axhal` | Hardware Abstraction Layer — `UserContext`, `ReturnReason::PageFault`, page tables |
| `axmm` | Memory management — `AddrSpace`, `SharedPages` for pre-allocated page pool, `Backend::new_shared` |
| `axtask` | Task scheduler — kernel task spawning, CFS scheduling, context switching |
| `axfs` / `axfeat` | Filesystem — FAT32 virtual disk access for loading the user binary |
| `axio` | I/O traits (`Read`) for file operations |
| `axlog` | Kernel logging (`ax_println!`) |
| `memory_addr` | Virtual/physical address types and alignment utilities |

## How Demand Paging Works

```
┌─────────────────────────────────────────────────────────────┐
│  Kernel (supervisor mode)                                   │
│                                                             │
│  1. Allocate SharedPages (physical pages for stack)          │
│  2. Map stack area with Backend::new_shared (populate=true)  │
│  3. UNMAP all stack page table entries                       │
│     (pages still allocated, just invisible to MMU)           │
│  4. Enter user mode via UserContext::run()                   │
│                                          │                  │
│  6. ReturnReason::PageFault(vaddr)  ◄────┤                  │
│  7. Look up physical page from SharedPages                   │
│  8. page_table.map(vaddr → paddr)                           │
│  9. Resume user mode  ───────────────────┐                  │
│                                          │                  │
│ 11. ReturnReason::Syscall  ◄─────────────┤                  │
│ 12. SYS_EXIT(0) → axtask::exit(0)       │                  │
│                                          ▼                  │
│                            ┌──────────────────────┐         │
│                            │  User mode           │         │
│                            │                      │         │
│                            │  5. addi sp, sp, -4  │         │
│                            │     → PAGE FAULT!    │         │
│                            │                      │         │
│                            │ 10. ecall (SYS_EXIT) │         │
│                            └──────────────────────┘         │
└─────────────────────────────────────────────────────────────┘
```

## Architecture-Specific Notes

### x86_64

- **16-byte stack alignment**: The CPU enforces 16-byte RSP alignment when delivering interrupts from ring 3 → ring 0. `AlignedUserContext` (`#[repr(C, align(16))]`) wraps `UserContext` to prevent triple faults caused by misaligned TSS.RSP0.

### TLB Flush

After unmapping page table entries, the TLB (Translation Lookaside Buffer) may still cache stale mappings. The unmap is performed inside the task closure (after the user page table is active) to ensure the TLB flush takes effect on the correct address space.

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
| 10 | **arceos-lazymapping** (this crate) | Lazy page mapping (demand paging): user-space program triggers page faults, and the kernel maps physical pages on demand |
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
