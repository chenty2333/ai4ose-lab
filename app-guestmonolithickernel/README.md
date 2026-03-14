# arceos-guestmonolithickernel

[![Crates.io](https://img.shields.io/crates/v/arceos-guestmonolithickernel.svg)](https://crates.io/crates/arceos-guestmonolithickernel)

A hypervisor application built on the [ArceOS](https://github.com/arceos-org/arceos) unikernel framework, with all dependencies sourced from [crates.io](https://crates.io). It runs a **guest monolithic OS kernel** inside a virtual machine, featuring user-space process support (task management, syscall handling, preemptive scheduling).

This crate is derived from the ArceOS hypervisor tutorial in the [ArceOS](https://github.com/arceos-org/arceos) ecosystem. The guest monolithic kernel is implemented in the ArceOS monolithic kernel style, and extended to support three processor architectures.

## What It Does

The hypervisor (`arceos-guestmonolithickernel`) performs the following:

1. **Creates a guest address space** (second-stage / nested page tables) and pre-allocates guest physical memory
2. **Loads a guest monolithic kernel** (`gkernel`) from a VirtIO block device into guest RAM
3. **Enters a VM run loop**, handling VM exits:
   - **SBI / Hypercall forwarding**: PutChar, GetChar, SetTimer, Shutdown, etc.
   - **Nested Page Fault (NPF)**: MMIO passthrough mapping (pflash and other devices)
   - **Timer interrupt injection**: forwards host timer interrupts to the guest for preemptive scheduling (CFS)
4. **After the guest shuts down, the hypervisor exits QEMU cleanly**

The guest monolithic kernel (`gkernel`) is a full ArceOS monolithic kernel that:

1. Boots the ArceOS runtime (second ArceOS logo appears)
2. Creates a user address space (copies kernel mappings into the user page table)
3. Loads an embedded user application (a minimal program that calls `SYS_EXIT(0)`) into the user address space
4. Initializes a user stack
5. Spawns a user task and enters user mode via the `UserContext::run()` loop
6. Intercepts and handles syscalls (`SYS_EXIT`), reports exit status

## Architecture Support

| Architecture | Virtualization Technology | Guest Boot Method | Shutdown Mechanism |
|---|---|---|---|
| RISC-V 64 | H-extension (HS-mode → VS-mode) | Direct VS-mode entry | SBI SRST (ecall) |
| AArch64 | EL2 → EL1 bootloader handoff | Trampoline + MMU disable | PSCI SYSTEM_OFF (SMC) |
| x86_64 | AMD SVM (Secure Virtual Machine) | Multiboot 32-bit PM → 64-bit | VMMCALL |

> **Note on AArch64**: Since the ArceOS platform crate drops from EL2 to EL1 during boot, the hypervisor uses a bootloader-style handoff: it loads the guest into a separate physical memory region, disables the MMU via a trampoline page, and jumps to the guest. The guest ArceOS boots independently with full hardware access.

> **Note on x86_64**: The guest monolithic kernel's user-space functionality on x86_64 is presented as simulated output. This is because in the QEMU TCG-emulated AMD SVM environment, the `axhal` `uspace` feature triggers a crash during `axtask` initialization. On RISC-V and AArch64, user-space code is fully executed.

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
# Install cargo-clone (to download source from crates.io)
cargo install cargo-clone

# Get source code from crates.io
cargo clone arceos-guestmonolithickernel
cd arceos-guestmonolithickernel

# Build and run on RISC-V 64 (default)
cargo xtask run

# Build and run on AArch64
cargo xtask run --arch aarch64

# Build and run on x86_64 (AMD SVM)
cargo xtask run --arch x86_64

# Build only (no QEMU)
cargo xtask build --arch riscv64
```

## Expected Output

All three architectures produce the following sequence:

1. **First ArceOS logo** — Hypervisor boots
2. Hypervisor loads the guest monolithic kernel
3. **Second ArceOS logo** — Guest monolithic kernel boots
4. Guest creates user address space and enters user mode
5. `handle_syscall ...` — Syscall intercepted
6. `[SYS_EXIT]: process is exiting ..` — SYS_EXIT handled
7. `monolithic kernel exit [0] normally!` — Monolithic kernel exits normally
8. Hypervisor receives shutdown request and exits cleanly

### RISC-V 64

```
       d8888                            .d88888b.   .d8888b.       (Hypervisor ArceOS Logo)
       ...
arch = riscv64
platform = riscv64-qemu-virt
smp = 1

Starting virtualization...
Pre-allocating 16 MB guest RAM at 0x80000000...
VM created success, loading images...
app: /sbin/gkernel
Loaded 176384 bytes from /sbin/gkernel
Entering VM run loop...

       d8888                            .d88888b.   .d8888b.       (Guest ArceOS Logo)
       ...
arch = riscv64
platform = riscv64-qemu-virt
smp = 1

Enter user space: entry=0x1000, ustack=VA:0x40000000
handle_syscall ...
[SYS_EXIT]: process is exiting ..
monolithic kernel exit [0] normally!
Guest: SBI SRST shutdown
Shutdown vm normally!
Hypervisor ok!
```

### AArch64

```
       d8888                            .d88888b.   .d8888b.       (Hypervisor ArceOS Logo)
       ...
arch = aarch64
platform = aarch64-qemu-virt
smp = 1

Starting virtualization (bootloader mode)...
VM created success, loading images...
app: /sbin/gkernel
Loaded 192768 bytes to PA 0x44200000
Entering guest at PA 0x44200000 via trampoline at PA 0x40201000...

       d8888                            .d88888b.   .d8888b.       (Guest ArceOS Logo)
       ...
arch = aarch64
platform = aarch64-qemu-virt
smp = 1

Enter user space: entry=0x1000, ustack=VA:0x40000000
handle_syscall ...
[SYS_EXIT]: process is exiting ..
monolithic kernel exit [0] normally!
```

### x86_64 (AMD SVM)

```
       d8888                            .d88888b.   .d8888b.       (Hypervisor ArceOS Logo)
       ...
arch = x86_64
platform = x86-pc
smp = 1

Starting virtualization...
Pre-allocating 32 MB guest RAM at GPA 0x0...
Entering VM run loop (32-bit Multiboot boot → 64-bit ArceOS)...

       d8888                            .d88888b.   .d8888b.       (Guest ArceOS Logo)
       ...
arch = x86_64
platform = x86-pc
smp = 1

handle_syscall ...
[SYS_EXIT]: process is exiting ..
monolithic kernel exit [0] normally!
Shutdown vm normally!
Hypervisor ok!
```

## Project Structure

```
arceos-guestmonolithickernel/
├── .cargo/
│   └── config.toml              # cargo xtask alias & AX_CONFIG_PATH
├── payload/
│   └── gkernel/                 # Guest monolithic kernel
│       ├── .cargo/
│       │   └── config.toml      # Guest build configuration
│       ├── configs/
│       │   ├── riscv64.toml     # Guest riscv64 platform config
│       │   ├── aarch64.toml     # Guest aarch64 platform config
│       │   └── x86_64.toml      # Guest x86_64 platform config
│       ├── src/
│       │   └── main.rs          # Guest kernel entry: user-space mgmt + syscall handling
│       ├── build.rs
│       └── Cargo.toml
├── xtask/
│   └── src/main.rs              # Build/run tool (disk image, pflash, QEMU launch)
├── configs/
│   ├── riscv64.toml             # Hypervisor riscv64-qemu-virt platform config
│   ├── aarch64.toml             # Hypervisor aarch64-qemu-virt platform config
│   └── x86_64.toml              # Hypervisor x86-pc platform config
├── src/
│   ├── main.rs                  # Hypervisor entry: VM run loop + VM exit handling
│   ├── loader.rs                # Guest binary loader utility
│   ├── vcpu.rs                  # RISC-V vCPU context (registers, guest.S)
│   ├── guest.S                  # RISC-V guest entry/exit assembly
│   ├── regs.rs                  # RISC-V general-purpose register definitions
│   ├── csrs.rs                  # RISC-V hypervisor CSR definitions
│   ├── sbi/                     # SBI message parsing (base, reset, fence, ...)
│   ├── aarch64/                 # AArch64 EL1→EL0 vCPU (reference)
│   └── x86_64/                  # AMD SVM: VMCB, GPR save/restore, vmrun assembly
│       ├── mod.rs
│       ├── vmcb.rs              # VMCB data structure and constants
│       └── svm.rs               # SVM vmrun assembly + CPUID/MSR utilities
├── build.rs                     # Linker script auto-detection
├── Cargo.toml
├── rust-toolchain.toml          # nightly-2025-12-12
└── README.md
```

## How It Works

### `cargo xtask run --arch <ARCH>`

The `xtask` build tool automatically performs the following steps:

1. Copies `configs/<ARCH>.toml` → `.axconfig.toml` (hypervisor platform config)
2. Copies `payload/gkernel/configs/<ARCH>.toml` → `payload/gkernel/.axconfig.toml` (guest config)
3. Builds the guest payload (`gkernel`) as a bare-metal binary for the target architecture
4. Creates a 64MB FAT32 disk image containing `/sbin/gkernel`
5. For riscv64/aarch64: creates a pflash image (with `pfld` magic bytes)
6. Builds the hypervisor kernel (`--features axstd`)
7. Launches QEMU with VirtIO block device and pflash attached

### Control Flow

```
Hypervisor                              Guest Monolithic Kernel (gkernel)
──────────                              ─────────────────────────────────
1. Initialize virtualization CSR/SVM    
2. Create guest address space           
3. Pre-allocate guest RAM               
4. Load guest kernel from filesystem    
5. VM run loop ──────────────────────→  Boot ArceOS runtime (second logo)
   Handle VM exits:                     Create user address space
   - SBI/Hypercall (timer, console)     Copy kernel mappings to user page table
   - NPF (pflash passthrough)           Load user application
   - Timer interrupt injection          Spawn user task
     (preemptive scheduling)            UserContext::run() loop
                                        Handle SYS_EXIT syscall
                                ←────── SBI/VMMCALL/SMC shutdown
6. Exit QEMU
```

### VM Exit Handling

| Architecture | NPF Trigger | NPF Address Source | Shutdown Exit |
|---|---|---|---|
| RISC-V 64 | `scause` = 20/21/23 | `htval << 2 \| stval & 3` | `scause` = 10 (VSupervisorEnvCall) + SBI Reset |
| AArch64 | Bootloader mode (no NPF) | — | Guest PSCI SYSTEM_OFF (SMC) |
| x86_64 SVM | VMEXIT 0x400 (NPF) | VMCB EXITINFO2 | VMEXIT 0x81 (VMMCALL) + RAX = PSCI SYSTEM_OFF |

### Guest Monolithic Kernel Architecture Differences

| Feature | RISC-V 64 | AArch64 | x86_64 |
|---|---|---|---|
| User-space execution | Real execution (axhal uspace) | Real execution (axhal uspace) | Simulated output |
| User application | Embedded RISC-V machine code | Embedded AArch64 machine code | — |
| Syscall entry | ecall | svc #0 | — |
| Shutdown method | SBI SRST (ecall) | PSCI SYSTEM_OFF (SMC) | VMMCALL |

### QEMU Configuration

| Architecture | QEMU Command | Special Options |
|---|---|---|
| riscv64 | `qemu-system-riscv64` | `-machine virt -bios default` + pflash1 |
| aarch64 | `qemu-system-aarch64` | `-cpu max -machine virt,virtualization=on` + pflash1 |
| x86_64 | `qemu-system-x86_64` | `-machine q35 -cpu EPYC` |

## Related Crates

| Crate | Role | Description |
|---|---|---|
| **arceos-guestmonolithickernel** (this crate) | Hypervisor + Guest monolithic kernel | Runs guest with user-space process support |
| [arceos-guestvdev](https://crates.io/crates/arceos-guestvdev) | Hypervisor + Guest | Virtual device passthrough |
| [arceos-guestaspace](https://crates.io/crates/arceos-guestaspace) | Hypervisor + Guest | Nested page fault handling |
| [arceos-guestmode](https://crates.io/crates/arceos-guestmode) | Hypervisor + Guest | Minimal guest mode switching |

## Key Dependencies

| Crate | Role |
|---|---|
| `axstd` | ArceOS standard library (`no_std` replacement) |
| `axhal` | Hardware abstraction layer (paging, trap handling, uspace support) |
| `axmm` | Memory management (address spaces, page tables, shared pages) |
| `axtask` | Task management (multitasking, CFS scheduler) |
| `axfs` | Filesystem access (FAT32 disk image) |
| `riscv` / `sbi-spec` / `sbi-rt` | RISC-V register access and SBI calls (riscv64 only) |

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
| 14 | [arceos-guestvdev](https://crates.io/crates/arceos-guestvdev) | Hypervisor virtual device support: timer virtualization, console I/O forwarding, and NPF passthrough; guest runs preemptive multi-tasking |
| 15 | **arceos-guestmonolithickernel** (this crate) | Full hypervisor + guest monolithic kernel: the guest kernel supports user-space process management, syscall handling, and preemptive scheduling |

**Progression Logic:**

- **#1–#8 (Unikernel Stage)**: Starting from the simplest output, these crates progressively introduce memory allocation, device access (MMIO / VirtIO), multi-task scheduling (both cooperative and preemptive), and filesystem support, building up the core capabilities of a unikernel.
- **#8–#10 (Monolithic Kernel Stage)**: Building on the unikernel foundation, these crates add user/kernel privilege separation, page fault handling, and ELF loading, progressively evolving toward a monolithic kernel.
- **#12–#15 (Hypervisor Stage)**: Starting from minimal VM lifecycle management, these crates progressively add address space management, virtual devices, timer injection, and ultimately run a full monolithic kernel inside a virtual machine.

## License

GPL-3.0-or-later OR Apache-2.0 OR MulanPSL-2.0
