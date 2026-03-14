# arceos-guestmode

A standalone hypervisor application running on [ArceOS](https://github.com/arceos-org/arceos), demonstrating hardware-assisted virtualization by booting a minimal guest OS (`skernel`) in guest mode. Supports **RISC-V H-extension**, **ARM AArch64 EL2**, and **AMD SVM (x86_64)** virtualization. All kernel dependencies are sourced from [crates.io](https://crates.io). The guest performs a single shutdown call (SBI ecall on RISC-V / PSCI HVC on AArch64 / VMMCALL on x86_64), which the hypervisor intercepts and handles, demonstrating the complete VM lifecycle: guest context setup, second-stage page table configuration, guest entry, VM exit handling, and shutdown call parsing.

## What It Does

This application builds a minimal Type-1 hypervisor on ArceOS that:

1. **Creates a guest address space** (`main.rs`): Allocates a new address space for the virtual machine using ArceOS's memory management (`axmm`).
2. **Loads guest image** (`loader.rs`): Reads the guest binary (`/sbin/skernel`) from a FAT32 virtual disk and maps it at the guest's entry point.
3. **Prepares guest context** (`main.rs`): Configures architecture-specific hypervisor registers and the guest's initial CPU state (CSRs on RISC-V, system registers on AArch64, VMCB on x86_64).
4. **Configures second-stage page table** (`main.rs`): Sets up stage-2 / nested address translation so the guest's physical addresses map to real host memory.
5. **Enters guest mode**: Saves hypervisor state, loads guest registers, and transitions to the guest (`sret` on RISC-V, `eret` on AArch64, `vmrun` on x86_64).
6. **Handles VM exit** (`main.rs`): On guest trap/exception, parses the shutdown request and terminates the VM gracefully.

### Architecture-Specific Details

| | RISC-V 64 (H-extension) | AArch64 (EL2) | x86_64 (AMD SVM) |
|---|---|---|---|
| **Guest entry** | `sret` with `hstatus.SPV=Guest` | `eret` from EL2 to EL1 | `vmrun` with guest VMCB |
| **Stage-2 PT register** | `hgatp` (Sv39x4) | `VTTBR_EL2` | `nCR3` (NPT root in VMCB) |
| **VM exit mechanism** | `scause` = VirtualSupervisorEnvCall (code 10) | `ESR_EL2` EC=0x16 (HVC64) | VMCB `EXITCODE` = 0x81 (VMMCALL) |
| **Guest shutdown call** | SBI legacy shutdown (`a7=8, ecall`) | PSCI SYSTEM_OFF (`x0=0x84000008, hvc #0`) | VMMCALL (`eax=0x84000008`) |
| **Guest entry point** | `0x8020_0000` | `0x4020_0000` | `0x10000` (real mode) |
| **Guest CPU mode** | S-mode (64-bit) | EL1 (AArch64) | 16-bit real mode |

### The Guest Payload

`skernel` is now a binary target of the main package, not a standalone crate.

The guest (`skernel`) is a minimal bare-metal program that immediately performs a shutdown:

**RISC-V:**
```asm
li a7, 8      // SBI legacy shutdown
ecall
```

**AArch64:**
```asm
movz x0, #0x0008
movk x0, #0x8400, lsl #16    // PSCI SYSTEM_OFF = 0x84000008
svc #0
```

**x86_64 (16-bit real mode):**
```asm
.code16
mov eax, 0x84000008    // function ID = PSCI-style SYSTEM_OFF
vmmcall                // AMD SVM hypercall → VMEXIT to hypervisor
```

> **Note (x86_64):** The guest payload is assembled as 16-bit real-mode code using `.code16`. It uses `global_asm!` instead of a Rust function to avoid the compiler inserting a function prologue (`push rax`), which would cause a stack access fault in a real-mode guest with no mapped stack.

## Supported Architectures

| Architecture | Rust Target | QEMU Machine | Platform | CPU |
|---|---|---|---|---|
| riscv64 | `riscv64gc-unknown-none-elf` | `qemu-system-riscv64 -machine virt` | riscv64-qemu-virt | default |
| aarch64 | `aarch64-unknown-none-softfloat` | `qemu-system-aarch64 -machine virt,virtualization=on` | aarch64-qemu-virt | max |
| x86_64 | `x86_64-unknown-none` | `qemu-system-x86_64 -machine q35` | x86-pc | EPYC |

> **Note (AArch64):** QEMU must be started with `-machine virt,virtualization=on` and `-cpu max` to enable EL2 support. ArceOS must be configured to stay at EL2 (with VHE — Virtualization Host Extensions) rather than dropping to EL1 during boot.

> **Note (x86_64):** QEMU must use `-cpu EPYC` (or another AMD CPU model) to enable AMD SVM extensions. Intel VMX is **not** supported by this implementation.

## Prerequisites

### 1. Rust nightly toolchain

```bash
rustup install nightly
rustup default nightly
```

### 2. Bare-metal targets (install the ones you need)

```bash
rustup target add riscv64gc-unknown-none-elf
rustup target add aarch64-unknown-none-softfloat
rustup target add x86_64-unknown-none
```

### 3. rust-objcopy (from `cargo-binutils`)

```bash
cargo install cargo-binutils
rustup component add llvm-tools
```

### 4. QEMU (install the emulators for your target architectures)

```bash
# Ubuntu 24.04
sudo apt update
sudo apt install qemu-system-misc qemu-system-arm qemu-system-x86

# macOS (Homebrew)
brew install qemu
```

> Note: On Ubuntu, `qemu-system-aarch64` is provided by `qemu-system-arm`. `qemu-system-riscv64` is provided by `qemu-system-misc`. `qemu-system-x86_64` is provided by `qemu-system-x86`.

### Summary of required packages (Ubuntu 24.04)

```bash
# All-in-one install
sudo apt update
sudo apt install -y build-essential qemu-system-misc qemu-system-arm qemu-system-x86

# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup install nightly && rustup default nightly
rustup target add riscv64gc-unknown-none-elf aarch64-unknown-none-softfloat x86_64-unknown-none
cargo install cargo-binutils
rustup component add llvm-tools
```

## Quick Start

```bash
# Install cargo-clone sub-command
cargo install cargo-clone
# Get source code of arceos-guestmode crate from crates.io
cargo clone arceos-guestmode
# Enter crate directory
cd arceos-guestmode

# Build and run on RISC-V 64 QEMU (default)
cargo xtask run

# Build and run on AArch64 QEMU
cargo xtask run --arch aarch64

# Build and run on x86_64 QEMU (AMD SVM)
cargo xtask run --arch x86_64

# Build only (no QEMU)
cargo xtask build --arch riscv64
cargo xtask build --arch aarch64
cargo xtask build --arch x86_64
```

### What `cargo xtask run` does

The `xtask` command automates the full workflow:

1. **Install config** — copies `configs/<arch>.toml` → `.axconfig.toml`
2. **Build payload** — compiles `payload/skernel` (a minimal `no_std` Rust guest) for the target architecture, converts to flat binary via `rust-objcopy`
3. **Create disk image** — builds a 64 MB FAT32 image containing `/sbin/skernel`
4. **Build kernel** — `AX_CONFIG_PATH=.axconfig.toml cargo build --release --target <target> --features axstd`
5. **Objcopy** — converts hypervisor ELF to raw binary (riscv64 and aarch64 only; x86_64 boots ELF directly)
6. **Run QEMU** — launches QEMU with VirtIO block device attached

### Expected output (RISC-V 64)

```
Hypervisor ...
app: /sbin/skernel
paddr: PA:0x80633000
VmExit Reason: VSuperEcall: Some(Reset(Reset { reset_type: Shutdown, reason: NoReason }))
Shutdown vm normally!
```

### Expected output (AArch64)

```
Hypervisor ...
app: /sbin/skernel
paddr: PA:0x404f4000
VmExit Reason: GuestSVC: Some(PsciSystemOff)
Shutdown vm normally!
Hypervisor ok!
```

### Expected output (x86_64)

```
Hypervisor ...
app: /sbin/skernel
paddr: PA:0x44e000
paddr: PA:0x415000
VmExit Reason: VMMCALL
Shutdown vm normally!
Hypervisor ok!
```

QEMU will automatically exit after the hypervisor prints the final message.

## Project Structure

```
app-guestmode/
├── xtask/
│   └── src/
│       └── main.rs               # Build/run tool: payload, disk image, QEMU
├── configs/
│   ├── riscv64.toml              # RISC-V platform config
│   ├── aarch64.toml              # AArch64 platform config
│   └── x86_64.toml               # x86_64 platform config
├── payload/
│   └── skernel/                  # Minimal guest OS (no_std Rust)
│       └── src/
│           └── main.rs           # SBI/PSCI/VMMCALL shutdown (multi-arch)
├── src/
│   ├── main.rs                   # Entry: arch dispatch, load guest, run VM
│   ├── loader.rs                 # Guest binary loader (FAT32 → address space)
│   │
│   │  ── RISC-V 64 specific ──
│   ├── vcpu.rs                   # vCPU state & guest.S inclusion
│   ├── guest.S                   # Save/restore hypervisor↔guest, sret
│   ├── regs.rs                   # GPR file abstraction
│   ├── csrs.rs                   # H-extension CSR definitions
│   └── sbi/                     # SBI message parsing
│       ├── mod.rs
│       ├── base.rs, srst.rs, rfnc.rs, pmu.rs, dbcn.rs
│   │
│   │  ── AArch64 specific ──
│   ├── aarch64/
│   │   ├── mod.rs                # Module declarations
│   │   ├── vcpu.rs               # vCPU state & guest.S inclusion
│   │   ├── guest.S               # Exception vector table, eret/exit
│   │   ├── regs.rs               # GPR file (x0-x30)
│   │   └── hvc.rs                # HVC/PSCI message parsing
│   │
│   │  ── x86_64 (AMD SVM) specific ──
│   └── x86_64/
│       ├── mod.rs                # Module declarations
│       ├── vmcb.rs               # VMCB structure, offsets, accessors
│       └── svm.rs                # CPUID/MSR helpers, VMRUN wrapper (global_asm)
│
├── build.rs                      # Linker script path (auto-detects arch)
├── Cargo.toml                    # Dependencies from crates.io
├── rust-toolchain.toml           # Nightly toolchain & targets
└── README.md
```

## Key Components

| Component | Role |
|---|---|
| `axstd` | ArceOS standard library (replaces Rust's `std` in `no_std` environment) |
| `axhal` | Hardware Abstraction Layer — paging, trap handling, `uspace` support |
| `axmm` | Memory management — guest address spaces, page mapping with `SharedPages` backend |
| `axtask` | Task scheduler — CFS scheduling, context switching |
| `axfs` / `axfeat` | Filesystem — FAT32 virtual disk access for loading the guest binary |
| `axio` | I/O traits for file operations |
| `axsync` | Synchronization primitives |
| `axlog` | Kernel logging (`ax_println!`) |
| `riscv` | RISC-V CSR access — sstatus, scause (riscv64 only) |
| `tock-registers` | Type-safe register bitfield definitions for H-extension CSRs (riscv64 only) |
| `sbi-spec` | SBI extension ID constants (riscv64 only) |
| `memoffset` | Struct field offset calculation for assembly register save/restore |

## Technical Details

### RISC-V H-Extension

The RISC-V Hypervisor extension provides:

- **VS-mode** (Virtual Supervisor): A virtualized supervisor mode for the guest OS
- **Two-stage address translation**: `hgatp` controls guest-physical → host-physical translation
- **Trap delegation**: Guest exceptions/interrupts can be delegated or handled by the hypervisor
- **CSRs**: `hstatus`, `hgatp`, `hedeleg`, `hideleg`, `hvip`, etc.

**Guest Entry/Exit Flow (RISC-V):**

1. **Entry** (`guest.S`): Save hypervisor GPRs → swap CSRs → set `stvec` to exit handler → load guest GPRs → `sret`
2. **Exit** (`guest.S`): Save guest GPRs → swap back CSRs → restore hypervisor GPRs → `ret`
3. **Handler** (`main.rs`): Read `scause` → if VirtualSupervisorEnvCall (code 10), parse SBI message → handle shutdown

### AArch64 EL2 Virtualization

The AArch64 hypervisor uses ARM's EL2 (Exception Level 2):

- **EL2**: The hypervisor exception level, runs the ArceOS hypervisor
- **EL1**: The guest exception level, runs the minimal skernel guest
- **Stage-2 translation**: `VTTBR_EL2` + `VTCR_EL2` control IPA → PA mapping
- **HCR_EL2**: Hypervisor Configuration Register — enables VM mode (`VM=1`) and AArch64 EL1 (`RW=1`)
- **Exception vectors**: Custom `VBAR_EL2` vector table catches guest HVC traps

**Guest Entry/Exit Flow (AArch64):**

1. **Entry** (`guest.S`): Save host x19-x30/SP → set `VBAR_EL2` to exit vectors → load guest x0-x30/ELR/SPSR → `eret` to EL1
2. **Exit** (`guest.S`): Save guest x0-x30/ELR/SPSR → read `ESR_EL2`/`FAR_EL2`/`HPFAR_EL2` → restore `VBAR_EL2` → restore host state → `ret`
3. **Handler** (`main.rs`): Check `ESR_EL2` EC field → if HVC64 (EC=0x16), parse PSCI function ID from x0 → handle SYSTEM_OFF

### AMD SVM (x86_64) Virtualization

AMD SVM (Secure Virtual Machine) is AMD's hardware-assisted virtualization extension for x86 processors:

- **VMCB** (Virtual Machine Control Block): A 4 KB page-aligned structure containing the guest state (Save Area) and hypervisor intercept configuration (Control Area). The CPU reads/writes the VMCB on `VMRUN`/`VMEXIT`.
- **VMRUN**: Instruction that enters the guest context described by the VMCB. Takes the VMCB physical address in `RAX`.
- **VMEXIT**: Occurs when the guest executes an intercepted instruction (e.g., `VMMCALL`). Exit reason and info are stored in the VMCB Control Area.
- **VMMCALL**: Guest hypercall instruction — triggers a `VMEXIT` with exit code `0x81`, allowing the guest to request services from the hypervisor.
- **NPT** (Nested Page Tables): Second-stage address translation in SVM. Enabled by setting `NP_ENABLE=1` in the VMCB and the nested page table root (`nCR3`).
- **VMSAVE/VMLOAD**: Instructions to save/restore host FS, GS, TR, LDTR, and related MSRs. Required because `VMRUN`/`VMEXIT` do **not** automatically save/restore these registers.
- **IOPM/MSRPM**: I/O Permission Map (12 KB) and MSR Permission Map (8 KB) — bitmaps that control which I/O port and MSR accesses by the guest trigger a VMEXIT.

**Guest Entry/Exit Flow (x86_64 SVM):**

1. **Setup**: Check SVM support via `CPUID` → enable `EFER.SVME` → allocate host save area (`MSR_VM_HSAVE_PA`) → allocate IOPM, MSRPM, and host VMCB pages → configure guest VMCB (intercepts, NPT, real-mode segments)
2. **Entry** (`svm.rs`): Push callee-saved GPRs → `cli` → `vmsave` host FS/GS/TR/LDTR → `vmrun` with guest VMCB PA in RAX
3. **Exit** (`svm.rs`): On VMEXIT, hardware restores host RSP/RIP/RAX → `vmload` host FS/GS/TR/LDTR → `sti` → pop callee-saved GPRs → `ret`
4. **Handler** (`main.rs`): Read `EXITCODE` from VMCB → if VMMCALL (0x81), read guest RAX → match `0x84000008` for shutdown → ACPI power-off QEMU

**Key VMCB Layout:**

| Region | Offset Range | Contents |
|---|---|---|
| Control Area | `0x000–0x3FF` | Intercept masks, IOPM/MSRPM base, ASID, NPT config, exit code/info |
| Save Area | `0x400–0xFFF` | Guest segment registers, CR0-4, EFER, DR6/7, RFLAGS, RIP, RSP, RAX |

**Real-Mode Guest:**

The x86_64 guest runs in 16-bit real mode to simplify setup — no GDT, IDT, or paging configuration is needed in the guest. The VMCB's CS segment base is set to the guest entry point (`0x10000`), so `RIP=0` begins execution at guest physical address `0x10000`. NPT handles address translation from guest physical to host physical addresses.

## Origin

This crate is derived from the ArceOS hypervisor tutorial in the [ArceOS](https://github.com/arceos-org/arceos) ecosystem, adapted to work as a standalone crate with all dependencies from crates.io and extended to support multiple architectures.

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
| 12 | **arceos-guestmode** (this crate) | Minimal hypervisor: creating a guest address space, entering guest mode, and handling a single VM exit (shutdown) |
| 13 | [arceos-guestaspace](https://crates.io/crates/arceos-guestaspace) | Hypervisor address space management: loop-based VM exit handling with nested page fault (NPF) on-demand mapping |
| 14 | [arceos-guestvdev](https://crates.io/crates/arceos-guestvdev) | Hypervisor virtual device support: timer virtualization, console I/O forwarding, and NPF passthrough; guest runs preemptive multi-tasking |
| 15 | [arceos-guestmonolithickernel](https://crates.io/crates/arceos-guestmonolithickernel) | Full hypervisor + guest monolithic kernel: the guest kernel supports user-space process management, syscall handling, and preemptive scheduling |

**Progression Logic:**

- **#1–#8 (Unikernel Stage)**: Starting from the simplest output, these crates progressively introduce memory allocation, device access (MMIO / VirtIO), multi-task scheduling (both cooperative and preemptive), and filesystem support, building up the core capabilities of a unikernel.
- **#8–#10 (Monolithic Kernel Stage)**: Building on the unikernel foundation, these crates add user/kernel privilege separation, page fault handling, and ELF loading, progressively evolving toward a monolithic kernel.
- **#11–#14 (Hypervisor Stage)**: Starting from minimal VM lifecycle management, these crates progressively add address space management, virtual devices, timer injection, and ultimately run a full monolithic kernel inside a virtual machine.

## License

GPL-3.0-or-later OR Apache-2.0 OR MulanPSL-2.0
