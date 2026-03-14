# arceos-loadapp

A standalone filesystem-based application loader running on [ArceOS](https://github.com/arceos-org/arceos) unikernel, with all dependencies sourced from [crates.io](https://crates.io). Demonstrates **FAT filesystem initialization, file I/O, and VirtIO block device driver** across multiple architectures.

## What It Does

This application demonstrates the full I/O stack from filesystem down to block device:

1. **VirtIO-blk driver**: Automatically discovered and initialized via PCI bus probing.
2. **FAT filesystem**: Mounted on the VirtIO block device during ArceOS runtime startup.
3. **File read**: Opens `/sbin/origin.bin` from the FAT filesystem and reads its first 64 bytes.
4. **Child task**: Spawns a worker thread that prints the first 8 bytes of the file as hex values.
5. **CFS scheduling**: Uses preemptive CFS scheduler for task management.

### I/O Stack

```
Application (std::fs::File)
    └── axfs (FAT filesystem)
        └── axdriver (VirtIO-blk)
            └── virtio-drivers (PCI transport)
                └── QEMU VirtIO block device
```

## Supported Architectures

| Architecture | Rust Target | QEMU Machine | Platform |
|---|---|---|---|
| riscv64 | `riscv64gc-unknown-none-elf` | `qemu-system-riscv64 -machine virt` | riscv64-qemu-virt |
| aarch64 | `aarch64-unknown-none-softfloat` | `qemu-system-aarch64 -machine virt` | aarch64-qemu-virt |
| x86_64 | `x86_64-unknown-none` | `qemu-system-x86_64 -machine q35` | x86-pc |
| loongarch64 | `loongarch64-unknown-none` | `qemu-system-loongarch64 -machine virt` | loongarch64-qemu-virt |

## Prerequisites

- **Rust nightly toolchain** (edition 2024)
- **QEMU** for target architectures
- **rust-objcopy** (`cargo install cargo-binutils`)

## Quick Start

```bash
cargo install cargo-clone
cargo clone arceos-loadapp
cd arceos-loadapp

# Build and run on RISC-V 64 QEMU (default)
cargo xtask run

# Other architectures
cargo xtask run --arch aarch64
cargo xtask run --arch x86_64
cargo xtask run --arch loongarch64
```

Expected output:

```
Load app from fat-fs ...
fname: /sbin/origin.bin
Wait for workers to exit ...
worker1 checks code:
0x10 0x21 0x32 0x43 0x54 0x65 0x76 0x87
worker1 ok!
Load app from disk ok!
```

## Project Structure

```
app-loadapp/
├── .cargo/
│   └── config.toml       # cargo xtask alias & AX_CONFIG_PATH
├── xtask/
│   └── src/
│       └── main.rs       # build/run tool (FAT32 disk image + QEMU)
├── configs/
│   ├── riscv64.toml
│   ├── aarch64.toml
│   ├── x86_64.toml
│   └── loongarch64.toml
├── src/
│   └── main.rs           # File open/read + worker thread
├── build.rs
├── Cargo.toml
└── README.md
```

## Key Components

| Component | Role |
|---|---|
| `axstd` | ArceOS standard library (`std::fs::File`, `std::io`, `std::thread`) |
| `axfs` | Filesystem module — mounts FAT32 on the VirtIO block device |
| `axdriver` | Device driver framework — VirtIO-blk via PCI bus |
| `axtask` | Task scheduler with CFS algorithm |
| `fatfs` (xtask) | Creates the FAT32 disk image with `/sbin/origin.bin` at build time |

## How the Disk Image Works

The `xtask` tool uses the `fatfs` Rust crate to create a 64MB FAT32 disk image (`target/disk.img`):

1. Allocates a 64MB raw file
2. Formats it as FAT32 using `fatfs::format_volume()`
3. Creates `/sbin/` directory
4. Writes `/sbin/origin.bin` with 64 bytes of sample binary data
5. Attaches the image to QEMU as `-device virtio-blk-pci`

No external tools (`mkfs.fat`, `mtools`) are required.


## Exercise
### Requirements
Based on the `arceos-loadapp` kernel component and the reference code under the `exercise` directory, implement a kernel component named `arceos-loadapp-ramfs--rename` that supports two operations: `rename` and `mv`.

Within the kernel, the following similar operations can be completed:
```
mkdir dira
rename dira dirb
echo "hello" > a.txt
rename a.txt b.txt
mv b.txt ./dirb
ls ./dirb
```

### Expectation
```
[Ramfs-Rename]: ok!
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
| 8 | **arceos-loadapp** (this crate) | FAT filesystem initialization and file I/O, demonstrating the full I/O stack from VirtIO block device to filesystem |
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
