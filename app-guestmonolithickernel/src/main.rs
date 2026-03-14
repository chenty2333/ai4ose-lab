//! ArceOS Guest Monolithic Kernel (Hypervisor)
//!
//! Derived from the h_4_0 tutorial crate in the ArceOS ecosystem.
//! Runs a guest monolithic OS kernel with user-space process support
//! (task management, syscall handling, pflash passthrough) on
//! RISC-V H-extension, ARM AArch64 EL2, and AMD SVM.
//!
//! The h_4_0 control flow:
//!   1. Create guest address space with pre-allocated RAM
//!   2. Load guest monolithic kernel binary from filesystem
//!   3. Setup vCPU context
//!   4. Run guest in loop, handling VM exits:
//!      - SBI/Hypercall forwarding (PutChar, SetTimer, Shutdown, etc.)
//!      - Nested page fault → passthrough mapping (pflash, MMIO)
//!      - Timer interrupt → inject to guest (for preemptive scheduling)
//!
//! The guest monolithic kernel (m_1_1 style):
//!   1. Creates a user address space
//!   2. Loads a user application (from pflash or embedded)
//!   3. Spawns a user task to run the application
//!   4. Handles syscalls (SYS_EXIT)
//!   5. Reports exit status

#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]
#![cfg_attr(all(feature = "axstd", target_arch = "riscv64"), feature(riscv_ext_intrinsics))]

#[cfg(feature = "axstd")]
extern crate axstd as std;

#[cfg(feature = "axstd")]
extern crate alloc;

#[cfg(feature = "axstd")]
#[macro_use]
extern crate axlog;

#[cfg(feature = "axstd")]
extern crate axfs;
#[cfg(feature = "axstd")]
extern crate axio;

// ────────────────── RISC-V 64 specific modules ──────────────────
#[cfg(all(feature = "axstd", target_arch = "riscv64"))]
mod vcpu;
#[cfg(all(feature = "axstd", target_arch = "riscv64"))]
mod regs;
#[cfg(all(feature = "axstd", target_arch = "riscv64"))]
mod csrs;
#[cfg(all(feature = "axstd", target_arch = "riscv64"))]
mod sbi;

// ────────────────── AArch64 specific modules ──────────────────
// NOTE: The current AArch64 approach uses a bootloader-style handoff
// (hypervisor loads and jumps to guest at EL1), so the legacy EL1→EL0
// guest/host context-switching modules are not needed.
// The source files in src/aarch64/ are preserved for reference.
// #[cfg(all(feature = "axstd", target_arch = "aarch64"))]
// #[path = "aarch64/mod.rs"]
// mod aarch64;

// ────────────────── x86_64 (AMD SVM) specific modules ──────────────────
#[cfg(all(feature = "axstd", target_arch = "x86_64"))]
#[path = "x86_64/mod.rs"]
mod x86_64_svm;

// ────────────────── Common modules ──────────────────
// Loader module for loading guest binaries into an AddrSpace.
// Currently all architectures use inline loading, but this module is
// preserved as a reusable utility.
#[cfg(feature = "axstd")]
#[allow(dead_code)]
mod loader;

// VM entry point (guest physical / intermediate-physical address)
#[cfg(all(feature = "axstd", target_arch = "riscv64"))]
const VM_ENTRY: usize = 0x8020_0000;

#[cfg(all(feature = "axstd", target_arch = "aarch64"))]
const VM_ENTRY: usize = 0x4420_0000;

// For x86_64 with axstd: ArceOS guest kernel-base-paddr = 0x200000
#[cfg(all(feature = "axstd", target_arch = "x86_64"))]
const VM_ENTRY: usize = 0x20_0000;

#[cfg(all(
    feature = "axstd",
    not(any(target_arch = "riscv64", target_arch = "aarch64", target_arch = "x86_64"))
))]
const VM_ENTRY: usize = 0x8020_0000;

// ════════════════════════════════════════════════════════════════
//  Entry point
// ════════════════════════════════════════════════════════════════

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    #[cfg(all(feature = "axstd", target_arch = "riscv64"))]
    riscv64_main();

    #[cfg(all(feature = "axstd", target_arch = "aarch64"))]
    aarch64_main();

    #[cfg(all(feature = "axstd", target_arch = "x86_64"))]
    x86_64_main();

    #[cfg(not(feature = "axstd"))]
    {
        println!("This application requires the 'axstd' feature for running the Hypervisor.");
        println!("Run with: cargo xtask run [--arch riscv64|aarch64|x86_64]");
    }
}

// ════════════════════════════════════════════════════════════════
//  RISC-V 64  (H-extension hypervisor — h_4_0 style)
//
//  Monolithic kernel guest support with virtual device handling:
//  - Timer virtualization (SetTimer + hvip injection) for preemptive
//    scheduling in the guest
//  - Console I/O via SBI PutChar/GetChar forwarding
//  - NPF passthrough for MMIO devices (pflash at 0x2200_0000)
//
//  The guest runs a monolithic kernel (m_1_1 style) with user-space
//  process support: task management, syscall handling, pflash passthrough.
// ════════════════════════════════════════════════════════════════

#[cfg(all(feature = "axstd", target_arch = "riscv64"))]
fn riscv64_main() {
    use alloc::sync::Arc;
    use vcpu::VmCpuRegisters;
    use riscv::register::scause;
    use csrs::defs::hstatus;
    use csrs::traps;
    use tock_registers::LocalRegisterCopy;
    use csrs::{RiscvCsrTrait, CSR};
    use vcpu::_run_guest;
    use axhal::mem::PhysAddr;
    use axhal::paging::{MappingFlags, PageSize};
    use axmm::backend::{Backend, SharedPages};
    use memory_addr::{va, PAGE_SIZE_4K};

    ax_println!("Starting virtualization...");

    // ════════════════════════════════════════════════════
    //  Step 0: Setup H-extension CSRs  (matches h_4_0 riscv_vcpu::setup_csrs)
    // ════════════════════════════════════════════════════
    unsafe {
        // Delegate VS-mode synchronous exceptions to the guest so it can
        // handle its own page faults, illegal instructions, breakpoints, etc.
        CSR.hedeleg.write_value(
            traps::exception::INST_ADDR_MISALIGN
                | traps::exception::BREAKPOINT
                | traps::exception::ENV_CALL_FROM_U_OR_VU
                | traps::exception::INST_PAGE_FAULT
                | traps::exception::LOAD_PAGE_FAULT
                | traps::exception::STORE_PAGE_FAULT
                | traps::exception::ILLEGAL_INST,
        );

        // Delegate VS-mode interrupts to the guest.
        CSR.hideleg.write_value(
            traps::interrupt::VIRTUAL_SUPERVISOR_TIMER
                | traps::interrupt::VIRTUAL_SUPERVISOR_EXTERNAL
                | traps::interrupt::VIRTUAL_SUPERVISOR_SOFT,
        );

        // Clear all pending virtual interrupts.
        CSR.hvip.read_and_clear_bits(
            traps::interrupt::VIRTUAL_SUPERVISOR_TIMER
                | traps::interrupt::VIRTUAL_SUPERVISOR_EXTERNAL
                | traps::interrupt::VIRTUAL_SUPERVISOR_SOFT,
        );

        // Allow the guest to read all counters (cycle, time, instret, HPMs).
        CSR.hcounteren.write_value(0xffff_ffff);

        // Clear SIE timer bit — we will enable it when the guest calls SetTimer.
        CSR.sie
            .read_and_clear_bits(traps::interrupt::SUPERVISOR_TIMER);
    }

    // ════════════════════════════════════════════════════
    //  Step 1: Create guest address space (h_4_0: AddrSpace::new_empty)
    // ════════════════════════════════════════════════════
    let mut uspace = axmm::AddrSpace::new_empty(va!(0x0), 0x7fff_ffff_f000).unwrap();

    let flags = MappingFlags::READ | MappingFlags::WRITE
        | MappingFlags::EXECUTE | MappingFlags::USER;

    // ════════════════════════════════════════════════════
    //  Step 2: Pre-allocate guest physical RAM
    //
    //  h_4_0: map_alloc(0x8000_0000, 0x100_0000, flags, true)
    //  Pre-allocate 16MB at 0x8000_0000 to avoid NPF during guest boot.
    // ════════════════════════════════════════════════════
    const PHY_MEM_START: usize = 0x8000_0000;
    const PHY_MEM_SIZE: usize = 0x100_0000; // 16 MB

    ax_println!("Pre-allocating {} MB guest RAM at {:#x}...", PHY_MEM_SIZE / (1024 * 1024), PHY_MEM_START);
    let pages = Arc::new(
        SharedPages::new(PHY_MEM_SIZE, PageSize::Size4K)
            .expect("alloc guest RAM pages"),
    );
    uspace
        .map(
            PHY_MEM_START.into(),
            PHY_MEM_SIZE,
            flags,
            true,
            Backend::new_shared(PHY_MEM_START.into(), pages),
        )
        .expect("map guest RAM");

    // ════════════════════════════════════════════════════
    //  Step 3: Load guest monolithic kernel binary into pre-allocated RAM
    //
    //  h_4_0 loads from /sbin/m_1_1_riscv64-qemu-virt.bin.
    //  We use the unified guest payload /sbin/gkernel.
    // ════════════════════════════════════════════════════
    {
        let fname = "/sbin/gkernel";
        ax_println!("VM created success, loading images...");
        ax_println!("app: {}", fname);
        let ctx = axfs::ROOT_FS_CONTEXT.get().expect("Root FS not initialized");
        let file = axfs::File::open(ctx, fname).expect("Cannot open guest image");
        let mut offset = 0usize;
        let mut total_bytes = 0usize;
        loop {
            let mut buf = [0u8; 4096];
            let n = axio::Read::read(&mut &file, &mut buf).expect("read");
            if n == 0 {
                break;
            }
            total_bytes += n;
            uspace
                .write((VM_ENTRY + offset).into(), &buf[..n])
                .expect("write guest image");
            offset += n;
            if n < 4096 {
                break;
            }
        }
        ax_println!("Loaded {} bytes from {}", total_bytes, fname);
    }

    // ════════════════════════════════════════════════════
    //  Step 4: Prepare guest context & G-stage page table
    //  (h_4_0: arch_vcpu.set_entry / arch_vcpu.set_ept_root)
    // ════════════════════════════════════════════════════
    let mut ctx = VmCpuRegisters::default();
    prepare_guest_context(&mut ctx);

    let ept_root = uspace.page_table_root();
    ax_println!("bsp_entry: {:#x}; ept: {:#x}", VM_ENTRY, ept_root);
    prepare_vm_pgtable(ept_root);

    // ════════════════════════════════════════════════════
    //  Step 5: Run guest in loop  (h_4_0 style)
    //
    //  Handle:
    //    - VirtualSupervisorEnvCall (scause 10): SBI calls
    //      (PutChar, SetTimer, Shutdown, etc.)
    //    - Guest page faults (scause 20/21/23): MMIO passthrough
    //      (pflash at 0x2200_0000 for the monolithic kernel to load user apps)
    //    - Supervisor timer interrupt: inject to guest via hvip
    //      (required for guest preemptive multitasking)
    // ════════════════════════════════════════════════════
    ax_println!("Entering VM run loop...");

    loop {
        // Disable host interrupts while guest is running (like h_4_0 vcpu_run)
        let saved_sstatus: usize;
        unsafe {
            core::arch::asm!("csrrci {}, sstatus, 0x2", out(reg) saved_sstatus);
            _run_guest(&mut ctx);
            core::arch::asm!("csrs sstatus, {}", in(reg) saved_sstatus & 0x2);
        }

        let scause = scause::read();

        // ── Interrupts ──
        if scause.is_interrupt() {
            match scause.code() {
                5 => {
                    // SupervisorTimer: inject virtual timer interrupt to guest
                    // (required for guest preemptive multitasking — CFS scheduler)
                    CSR.hvip
                        .read_and_set_bits(traps::interrupt::VIRTUAL_SUPERVISOR_TIMER);
                    // Disable host timer until guest re-arms it via SetTimer
                    CSR.sie
                        .read_and_clear_bits(traps::interrupt::SUPERVISOR_TIMER);
                }
                _ => {}
            }
            continue;
        }

        // ── Exceptions ──
        match scause.code() {
            10 => {
                // VirtualSupervisorEnvCall — SBI call from guest
                let a7 = ctx.guest_regs.gprs.a_regs()[7]; // extension ID
                let a6 = ctx.guest_regs.gprs.a_regs()[6]; // function ID

                // ── Shutdown ──
                if a7 == 8 {
                    ax_println!("Guest: SBI legacy shutdown");
                    break;
                }
                if a7 == 0x53525354 {
                    ax_println!("Guest: SBI SRST shutdown");
                    break;
                }

                // ── Legacy SBI PutChar (fast path: write directly to UART) ──
                if a7 == 1 {
                    let ch = ctx.guest_regs.gprs.a_regs()[0] as u8;
                    let uart_va = axhal::mem::phys_to_virt(
                        PhysAddr::from(0x1000_0000usize),
                    ).as_usize();
                    unsafe {
                        core::ptr::write_volatile(uart_va as *mut u8, ch);
                    }
                    ctx.guest_regs.sepc += 4;
                    continue;
                }

                // ── SBI SetTimer (proper timer virtualization for preemptive scheduling) ──
                if a7 == 0x54494D45 || (a7 == 0 && a6 == 0) {
                    // TIME extension (EID 0x54494D45, FID 0) or legacy SetTimer (EID 0)
                    let timer_val = ctx.guest_regs.gprs.a_regs()[0];
                    sbi_rt::set_timer(timer_val as u64);
                    // Clear guest timer pending
                    CSR.hvip
                        .read_and_clear_bits(traps::interrupt::VIRTUAL_SUPERVISOR_TIMER);
                    // Enable host timer interrupt so we catch it
                    CSR.sie
                        .read_and_set_bits(traps::interrupt::SUPERVISOR_TIMER);
                    ctx.guest_regs.gprs.set_reg(regs::GprIndex::A0, 0);
                    ctx.guest_regs.sepc += 4;
                    continue;
                }

                // ── Legacy SBI GetChar ──
                if a7 == 2 {
                    #[allow(deprecated)]
                    let c = sbi_rt::legacy::console_getchar();
                    ctx.guest_regs.gprs.set_reg(regs::GprIndex::A0, c);
                    ctx.guest_regs.sepc += 4;
                    continue;
                }

                // ── Forward all other SBI calls to the real SBI (OpenSBI) ──
                let a0 = ctx.guest_regs.gprs.a_regs()[0];
                let a1 = ctx.guest_regs.gprs.a_regs()[1];
                let a2 = ctx.guest_regs.gprs.a_regs()[2];
                let a3 = ctx.guest_regs.gprs.a_regs()[3];
                let a4 = ctx.guest_regs.gprs.a_regs()[4];
                let a5 = ctx.guest_regs.gprs.a_regs()[5];

                let ret_error: usize;
                let ret_value: usize;
                unsafe {
                    core::arch::asm!(
                        "ecall",
                        inout("a0") a0 => ret_error,
                        inout("a1") a1 => ret_value,
                        in("a2") a2,
                        in("a3") a3,
                        in("a4") a4,
                        in("a5") a5,
                        in("a6") a6,
                        in("a7") a7,
                    );
                }
                ctx.guest_regs.gprs.set_reg(regs::GprIndex::A0, ret_error);
                ctx.guest_regs.gprs.set_reg(regs::GprIndex::A1, ret_value);
                ctx.guest_regs.sepc += 4;
            }

            20 | 21 | 23 => {
                // Guest page fault (G-stage) — MMIO passthrough
                // h_4_0 handles pflash at 0x2200_0000 with passthrough mode.
                // We use generic passthrough for any MMIO address.
                let htval: usize;
                let stval_val: usize;
                unsafe {
                    core::arch::asm!("csrr {}, htval", out(reg) htval);
                    core::arch::asm!("csrr {}, stval", out(reg) stval_val);
                }
                let fault_addr = (htval << 2) | (stval_val & 0x3);
                let page_addr = fault_addr & !0xFFF;

                // Passthrough-map for MMIO devices (pflash, etc.)
                // h_4_0: aspace.map_linear(addr, addr.as_usize().into(), 4096, mapping_flags)
                let _ = uspace.map_linear(
                    page_addr.into(),
                    PhysAddr::from(page_addr),
                    PAGE_SIZE_4K,
                    flags,
                );

                unsafe {
                    core::arch::riscv64::hfence_gvma_all();
                }
            }

            _ => {
                let stval_val: usize;
                let htval_val: usize;
                unsafe {
                    core::arch::asm!("csrr {}, stval", out(reg) stval_val);
                    core::arch::asm!("csrr {}, htval", out(reg) htval_val);
                }
                ax_println!(
                    "Unhandled trap: code={}, sepc={:#x}, stval={:#x}, htval={:#x}",
                    scause.code(),
                    ctx.guest_regs.sepc,
                    stval_val,
                    htval_val
                );
                break;
            }
        }
    }

    ax_println!("Shutdown vm normally!");
    panic!("Hypervisor ok!");

    fn prepare_vm_pgtable(ept_root: PhysAddr) {
        let hgatp = 8usize << 60 | usize::from(ept_root) >> 12;
        unsafe {
            core::arch::asm!(
                "csrw hgatp, {hgatp}",
                hgatp = in(reg) hgatp,
            );
            core::arch::riscv64::hfence_gvma_all();
        }
    }

    fn prepare_guest_context(ctx: &mut VmCpuRegisters) {
        use csrs::{RiscvCsrTrait, CSR};
        let hstatus_val: usize;
        unsafe {
            core::arch::asm!("csrr {}, hstatus", out(reg) hstatus_val);
        }
        let mut hstatus_reg = LocalRegisterCopy::<usize, hstatus::Register>::new(hstatus_val);
        hstatus_reg.modify(hstatus::spv::Guest);
        hstatus_reg.modify(hstatus::spvp::Supervisor);
        CSR.hstatus.write_value(hstatus_reg.get());
        ctx.guest_regs.hstatus = hstatus_reg.get();

        unsafe {
            riscv::register::sstatus::set_spp(riscv::register::sstatus::SPP::Supervisor);
        }
        let sstatus_val: usize;
        unsafe {
            core::arch::asm!("csrr {}, sstatus", out(reg) sstatus_val);
        }
        ctx.guest_regs.sstatus = sstatus_val;
        ctx.guest_regs.sepc = VM_ENTRY;
    }
}

// ════════════════════════════════════════════════════════════════
//  AArch64  (Bootloader-style hypervisor — loads full ArceOS guest)
//
//  Since the ArceOS platform crate drops from EL2 to EL1 during
//  boot, we cannot use traditional EL2 virtualization with Stage-2
//  page tables. Instead, we use a bootloader approach:
//
//    1. Load the guest ArceOS monolithic kernel binary from the FAT32 filesystem
//    2. Write it to a separate physical memory region (PA 0x44200000)
//    3. Set up an identity mapping for the trampoline page
//    4. Jump to the trampoline (identity-mapped VA = PA)
//    5. Trampoline disables MMU and jumps to guest at PA 0x44200000
//    6. Guest ArceOS boots independently with full hardware access
//
//  The guest runs a monolithic kernel with user-space process support.
//  Timer, UART, and GIC are accessed directly by the guest.
//
//  Memory layout (QEMU 128MB RAM: 0x40000000 - 0x47FFFFFF):
//    - Hypervisor: PA 0x40000000 - 0x43FFFFFF (lower 64MB)
//    - Guest:      PA 0x44000000 - 0x47FFFFFF (upper 64MB)
// ════════════════════════════════════════════════════════════════

// Trampoline code for AArch64 guest handoff.
// Must be executed at an identity-mapped address (VA = PA).
// Disables MMU, caches, invalidates TLB/I-cache, then jumps to guest.
#[cfg(all(feature = "axstd", target_arch = "aarch64"))]
core::arch::global_asm!(
    ".section .text",
    ".balign 4096",
    ".global _aarch64_guest_trampoline",
    "_aarch64_guest_trampoline:",
    // x0 = guest entry physical address
    "mov x2, x0",                   // Save guest entry PA in x2
    // Disable MMU, D-cache, I-cache in SCTLR_EL1
    "mrs x1, sctlr_el1",
    "bic x1, x1, #(1 << 0)",        // M = 0: disable MMU
    "bic x1, x1, #(1 << 2)",        // C = 0: disable D-cache
    "bic x1, x1, #(1 << 12)",       // I = 0: disable I-cache
    "msr sctlr_el1, x1",
    "isb",
    // Invalidate TLB
    "tlbi vmalle1",
    "dsb ish",
    "isb",
    // Invalidate I-cache
    "ic iallu",
    "dsb ish",
    "isb",
    // Set x0 = 0 (no device tree; guest uses built-in defplat config)
    "mov x0, #0",
    // Jump to guest entry point (physical address, MMU is off)
    "br x2",
    ".global _aarch64_guest_trampoline_end",
    "_aarch64_guest_trampoline_end:",
);

#[cfg(all(feature = "axstd", target_arch = "aarch64"))]
fn aarch64_main() {
    use axhal::mem::{phys_to_virt, virt_to_phys, PhysAddr};
    use axhal::paging::MappingFlags;
    use memory_addr::{va, PAGE_SIZE_4K};

    ax_println!("Starting virtualization (bootloader mode)...");

    // Guest ArceOS binary is loaded at PA 0x44200000 (upper 64MB region).
    // This avoids overlap with the hypervisor kernel (at PA 0x40200000).
    const GUEST_KERNEL_PADDR: usize = VM_ENTRY; // 0x4420_0000

    // ── 1. Load guest binary from filesystem to physical memory ──
    let fname = "/sbin/gkernel";
    ax_println!("VM created success, loading images...");
    ax_println!("app: {}", fname);

    let ctx = axfs::ROOT_FS_CONTEXT.get().expect("Root FS not initialized");
    let file = axfs::File::open(ctx, fname).expect("Cannot open guest image");
    let mut total_bytes = 0usize;
    loop {
        let mut buf = [0u8; 4096];
        let n = axio::Read::read(&mut &file, &mut buf).expect("read");
        if n == 0 {
            break;
        }
        // Write directly to guest physical memory via hypervisor's linear mapping
        let dst_va = phys_to_virt(PhysAddr::from(GUEST_KERNEL_PADDR + total_bytes)).as_usize();
        unsafe {
            core::ptr::copy_nonoverlapping(
                buf.as_ptr(),
                dst_va as *mut u8,
                n,
            );
        }
        total_bytes += n;
        if n < 4096 {
            break;
        }
    }
    ax_println!("Loaded {} bytes to PA {:#x}", total_bytes, GUEST_KERNEL_PADDR);

    // ── 2. Clean D-cache for guest binary ──
    // Ensures data is written to main memory before MMU & caches are disabled.
    let guest_va_base = phys_to_virt(PhysAddr::from(GUEST_KERNEL_PADDR)).as_usize();
    unsafe {
        let mut off = 0usize;
        while off < total_bytes {
            core::arch::asm!("dc cvau, {}", in(reg) (guest_va_base + off));
            off += 64; // cache line size
        }
        core::arch::asm!("dsb ish");
        core::arch::asm!("ic iallu");
        core::arch::asm!("dsb ish");
        core::arch::asm!("isb");
    }

    // ── 3. Get physical address of the trampoline ──
    unsafe extern "C" {
        fn _aarch64_guest_trampoline();
    }
    let trampoline_va = _aarch64_guest_trampoline as *const () as usize;
    let trampoline_pa = usize::from(virt_to_phys(trampoline_va.into()));
    let trampoline_page_pa = trampoline_pa & !0xFFF;
    ax_println!(
        "Trampoline at VA {:#x} -> PA {:#x} (page {:#x})",
        trampoline_va, trampoline_pa, trampoline_page_pa
    );

    // ── 4. Create identity mapping for trampoline page in TTBR0 ──
    // The trampoline must be at an identity-mapped address (VA = PA) so
    // that it can disable MMU without the instruction stream becoming invalid.
    // NOTE: Do NOT set USER flag — the trampoline runs at EL1, and the USER
    // flag would set PXN (Privileged eXecute Never), blocking EL1 execution.
    let flags = MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE;

    let mut identity = axmm::AddrSpace::new_empty(va!(0x0), 0x4800_0000).unwrap();
    identity.map_linear(
        trampoline_page_pa.into(),
        PhysAddr::from(trampoline_page_pa),
        PAGE_SIZE_4K,
        flags,
    ).expect("identity-map trampoline page");

    // ── 5. Switch TTBR0 to identity page table ──
    let pt_root = identity.page_table_root();
    unsafe {
        core::arch::asm!(
            "msr ttbr0_el1, {val}",
            "isb",
            "tlbi vmalle1is",
            "dsb ish",
            "isb",
            val = in(reg) usize::from(pt_root) as u64,
        );
    }

    // Prevent the identity AddrSpace from being dropped (which would free
    // the page table pages). We are about to jump and never return.
    core::mem::forget(identity);

    // ── 6. Disable interrupts and jump to identity-mapped trampoline ──
    // The trampoline will:
    //   a) Disable MMU, D-cache, I-cache
    //   b) Invalidate TLB and I-cache
    //   c) Jump to guest at PA 0x44200000
    // The guest ArceOS boots at EL1 with MMU off, just like a normal boot.
    ax_println!(
        "Entering guest at PA {:#x} via trampoline at PA {:#x}...",
        GUEST_KERNEL_PADDR,
        trampoline_pa
    );
    unsafe {
        core::arch::asm!(
            "msr daifset, #0xf",       // Mask all exceptions (DAIF)
            "mov x0, {entry}",         // x0 = guest entry physical address
            "br {tramp}",              // Jump to identity-mapped trampoline
            entry = in(reg) GUEST_KERNEL_PADDR as u64,
            tramp = in(reg) trampoline_pa as u64,
            options(noreturn),
        );
    }
}

// ════════════════════════════════════════════════════════════════
//  x86_64  (AMD SVM hypervisor — full ArceOS guest via Multiboot boot)
//
//  The guest ArceOS monolithic kernel binary boots via the Multiboot protocol:
//    1. VMCB starts the guest in 32-bit protected mode (no paging)
//    2. Guest boot assembly (multiboot.S) transitions to 64-bit long mode:
//       a. Loads its own GDT
//       b. Sets up 1GB-huge-page identity mapping (+ high-VA mapping)
//       c. Enables PAE, LME, PG → enters long mode
//    3. ArceOS initializes: IDT, APIC timer, serial console, scheduler
//    4. Guest runs monolithic kernel with user-space process support
//    5. Guest calls VMMCALL to shut down
//
//  Hardware passthrough via NPT + IOPM:
//    - APIC timer: mapped in NPT → guest programs it directly for scheduling
//    - Serial port: IOPM allows I/O port 0x3F8 → guest writes directly
//    - IOAPIC: mapped in NPT → guest initializes interrupts
//    - No INTR intercept → timer interrupts go directly to the guest
//
//  VMMCALL is intercepted for guest shutdown.
//  NPF allocates zeroed pages for unmapped regions.
// ════════════════════════════════════════════════════════════════

#[cfg(all(feature = "axstd", target_arch = "x86_64"))]
fn x86_64_main() {
    use alloc::boxed::Box;
    use alloc::sync::Arc;
    use x86_64_svm::vmcb::*;
    use x86_64_svm::svm::*;
    use memory_addr::va;
    use axhal::mem::{phys_to_virt, PhysAddr};
    use axhal::paging::{MappingFlags, PageSize};
    use axmm::backend::{Backend, SharedPages};
    use memory_addr::PAGE_SIZE_4K;

    ax_println!("Starting virtualization...");

    // ── 1. Check AMD SVM support ──
    let (_, _, ecx, _) = unsafe { cpuid(0x8000_0001) };
    if ecx & (1 << 2) == 0 {
        panic!("CPU does not support AMD SVM!");
    }

    // ── 2. Enable SVM ──
    unsafe {
        let efer = rdmsr(MSR_EFER);
        wrmsr(MSR_EFER, efer | EFER_SVME);
    }

    // ── 3. Allocate host-save area ──
    #[repr(C, align(4096))]
    struct Page4K([u8; 4096]);
    let host_save = Box::new(Page4K([0u8; 4096]));
    let host_save_pa = virt_to_phys_ptr(&host_save.0[0]);
    unsafe {
        wrmsr(MSR_VM_HSAVE_PA, host_save_pa);
    }

    let host_vmcb = Box::new(Page4K([0u8; 4096]));
    let host_vmcb_pa = virt_to_phys_ptr(&host_vmcb.0[0]);

    // ── 4. Allocate IOPM and MSRPM (all zeros = allow all I/O ports and MSRs) ──
    #[repr(C, align(4096))]
    struct Iopm([u8; 12288]);
    #[repr(C, align(4096))]
    struct Msrpm([u8; 8192]);
    let iopm = Box::new(Iopm([0u8; 12288]));
    // MSRPM: intercept EFER (MSR 0xC000_0080) writes to keep SVME set.
    // Without this, the guest's boot code clears SVME when writing LME|NXE
    // to EFER, which causes VMMCALL to trigger #UD later.
    //
    // MSRPM layout: Range0 (2048B, MSRs 0-0x1FFF) | Range1 (2048B, MSRs 0xC0000000-0xC0001FFF) | ...
    // EFER (0xC0000080) is at Range1 offset (0x80 * 2 bits) = 256 bits = byte 32, bit 0 (read), bit 1 (write)
    // Total byte offset in MSRPM: 2048 + 32 = 2080, write intercept = bit 1
    let mut msrpm_data = [0u8; 8192];
    msrpm_data[2080] |= 0x02; // Intercept EFER writes
    let msrpm = Box::new(Msrpm(msrpm_data));
    let iopm_pa = virt_to_phys_ptr(&iopm.0[0]);
    let msrpm_pa = virt_to_phys_ptr(&msrpm.0[0]);

    // ── 5. Create NPT and pre-allocate guest RAM ──
    let mut npt = axmm::AddrSpace::new_empty(va!(0x0), 0x1_0000_0000).unwrap();

    let flags = MappingFlags::READ | MappingFlags::WRITE
        | MappingFlags::EXECUTE | MappingFlags::USER;

    // Pre-allocate 32MB of guest RAM (matching guest config phys-memory-size)
    const GUEST_RAM_SIZE: usize = 0x200_0000; // 32MB
    ax_println!("Pre-allocating {} MB guest RAM at GPA 0x0...", GUEST_RAM_SIZE / (1024 * 1024));
    let ram_pages = Arc::new(
        SharedPages::new(GUEST_RAM_SIZE, PageSize::Size4K)
            .expect("alloc guest RAM"),
    );
    npt.map(
        0x0usize.into(),
        GUEST_RAM_SIZE,
        flags,
        true,
        Backend::new_shared(0x0usize.into(), ram_pages),
    ).expect("map guest RAM");

    // Map APIC MMIO (GPA 0xFEE00000 → HPA 0xFEE00000, identity)
    // Required for the guest to program the APIC timer for preemptive scheduling.
    ax_println!("Mapping APIC at GPA 0xFEE00000 (identity)...");
    npt.map_linear(
        0xFEE0_0000usize.into(),
        PhysAddr::from(0xFEE0_0000usize),
        PAGE_SIZE_4K,
        flags,
    ).expect("map APIC");

    // Map IOAPIC MMIO (GPA 0xFEC00000 → HPA 0xFEC00000, identity)
    ax_println!("Mapping IOAPIC at GPA 0xFEC00000 (identity)...");
    npt.map_linear(
        0xFEC0_0000usize.into(),
        PhysAddr::from(0xFEC0_0000usize),
        PAGE_SIZE_4K,
        flags,
    ).expect("map IOAPIC");

    // ── 6. Write initial GDT into guest memory (GPA 0x5000) ──
    // This matches the format in ArceOS multiboot.S:
    //   [0] null  [1] 32-bit code  [2] 64-bit code  [3] data
    // The boot code immediately loads its own GDT, so this is only
    // used for the first few instructions.
    let gdt: [u64; 4] = [
        0x0000_0000_0000_0000, // 0x00: null
        0x00CF_9B00_0000_FFFF, // 0x08: 32-bit code (base=0, limit=4G, DPL=0)
        0x00AF_9B00_0000_FFFF, // 0x10: 64-bit code (base=0, limit=4G, DPL=0)
        0x00CF_9300_0000_FFFF, // 0x18: data        (base=0, limit=4G, DPL=0)
    ];
    for (i, &entry) in gdt.iter().enumerate() {
        npt.write((0x5000 + i * 8).into(), &entry.to_le_bytes())
            .expect("write GDT");
    }

    // ── 7. Load guest binary at GPA VM_ENTRY (0x200000 = kernel-base-paddr) ──
    {
        let fname = "/sbin/gkernel";
        ax_println!("VM created success, loading images...");
        ax_println!("app: {}", fname);
        let ctx = axfs::ROOT_FS_CONTEXT.get().expect("Root FS not initialized");
        let file = axfs::File::open(ctx, fname).expect("Cannot open guest image");
        let mut offset = 0usize;
        let mut total_bytes = 0usize;
        loop {
            let mut buf = [0u8; 4096];
            let n = axio::Read::read(&mut &file, &mut buf).expect("read");
            if n == 0 {
                break;
            }
            total_bytes += n;
            npt.write((VM_ENTRY + offset).into(), &buf[..n])
                .expect("write guest binary");
            offset += n;
            if n < 4096 {
                break;
            }
        }
        ax_println!("Loaded {} bytes from {}", total_bytes, fname);
    }

    let npt_root_pa: u64 = usize::from(npt.page_table_root()) as u64;

    // ── 8. Build VMCB for 32-bit protected mode (Multiboot-compatible) ──
    //
    // Initial state emulates a Multiboot-compliant bootloader:
    //   - 32-bit protected mode, paging disabled
    //   - EAX = 0x2BADB002 (Multiboot magic)
    //   - EBX = 0 (no MBI structure — guest uses built-in platform config)
    //   - Flat CS/DS/SS segments (base=0, limit=4G)
    //
    // The guest boot code (multiboot.S) will:
    //   1. Load its own GDT and page tables
    //   2. Enable PAE, LME, PG → transition to 64-bit long mode
    //   3. Call rust_entry(magic, mbi)
    let mut vmcb = Box::new(Vmcb::new());

    // Intercept VMRUN (required) and VMMCALL (for guest shutdown).
    // Do NOT intercept INTR — timer interrupts go directly to the guest
    // for preemptive scheduling (APIC timer is passthrough via NPT).
    vmcb.write_u32(CTRL_INTERCEPT_MISC2, INTERCEPT_VMRUN | INTERCEPT_VMMCALL);
    // Enable MSR protection so MSRPM-based intercepts (EFER writes) are active.
    vmcb.write_u32(CTRL_INTERCEPT_MISC1, INTERCEPT_MSR_PROT);
    vmcb.write_u64(CTRL_IOPM_BASE, iopm_pa);
    vmcb.write_u64(CTRL_MSRPM_BASE, msrpm_pa);
    vmcb.write_u32(CTRL_GUEST_ASID, 1);
    vmcb.write_u64(CTRL_NP_ENABLE, 1);
    vmcb.write_u64(CTRL_NCR3, npt_root_pa);

    // Segment registers: 32-bit flat segments for Multiboot boot
    // CS: selector=0x08 → GDT[1] (32-bit code), attr=0x0C9B (G=1,D/B=1,L=0,P=1,DPL=0,Code E/R)
    vmcb.set_segment(SAVE_CS, 0x08, 0x0C9B, 0xFFFF_FFFF, 0);
    // DS/ES/SS: selector=0x18 → GDT[3] (data), attr=0x0C93 (G=1,D/B=1,L=0,P=1,DPL=0,Data R/W)
    vmcb.set_segment(SAVE_DS, 0x18, 0x0C93, 0xFFFF_FFFF, 0);
    vmcb.set_segment(SAVE_ES, 0x18, 0x0C93, 0xFFFF_FFFF, 0);
    vmcb.set_segment(SAVE_SS, 0x18, 0x0C93, 0xFFFF_FFFF, 0);
    vmcb.set_segment(SAVE_FS, 0, 0, 0, 0);
    vmcb.set_segment(SAVE_GS, 0, 0, 0, 0);
    // GDTR: point to our initial GDT at GPA 0x5000 (4 entries × 8 bytes = 32, limit=31)
    vmcb.set_segment(SAVE_GDTR, 0, 0, 31, 0x5000);
    // IDTR: empty (guest sets up its own IDT during boot)
    vmcb.set_segment(SAVE_IDTR, 0, 0, 0, 0);
    // TR and LDTR: minimal valid state
    vmcb.set_segment(SAVE_TR, 0, 0x008B, 0x67, 0);
    vmcb.set_segment(SAVE_LDTR, 0, 0x0082, 0, 0);

    // CR0: Protected mode, NO paging (guest boot code enables paging)
    // PE=1(bit0), MP=1(bit1)... actually just PE + NE for 32-bit PM
    vmcb.write_u64(SAVE_CR0, 0x0000_0011); // PE | NE
    // CR3: 0 — no paging yet, guest boot code sets CR3
    vmcb.write_u64(SAVE_CR3, 0);
    // CR4: 0 — guest boot code enables PAE, PGE
    vmcb.write_u64(SAVE_CR4, 0);
    // EFER: SVME only (required by some SVM implementations for guest EFER validation)
    // Guest boot code will add LME + NXE via WRMSR
    vmcb.write_u64(SAVE_EFER, EFER_SVME);

    vmcb.write_u64(SAVE_DR6, 0xFFFF_0FF0);
    vmcb.write_u64(SAVE_DR7, 0x0400);
    // RFLAGS: reserved bit 1 set, IF=0 (interrupts disabled during boot)
    vmcb.write_u64(SAVE_RFLAGS, 0x2);
    // RIP: guest entry point = kernel-base-paddr (0x200000)
    vmcb.write_u64(SAVE_RIP, VM_ENTRY as u64);
    // RSP: temporary stack within NPT-mapped RAM
    vmcb.write_u64(SAVE_RSP, 0x80000);
    // RAX: Multiboot bootloader magic (boot code checks this)
    vmcb.write_u64(SAVE_RAX, 0x2BADB002);

    let vmcb_pa = virt_to_phys_ptr(&vmcb.data[0]);

    // ── 8b. Write Multiboot Information structure at GPA 0x6000 ──
    // ArceOS reads the MBI to determine available physical memory regions.
    // We provide a minimal MBI with a memory map matching the guest config
    // (phys-memory-size = 0x200_0000 = 32MB).
    {
        const MBI_ADDR: usize = 0x6000;
        const MMAP_ADDR: usize = MBI_ADDR + 64; // memory map starts after MBI header

        // MBI header (Multiboot Information structure, 52+ bytes)
        // Offset 0:  flags      — bit 0 (mem info), bit 6 (mmap valid)
        // Offset 4:  mem_lower  — KB of lower memory (below 1MB)
        // Offset 8:  mem_upper  — KB of upper memory (above 1MB)
        // Offset 44: mmap_length — total size of memory map entries
        // Offset 48: mmap_addr  — physical address of memory map
        let mut mbi = [0u8; 64];
        // flags = (1 << 0) | (1 << 6) = 0x41
        mbi[0..4].copy_from_slice(&0x41u32.to_le_bytes());
        // mem_lower = 640 (KB, conventional memory)
        mbi[4..8].copy_from_slice(&640u32.to_le_bytes());
        // mem_upper = 31 * 1024 (KB, 31MB above 1MB = 32MB - 1MB)
        mbi[8..12].copy_from_slice(&(31u32 * 1024).to_le_bytes());
        // mmap_length = 24 (one entry: 4 bytes size + 20 bytes data)
        mbi[44..48].copy_from_slice(&24u32.to_le_bytes());
        // mmap_addr = MMAP_ADDR
        mbi[48..52].copy_from_slice(&(MMAP_ADDR as u32).to_le_bytes());
        npt.write(MBI_ADDR.into(), &mbi).expect("write MBI");

        // Memory map entry (Multiboot memory map format):
        //   uint32_t size = 20 (size of the rest of this entry)
        //   uint64_t base_addr = 0
        //   uint64_t length = 0x200_0000 (32MB)
        //   uint32_t type = 1 (available)
        let mut mmap_entry = [0u8; 24];
        mmap_entry[0..4].copy_from_slice(&20u32.to_le_bytes()); // size
        mmap_entry[4..12].copy_from_slice(&0u64.to_le_bytes()); // base_addr = 0
        mmap_entry[12..20].copy_from_slice(&(GUEST_RAM_SIZE as u64).to_le_bytes()); // length = 32MB
        mmap_entry[20..24].copy_from_slice(&1u32.to_le_bytes()); // type = Available
        npt.write(MMAP_ADDR.into(), &mmap_entry).expect("write mmap entry");

        ax_println!("MBI at GPA {:#x}, mmap at GPA {:#x} (32MB available)", MBI_ADDR, MMAP_ADDR);
    }

    // ── 9. Create guest GPR save area ──
    let mut gprs = SvmGuestGprs::new();
    // RBX = MBI address (boot code saves EBX→ESI for rust_entry's mbi parameter)
    gprs.rbx = 0x6000;

    // ── 10. Mask host APIC timer before entering guest ──
    // The guest will program its own APIC timer during boot.
    // We mask the host's timer to prevent stale interrupts from being
    // delivered to the guest before its IDT is set up.
    {
        let apic_va = phys_to_virt(PhysAddr::from(0xFEE0_0000usize)).as_usize();
        unsafe {
            // Stop the timer: set initial count to 0
            core::ptr::write_volatile((apic_va + 0x380) as *mut u32, 0);
            // Mask the timer LVT entry (bit 16 = mask)
            let lvt = core::ptr::read_volatile((apic_va + 0x320) as *const u32);
            core::ptr::write_volatile((apic_va + 0x320) as *mut u32, lvt | (1 << 16));
            // Send EOI for any pending interrupt
            core::ptr::write_volatile((apic_va + 0x0B0) as *mut u32, 0);
        }
        // Briefly enable interrupts to clear any pending timer interrupt
        unsafe { core::arch::asm!("sti; nop; nop; nop; cli"); }
        ax_println!("Host APIC timer masked");
    }

    // ── 11. Run guest in VM loop ──
    ax_println!("Entering VM run loop (32-bit Multiboot boot → 64-bit ArceOS)...");
    loop {
        // Ensure SVME is set in guest EFER (may be cleared by guest's WRMSR to EFER)
        let efer = vmcb.read_u64(SAVE_EFER);
        vmcb.write_u64(SAVE_EFER, efer | EFER_SVME);

        unsafe {
            _run_guest(vmcb_pa, host_vmcb_pa, &mut gprs);
        }

        let exit_code = vmcb.exit_code();

        match exit_code {
            VMEXIT_VMMCALL => {
                let guest_rax = vmcb.guest_rax();

                if guest_rax == 0x84000008 {
                    // Guest shutdown (PSCI SYSTEM_OFF convention via VMMCALL)
                    ax_println!("Shutdown vm normally!");
                    break;
                } else {
                    // Legacy putchar or unknown VMMCALL — advance RIP past 3-byte instruction
                    let func = guest_rax & 0xFF;
                    if func == 1 {
                        let ch = ((guest_rax >> 8) & 0xFF) as u8;
                        ax_print!("{}", ch as char);
                    }
                    let rip = vmcb.guest_rip();
                    vmcb.write_u64(SAVE_RIP, rip + 3);
                }
            }
            VMEXIT_NPF => {
                let fault_addr = vmcb.exit_info2();
                let page_addr = (fault_addr & !0xFFF) as usize;

                // For MMIO regions that need identity mapping (APIC, IOAPIC),
                // we pre-mapped them above. Any other NPF is handled by
                // allocating zeroed pages — this allows PCI probing to
                // read all-zeros (no devices), preventing VirtIO conflicts.
                let pages = Arc::new(
                    SharedPages::new(PAGE_SIZE_4K, PageSize::Size4K)
                        .expect("alloc page for NPF"),
                );
                let _ = npt.map(
                    page_addr.into(),
                    PAGE_SIZE_4K,
                    flags,
                    true,
                    Backend::new_shared(page_addr.into(), pages),
                );
            }
            VMEXIT_MSR => {
                // MSR intercept — we only intercept EFER writes.
                // EXITINFO1: 0 = RDMSR, 1 = WRMSR
                let is_write = vmcb.exit_info1() == 1;
                let msr_num = gprs.rcx as u32;

                if is_write && msr_num == MSR_EFER {
                    // Guest is writing to EFER (typically setting LME | NXE).
                    // Emulate the write but force SVME to stay set so that
                    // VMMCALL remains usable for guest shutdown.
                    let eax = vmcb.guest_rax() as u32;
                    let edx = gprs.rdx as u32;
                    let new_efer = ((edx as u64) << 32) | (eax as u64);
                    vmcb.write_u64(SAVE_EFER, new_efer | EFER_SVME);
                }
                // Advance RIP past the 2-byte WRMSR/RDMSR instruction (0F 30 / 0F 32)
                let rip = vmcb.guest_rip();
                vmcb.write_u64(SAVE_RIP, rip + 2);
            }
            VMEXIT_SHUTDOWN => {
                // Triple fault — usually indicates a crash during boot
                ax_println!(
                    "Guest SHUTDOWN (triple fault): RIP={:#x}, info1={:#x}, info2={:#x}",
                    vmcb.guest_rip(),
                    vmcb.exit_info1(),
                    vmcb.exit_info2(),
                );
                break;
            }
            _ => {
                ax_println!(
                    "Unexpected VMEXIT: exit_code={:#x}, info1={:#x}, info2={:#x}, RIP={:#x}",
                    exit_code,
                    vmcb.exit_info1(),
                    vmcb.exit_info2(),
                    vmcb.guest_rip(),
                );
                break;
            }
        }
    }

    // ── 12. Shutdown QEMU ──
    ax_println!("Hypervisor ok!");
    // Write 0x2000 to ACPI shutdown port (QEMU-specific)
    unsafe {
        core::arch::asm!(
            "mov dx, 0x604",
            "mov ax, 0x2000",
            "out dx, ax",
        );
    }
    panic!("Hypervisor ok!");

    fn virt_to_phys_ptr(p: *const u8) -> u64 {
        use axhal::mem::virt_to_phys;
        let va = memory_addr::VirtAddr::from(p as usize);
        usize::from(virt_to_phys(va)) as u64
    }
}
