# arceos-guestvdev

A standalone hypervisor application running on [ArceOS](https://github.com/arceos-org/arceos) unikernel, with all dependencies sourced from [crates.io](https://crates.io). Implements guest virtual device support with **timer virtualization**, **console I/O forwarding**, and **nested page fault (NPF) passthrough** across three architectures.

This crate is derived from the ArceOS hypervisor tutorial in the [ArceOS](https://github.com/arceos-org/arceos) ecosystem, extending it to support multiple processor architectures. The guest runs a preemptive multi-tasking demo as the guest OS.

## What It Does

The hypervisor (`arceos-guestvdev`) performs the following:

1. **Creates a guest address space** with second-stage/nested page tables
2. **Pre-allocates guest RAM** (16 MB on RISC-V, 2 MB on x86_64) to minimize NPF exits
3. **Loads a guest kernel** (`gkernel`) from a VirtIO block device (FAT32 filesystem)
4. **Virtualizes timer device** for guest preemptive scheduling (RISC-V: SBI SetTimer + hvip injection)
5. **Forwards console I/O** via SBI/SVC/VMMCALL hypercalls
6. **Handles nested page faults** with MMIO passthrough mapping
7. **Demonstrates the VM run loop control flow**: loop → guest entry → VM exit → handle → repeat

The guest kernel (`gkernel`) behavior varies by architecture:

- **RISC-V 64**: Full ArceOS multi-tasking demo with CFS scheduler and preemptive scheduling. Two worker threads communicate via a shared queue.
- **AArch64**: Bare-metal EL0 program that tests virtual device interaction (console I/O via SVC, PFlash read via NPF).
- **x86_64**: Bare-metal long-mode program that tests virtual device interaction (console I/O via VMMCALL, PFlash read via NPF).

## Architecture Support

| Architecture | Virtualization | Guest Mode | Virtual Devices | Shutdown Mechanism |
|---|---|---|---|---|
| RISC-V 64 | H-extension (hgatp) | VS-mode | Timer (SBI), Console (SBI PutChar), PFlash (NPF) | SBI `ecall` (Reset) |
| AArch64 | EL1→EL0 (TTBR0) | EL0 | Console (SVC), PFlash (NPF) | SVC hypercall (exit) |
| x86_64 | AMD SVM (NPT) | Long mode | Console (VMMCALL), PFlash (NPF) | `vmmcall` (PSCI) |

> **Note on RISC-V Timer Virtualization**: The hypervisor intercepts SBI SetTimer calls, forwards them to the host SBI (OpenSBI), and injects virtual supervisor timer interrupts to the guest via `hvip`. This enables the guest's CFS scheduler to perform preemptive context switching between worker threads.

> **Note on AArch64**: Because the ArceOS platform crate drops from EL2 to EL1 during boot, the hypervisor runs at EL1 and the guest at EL0. Guest page tables are managed via TTBR0_EL1, and data aborts from EL0 serve as the equivalent of nested page faults.

> **Note on x86_64 AMD SVM**: The hypervisor uses VMRUN/VMEXIT with hardware Nested Page Tables (NPT). Guest GPRs (RCX–R15) are saved/restored by software via an `SvmGuestGprs` structure. PFlash is emulated in software.

## Control Flow

```
Hypervisor starts
    │
    ├─ Create guest address space (AddrSpace)
    ├─ Pre-allocate guest RAM
    ├─ Load guest binary from /sbin/gkernel
    ├─ Setup vCPU context
    │
    └─ VM Run Loop ──────────────────────┐
         │                               │
         ├─ Enter guest (vmrun/eret)     │
         │                               │
         ├─ SBI call (PutChar/SetTimer) │
         │   └─ Forward to host SBI ────┘
         │                               │
         ├─ Timer interrupt              │
         │   └─ Inject to guest (hvip) ──┘
         │                               │
         ├─ Guest accesses unmapped addr │
         │   └─ NPF / Page Fault exit   │
         │       └─ Map the page ────────┘
         │
         ├─ Guest issues shutdown call
         │   └─ Shutdown exit
         │       └─ Break loop
         │
         └─ "Hypervisor ok!"
```

## Comparison with Related Crates

| Crate | Role | Description |
|---|---|---|
| **arceos-guestvdev** (this) | Hypervisor | Runs guest with virtual device support |
| [arceos-guestaspace](https://crates.io/crates/arceos-guestaspace) | Hypervisor | Runs guest with NPF handling |
| [arceos-guestmode](https://crates.io/crates/arceos-guestmode) | Hypervisor | Runs minimal guest, single VM exit |

## Prerequisites

- **Rust nightly toolchain** (edition 2024)

  ```bash
  rustup install nightly
  rustup default nightly
  ```

- **Bare-metal targets**

  ```bash
  rustup target add riscv64gc-unknown-none-elf
  rustup target add aarch64-unknown-none-softfloat
  rustup target add x86_64-unknown-none
  ```

- **QEMU** (with virtualization support)

  ```bash
  # Ubuntu/Debian
  sudo apt install qemu-system-riscv64 qemu-system-aarch64 qemu-system-x86

  # macOS (Homebrew)
  brew install qemu
  ```

- **rust-objcopy** (from `cargo-binutils`)

  ```bash
  cargo install cargo-binutils
  rustup component add llvm-tools
  ```

## Quick Start

```bash
# Install cargo-clone
cargo install cargo-clone

# Get source code from crates.io
cargo clone arceos-guestvdev
cd arceos-guestvdev

# Build and run on RISC-V 64 (default)
cargo xtask run

# Build and run on other architectures
cargo xtask run --arch aarch64
cargo xtask run --arch x86_64

# Build only (no QEMU)
cargo xtask build --arch riscv64
```

## Expected Output

### RISC-V 64

```
Starting virtualization...
Pre-allocating 16 MB guest RAM at 0x80000000...
VM created success, loading images...
app: /sbin/gkernel
Loaded XXXXX bytes from /sbin/gkernel
bsp_entry: 0x80200000; ept: 0x...
Entering VM run loop...
       d8888                            .d88888b.   .d8888b.   (guest ArceOS banner)
       ...
Multi-task(Preemptible) is starting ...
worker1 ... ThreadId(2)
worker1 [0]
worker2 ... ThreadId(3)
worker2: nothing to do!
worker1 [1]
...
worker2 [256]
worker2 ok!
Wait for workers to exit ...
worker1 ok!
Multi-task(Preemptible) ok!
Guest: SBI SRST shutdown
Shutdown vm normally!
```

### AArch64

```
Starting virtualization...
app: /sbin/gkernel
...
Entering VM run loop...
       d8888                            .d88888b.   .d8888b.   (guest ArceOS banner)
       ...
arch = aarch64
platform = aarch64-qemu-virt
smp = 1

Virtual Device (vdev) Test
Reading PFlash at physical address 0x04000000...
Try to access pflash dev region [0x04000000], got 0x646c6670
Got pflash magic: pfld
Shutdown vm normally!
Hypervisor ok!
```

### x86_64 (AMD SVM)

```
Starting virtualization...
Pre-allocating 2048 KB guest RAM at GPA 0x0...
VM created success, loading images...
app: /sbin/gkernel
Loaded XXXX bytes from /sbin/gkernel
Entering VM run loop...

       d8888                            .d88888b.   .d8888b.
       ...

arch = x86_64
platform = x86-pc
smp = 1

Virtual Device (vdev) Test
Reading PFlash at physical address 0xFFC00000...
Try to access pflash dev region [0xFFC00000], got 0x646c6670
Got pflash magic: pfld
Shutdown vm normally!
Hypervisor ok!
```

## Project Structure

```
app-guestvdev/
├── .cargo/
│   └── config.toml            # cargo xtask alias & AX_CONFIG_PATH
├── payload/
│   └── gkernel/               # Guest kernel payload
│       ├── Cargo.toml          #   riscv64: ArceOS multitask; others: bare-metal
│       └── src/main.rs         #   Architecture-specific guest code
├── xtask/
│   └── src/main.rs            # Build/run tool (disk image, pflash, QEMU)
├── configs/
│   ├── riscv64.toml           # Platform config for riscv64-qemu-virt
│   ├── aarch64.toml           # Platform config for aarch64-qemu-virt
│   └── x86_64.toml            # Platform config for x86-pc
├── src/
│   ├── main.rs                # Hypervisor entry: VM exit handling loop
│   ├── loader.rs              # Guest binary loader (FAT32 → address space)
│   ├── vcpu.rs                # RISC-V vCPU context (registers, guest.S)
│   ├── guest.S                # RISC-V guest entry/exit assembly
│   ├── regs.rs                # RISC-V general-purpose registers
│   ├── csrs.rs                # RISC-V hypervisor CSR definitions
│   ├── sbi/                   # SBI message parsing (base, reset, fence, ...)
│   ├── aarch64/               # AArch64 EL1→EL0 vCPU, guest.S, SVC handling
│   └── x86_64/                # AMD SVM: VMCB, GPR save/restore, vmrun assembly
├── build.rs                   # Linker script auto-detection
├── Cargo.toml
├── rust-toolchain.toml
└── README.md
```

## How It Works

### `cargo xtask run --arch <ARCH>`

1. Copies `configs/<ARCH>.toml` → `.axconfig.toml`
2. Builds the guest payload (`gkernel`) for the target architecture
3. Creates a 64MB FAT32 disk image with `/sbin/gkernel`
4. For riscv64/aarch64: creates a pflash image with "pfld" magic at offset 0
5. Builds the hypervisor kernel with `--features axstd`
6. Launches QEMU with VirtIO block device and pflash

### VM Exit Handling

| Architecture | NPF Exit | SBI/Hypercall Exit | Timer Handling |
|---|---|---|---|
| RISC-V 64 | `scause` = 20/21/23 | `scause` = 10 (VSupervisorEnvCall) | SetTimer → hvip injection |
| AArch64 | ESR EC = 0x24 (Data Abort) | ESR EC = 0x15 (SVC) | N/A (bare-metal guest) |
| x86_64 SVM | VMEXIT 0x400 (NPF) | VMEXIT 0x81 (VMMCALL) | N/A (bare-metal guest) |

## Key Dependencies

| Crate | Role |
|---|---|
| `axstd` | ArceOS standard library (`no_std` replacement) |
| `axhal` | Hardware abstraction layer (paging, traps) |
| `axmm` | Memory management (address spaces, page tables) |
| `axfs` | Filesystem access (FAT32 disk image) |
| `riscv` | RISC-V register access (riscv64 only) |
| `sbi-spec` / `sbi-rt` | SBI specification and runtime (riscv64 only) |

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
| 11 | [arceos-runlinuxapp](https://crates.io/crates/arceos-runlinuxapp) | Loading and running real Linux ELF applications (musl libc) on ArceOS, with ELF parsing and Linux syscall handling |
| 12 | [arceos-guestmode](https://crates.io/crates/arceos-guestmode) | Minimal hypervisor: creating a guest address space, entering guest mode, and handling a single VM exit (shutdown) |
| 13 | [arceos-guestaspace](https://crates.io/crates/arceos-guestaspace) | Hypervisor address space management: loop-based VM exit handling with nested page fault (NPF) on-demand mapping |
| 14 | **arceos-guestvdev** (this crate) | Hypervisor virtual device support: timer virtualization, console I/O forwarding, and NPF passthrough; guest runs preemptive multi-tasking |
| 15 | [arceos-guestmonolithickernel](https://crates.io/crates/arceos-guestmonolithickernel) | Full hypervisor + guest monolithic kernel: the guest kernel supports user-space process management, syscall handling, and preemptive scheduling |

**Progression Logic:**

- **#1–#8 (Unikernel Stage)**: Starting from the simplest output, these crates progressively introduce memory allocation, device access (MMIO / VirtIO), multi-task scheduling (both cooperative and preemptive), and filesystem support, building up the core capabilities of a unikernel.
- **#8–#10 (Monolithic Kernel Stage)**: Building on the unikernel foundation, these crates add user/kernel privilege separation, page fault handling, and ELF loading, progressively evolving toward a monolithic kernel.
- **#12–#15 (Hypervisor Stage)**: Starting from minimal VM lifecycle management, these crates progressively add address space management, virtual devices, timer injection, and ultimately run a full monolithic kernel inside a virtual machine.

## License

GPL-3.0-or-later OR Apache-2.0 OR MulanPSL-2.0
