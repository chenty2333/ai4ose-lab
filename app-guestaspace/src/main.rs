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
#[cfg(all(feature = "axstd", target_arch = "aarch64"))]
#[path = "aarch64/mod.rs"]
mod aarch64;

// ────────────────── x86_64 (AMD SVM) specific modules ──────────────────
#[cfg(all(feature = "axstd", target_arch = "x86_64"))]
#[path = "x86_64/mod.rs"]
mod x86_64_svm;

// ────────────────── Common modules ──────────────────
#[cfg(feature = "axstd")]
mod loader;

// VM entry point (guest physical / intermediate-physical address)
#[cfg(all(feature = "axstd", target_arch = "riscv64"))]
const VM_ENTRY: usize = 0x8020_0000;

#[cfg(all(feature = "axstd", target_arch = "aarch64"))]
const VM_ENTRY: usize = 0x4020_0000;

#[cfg(all(feature = "axstd", target_arch = "x86_64"))]
const VM_ENTRY: usize = 0x10000;

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
//  RISC-V 64  (H-extension hypervisor — h_2_0 style)
//  Full OS guest support: SBI forwarding, on-demand NPF mapping
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

    ax_println!("Hypervisor ...");

    // ════════════════════════════════════════════════════
    //  Step 0: Setup H-extension CSRs  (matches riscv_vcpu::setup_csrs)
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
    //  Step 1: Create guest address space
    // ════════════════════════════════════════════════════
    let mut uspace = axmm::AddrSpace::new_empty(va!(0x0), 0x7fff_ffff_f000).unwrap();

    let flags = MappingFlags::READ | MappingFlags::WRITE
        | MappingFlags::EXECUTE | MappingFlags::USER;

    // Check pflash
    // PFlash1 physical address on RISC-V 64 QEMU virt machine.
    // pflash0 @ 0x20000000 (32MB), pflash1 @ 0x22000000 (32MB).
    const PFLASH_START: usize = 0x2200_0000;
    
    // Check pflash
    ax_println!("Reading PFlash at physical address {:#X}...", PFLASH_START);
    let va = axhal::mem::phys_to_virt(PFLASH_START.into()).as_usize();
    let ptr = va as *const u32;
    unsafe {
        ax_println!("Try to access pflash dev region [{:#X}], got {:#X}", va, *ptr);
        let magic = (*ptr).to_ne_bytes();
        ax_println!("Got pflash magic: {}", core::str::from_utf8(&magic).unwrap());
    }

    // ════════════════════════════════════════════════════
    //  Step 2: Pre-allocate guest physical RAM  (like h_2_0 map_alloc)
    //
    //  h_2_0 allocates 16MB at 0x8000_0000 up front.
    //  This eliminates thousands of NPF VM-exits during guest boot.
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
    //  Step 3: Load guest binary into pre-allocated RAM
    //
    //  h_2_0 uses translated_byte_buffer().  We use AddrSpace::write()
    //  which is available in axmm 0.2.2-preview.1.
    // ════════════════════════════════════════════════════
    {
        let fname = "/sbin/gkernel";
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
    // ════════════════════════════════════════════════════
    let mut ctx = VmCpuRegisters::default();
    prepare_guest_context(&mut ctx);

    let ept_root = uspace.page_table_root();
    prepare_vm_pgtable(ept_root);

    // ════════════════════════════════════════════════════
    //  Step 5: Run guest in loop  (h_2_0 style)
    //
    //  Handle:
    //    - VirtualSupervisorEnvCall (scause 10): SBI calls
    //    - Guest page faults (scause 20/21/23): MMIO passthrough
    //    - Supervisor timer interrupt: inject to guest via hvip
    // ════════════════════════════════════════════════════
    ax_println!("Entering VM run loop...");

    loop {
        // Disable host interrupts while guest is running (like h_2_0 vcpu_run)
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

                // ── SBI SetTimer (proper timer virtualization) ──
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
                // Guest page fault (G-stage) — should only be MMIO now
                // since all RAM is pre-allocated.
                let htval: usize;
                let stval_val: usize;
                unsafe {
                    core::arch::asm!("csrr {}, htval", out(reg) htval);
                    core::arch::asm!("csrr {}, stval", out(reg) stval_val);
                }
                let fault_addr = (htval << 2) | (stval_val & 0x3);
                let page_addr = fault_addr & !0xFFF;

                // Passthrough-map for MMIO devices (pflash, etc.)
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
//  AArch64  (EL1 hypervisor — bare-metal guest at EL0)
//
//  Since the ArceOS platform crate drops from EL2 to EL1 during
//  boot, the hypervisor runs at EL1 and the guest at EL0.
//  The guest uses SVC hypercalls for console I/O and shutdown.
//  Data aborts from EL0 (page faults) are used to demonstrate
//  on-demand page mapping (analogous to stage-2 page faults).
// ════════════════════════════════════════════════════════════════

#[cfg(all(feature = "axstd", target_arch = "aarch64"))]
fn aarch64_main() {
    use alloc::sync::Arc;
    use aarch64::vcpu::VmCpuRegisters;
    use loader::load_vm_image;
    use memory_addr::va;
    use axhal::paging::{MappingFlags, PageSize};
    use axhal::mem::PhysAddr;
    use axmm::backend::{Backend, SharedPages};
    use memory_addr::PAGE_SIZE_4K;

    ax_println!("Hypervisor ...");

    // ── 1. Create guest address space ──
    // Must cover pflash (0x04000000) and guest code (0x40200000) + stack
    let mut uspace = axmm::AddrSpace::new_empty(va!(0x0), 0x4200_0000).unwrap();

    let flags = MappingFlags::READ | MappingFlags::WRITE
        | MappingFlags::EXECUTE | MappingFlags::USER;

    // ── 2. Load guest binary ──
    if let Err(e) = load_vm_image("/sbin/gkernel", &mut uspace) {
        panic!("Cannot load app! {:?}", e);
    }

    // ── 3. Allocate guest stack ──
    const STACK_SIZE: usize = 0x8000; // 32KB
    const STACK_BASE: usize = 0x4100_0000;
    const STACK_TOP: usize = STACK_BASE + STACK_SIZE;
    let stack_pages = Arc::new(
        SharedPages::new(STACK_SIZE, PageSize::Size4K)
            .expect("alloc guest stack"),
    );
    uspace
        .map(
            STACK_BASE.into(),
            STACK_SIZE,
            flags,
            true,
            Backend::new_shared(STACK_BASE.into(), stack_pages),
        )
        .expect("map guest stack");
    ax_println!("Guest stack: {:#x} - {:#x}", STACK_BASE, STACK_TOP);

    // ── 4. Switch TTBR0_EL1 to guest page table ──
    let pt_root = uspace.page_table_root();
    let new_ttbr0: u64 = usize::from(pt_root) as u64;
    let old_ttbr0: u64;
    unsafe {
        core::arch::asm!("mrs {}, ttbr0_el1", out(reg) old_ttbr0);
        core::arch::asm!(
            "msr ttbr0_el1, {val}",
            "isb",
            "tlbi vmalle1is",
            "dsb ish",
            "isb",
            val = in(reg) new_ttbr0,
        );
    }

    // ── 5. Prepare guest context ──
    let mut ctx = VmCpuRegisters::default();
    ctx.guest.elr = VM_ENTRY as u64;
    ctx.guest.spsr = 0x3C0; // EL0t, DAIF masked
    ctx.guest.sp = STACK_TOP as u64;

    // ── 6. Run guest in loop ──
    ax_println!("Entering VM run loop...");
    loop {
        unsafe {
            aarch64::vcpu::_run_guest(&mut ctx);
        }

        // Check if exit was caused by an IRQ/FIQ/SError (not a synchronous exception).
        // On AArch64, when an IRQ targets EL1 while executing at EL0, the CPU takes
        // the interrupt regardless of EL0's DAIF masks. ESR_EL1 is NOT updated for
        // asynchronous exceptions, so we must distinguish them via the vector entry.
        if ctx.trap.is_irq != 0 {
            // Asynchronous exit (IRQ/FIQ/SError) — just re-enter the guest.
            // Do NOT interpret ESR or advance ELR.
            continue;
        }

        let esr = ctx.trap.esr;
        let ec = (esr >> 26) & 0x3F;

        match ec {
            0x15 => {
                // SVC from EL0 — Hypercall
                // ABI: x8 = function ID, x0 = argument
                //
                // NOTE: On AArch64, ELR_EL1 for SVC already points to the
                // instruction AFTER the SVC (the "preferred return address").
                // This differs from RISC-V where sepc points to the ecall itself.
                // Therefore we do NOT advance ELR here.
                let func = ctx.guest.gprs.0[8]; // x8
                match func {
                    1 => {
                        // putchar: x0 = character
                        let ch = ctx.guest.gprs.0[0] as u8;
                        ax_print!("{}", ch as char);
                    }
                    2 => {
                        // exit
                        ax_println!("Shutdown vm normally!");
                        break;
                    }
                    _ => {}
                }
            }
            0x24 => {
                // Data abort from lower EL (EL0) — page fault
                // This demonstrates on-demand page mapping analogous to
                // nested page fault handling in true hypervisors.
                let far = ctx.trap.far;
                let page_addr = (far & !0xFFF) as usize;

                // Passthrough map: VA -> PA (same address)
                // Works for QEMU pflash at 0x04000000 and other MMIO
                let _ = uspace.map_linear(
                    page_addr.into(),
                    PhysAddr::from(page_addr),
                    PAGE_SIZE_4K,
                    flags,
                );

                // Flush TLB
                unsafe {
                    core::arch::asm!(
                        "tlbi vmalle1is",
                        "dsb ish",
                        "isb",
                    );
                }
            }
            _ => {
                ax_println!(
                    "Unhandled trap: EC={:#x}, ESR={:#x}, ELR={:#x}, FAR={:#x}",
                    ec, esr, ctx.guest.elr, ctx.trap.far
                );
                break;
            }
        }
    }

    // ── 7. Restore TTBR0_EL1 ──
    unsafe {
        core::arch::asm!(
            "msr ttbr0_el1, {val}",
            "isb",
            "tlbi vmalle1is",
            "dsb ish",
            "isb",
            val = in(reg) old_ttbr0,
        );
    }

    ax_println!("Hypervisor ok!");
    // Shutdown QEMU via PSCI SYSTEM_OFF (SMC at EL3)
    unsafe {
        core::arch::asm!(
            "movz x0, #0x0008",
            "movk x0, #0x8400, lsl #16",
            "smc  #0",
            options(noreturn)
        );
    }
}

// ════════════════════════════════════════════════════════════════
//  x86_64  (AMD SVM hypervisor — long-mode guest with NPT)
//
//  The guest runs in 64-bit long mode inside an SVM container.
//  The hypervisor creates initial page tables, GDT, and VMCB for
//  the guest, then uses VMRUN to execute it.
//
//  Nested Page Tables (NPT) provide GPA→HPA translation.
//  Guest page tables provide GVA→GPA translation.
//  Two-stage translation: GVA→GPA→HPA.
//
//  VMMCALL hypercalls are used for console I/O and shutdown.
//  NPF (Nested Page Fault) is used for pflash emulation.
// ════════════════════════════════════════════════════════════════

#[cfg(all(feature = "axstd", target_arch = "x86_64"))]
fn x86_64_main() {
    use alloc::boxed::Box;
    use alloc::sync::Arc;
    use x86_64_svm::vmcb::*;
    use x86_64_svm::svm::*;
    use memory_addr::va;
    use axhal::paging::{MappingFlags, PageSize};
    use axmm::backend::{Backend, SharedPages};
    use memory_addr::PAGE_SIZE_4K;

    ax_println!("Hypervisor ...");

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

    // Host VMCB for FS/GS/TR/LDTR save/restore
    let host_vmcb = Box::new(Page4K([0u8; 4096]));
    let host_vmcb_pa = virt_to_phys_ptr(&host_vmcb.0[0]);

    // ── 4. Allocate IOPM and MSRPM ──
    #[repr(C, align(4096))]
    struct Iopm([u8; 12288]);
    #[repr(C, align(4096))]
    struct Msrpm([u8; 8192]);
    let iopm = Box::new(Iopm([0u8; 12288]));  // all zeros = allow all I/O
    let msrpm = Box::new(Msrpm([0u8; 8192])); // all zeros = allow all MSRs
    let iopm_pa = virt_to_phys_ptr(&iopm.0[0]);
    let msrpm_pa = virt_to_phys_ptr(&msrpm.0[0]);

    // ── 5. Create NPT and pre-allocate guest RAM ──
    // Range covers both low memory (code, page tables, stack) and pflash
    let mut npt = axmm::AddrSpace::new_empty(va!(0x0), 0x1_0000_0000).unwrap();

    let flags = MappingFlags::READ | MappingFlags::WRITE
        | MappingFlags::EXECUTE | MappingFlags::USER;

    // Pre-allocate 2MB of guest RAM at GPA 0x0
    // This covers: page tables (0x1000-0x5000), GDT (0x5000),
    //              guest code (0x10000), and stack (up to 0x80000)
    const GUEST_RAM_SIZE: usize = 0x20_0000; // 2MB
    ax_println!("Pre-allocating {} KB guest RAM at GPA 0x0...", GUEST_RAM_SIZE / 1024);
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

    // ── 6. Write guest page tables into NPT-mapped memory ──
    // Guest paging: GVA → GPA (identity mapping for first 2MB + pflash)
    //
    // PML4 at GPA 0x1000:
    //   [0] → PDPT at GPA 0x2000
    //
    // PDPT at GPA 0x2000:
    //   [0] → PD0 at GPA 0x3000  (first 1GB)
    //   [3] → PD3 at GPA 0x4000  (3–4GB range, for pflash at 0xFFC00000)
    //
    // PD0 at GPA 0x3000:
    //   [0] = 2MB identity page: GVA 0x0–0x200000 → GPA 0x0–0x200000
    //
    // PD3 at GPA 0x4000:
    //   [510] = 2MB page: GVA 0xFFC00000 → GPA 0xFFC00000  (pflash)

    const PTE_PRESENT: u64 = 1;
    const PTE_RW: u64 = 1 << 1;
    const PTE_USER: u64 = 1 << 2;
    const PTE_PS: u64 = 1 << 7; // Page Size (2MB huge page)
    const PT_FLAGS: u64 = PTE_PRESENT | PTE_RW | PTE_USER;

    // PML4[0] → PDPT
    npt.write(0x1000usize.into(), &(0x2000u64 | PT_FLAGS).to_le_bytes())
        .expect("write PML4");

    // PDPT[0] → PD0, PDPT[3] → PD3
    npt.write(0x2000usize.into(), &(0x3000u64 | PT_FLAGS).to_le_bytes())
        .expect("write PDPT[0]");
    npt.write((0x2000 + 3 * 8usize).into(), &(0x4000u64 | PT_FLAGS).to_le_bytes())
        .expect("write PDPT[3]");

    // PD0[0] = 2MB identity page at GPA 0x0
    npt.write(0x3000usize.into(), &(0x0u64 | PT_FLAGS | PTE_PS).to_le_bytes())
        .expect("write PD0[0]");

    // PD3[510] = 2MB page at GPA 0xFFC00000 (pflash)
    npt.write((0x4000 + 510 * 8usize).into(), &(0xFFC0_0000u64 | PT_FLAGS | PTE_PS).to_le_bytes())
        .expect("write PD3[510]");

    // ── 7. Write GDT into guest memory (GPA 0x5000) ──
    // [0] Null, [1] 32-bit code, [2] 64-bit code (L=1), [3] Data
    let gdt: [u64; 4] = [
        0x0000_0000_0000_0000, // 0x00: null
        0x00CF_9B00_0000_FFFF, // 0x08: 32-bit code (not used, placeholder)
        0x00AF_9B00_0000_FFFF, // 0x10: 64-bit code (L=1, D=0, G=1)
        0x00CF_9300_0000_FFFF, // 0x18: data (R/W, G=1)
    ];
    for (i, &entry) in gdt.iter().enumerate() {
        npt.write((0x5000 + i * 8).into(), &entry.to_le_bytes())
            .expect("write GDT");
    }

    // ── 8. Load guest binary at GPA VM_ENTRY (0x10000) ──
    {
        let fname = "/sbin/gkernel";
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

    // ── 9. Build VMCB for 64-bit long mode ──
    let mut vmcb = Box::new(Vmcb::new());

    // Control area — intercept VMRUN and VMMCALL; enable NPT
    vmcb.write_u32(CTRL_INTERCEPT_MISC2, INTERCEPT_VMRUN | INTERCEPT_VMMCALL);
    vmcb.write_u64(CTRL_IOPM_BASE, iopm_pa);
    vmcb.write_u64(CTRL_MSRPM_BASE, msrpm_pa);
    vmcb.write_u32(CTRL_GUEST_ASID, 1);
    vmcb.write_u64(CTRL_NP_ENABLE, 1);
    vmcb.write_u64(CTRL_NCR3, npt_root_pa);

    // Save area — 64-bit long-mode guest
    // CS: 64-bit code segment (GDT offset 0x10)
    // Attrib: P=1 DPL=0 S=1 Type=0xB | L=1 D=0 G=1 = 0x0A9B
    vmcb.set_segment(SAVE_CS, 0x10, 0x0A9B, 0xFFFF_FFFF, 0);
    // DS/ES/SS: data segment (GDT offset 0x18)
    vmcb.set_segment(SAVE_DS, 0x18, 0x0C93, 0xFFFF_FFFF, 0);
    vmcb.set_segment(SAVE_ES, 0x18, 0x0C93, 0xFFFF_FFFF, 0);
    vmcb.set_segment(SAVE_SS, 0x18, 0x0C93, 0xFFFF_FFFF, 0);
    vmcb.set_segment(SAVE_FS, 0, 0, 0, 0);
    vmcb.set_segment(SAVE_GS, 0, 0, 0, 0);
    // GDTR: GDT at GPA 0x5000, 4 entries (32 bytes), limit = 31
    vmcb.set_segment(SAVE_GDTR, 0, 0, 31, 0x5000);
    // IDTR: no IDT needed for simple payload
    vmcb.set_segment(SAVE_IDTR, 0, 0, 0xFFF, 0);
    // TR: required but minimal
    vmcb.set_segment(SAVE_TR, 0, 0x008B, 0x67, 0);
    vmcb.set_segment(SAVE_LDTR, 0, 0x0082, 0, 0);

    // CR0: PE | ET | WP | PG (protected mode + paging)
    vmcb.write_u64(SAVE_CR0, 0x8001_0011);
    // CR3: PML4 at GPA 0x1000
    vmcb.write_u64(SAVE_CR3, 0x1000);
    // CR4: PAE | PGE
    vmcb.write_u64(SAVE_CR4, 0x00A0);
    // EFER: SVME | LME | LMA | NXE
    vmcb.write_u64(SAVE_EFER, EFER_SVME | (1 << 8) | (1 << 10) | (1 << 11));

    vmcb.write_u64(SAVE_DR6, 0xFFFF_0FF0);
    vmcb.write_u64(SAVE_DR7, 0x0400);
    vmcb.write_u64(SAVE_RFLAGS, 0x2);
    // RIP: guest entry point
    vmcb.write_u64(SAVE_RIP, VM_ENTRY as u64);
    // RSP: stack at 0x80000 (grows down, within the pre-allocated 2MB)
    vmcb.write_u64(SAVE_RSP, 0x80000);

    let vmcb_pa = virt_to_phys_ptr(&vmcb.data[0]);

    // ── 10. Create guest GPR save area ──
    let mut gprs = SvmGuestGprs::new();

    // ── 11. Run guest in loop ──
    ax_println!("Entering VM run loop...");
    loop {
        unsafe {
            _run_guest(vmcb_pa, host_vmcb_pa, &mut gprs);
        }

        let exit_code = vmcb.exit_code();

        match exit_code {
            VMEXIT_VMMCALL => {
                let guest_rax = vmcb.guest_rax();
                let func = guest_rax & 0xFF;

                if guest_rax == 0x84000008 {
                    // Exit (PSCI SYSTEM_OFF convention)
                    ax_println!("Shutdown vm normally!");
                    break;
                } else if func == 1 {
                    // Putchar: character in bits [15:8] of RAX
                    let ch = ((guest_rax >> 8) & 0xFF) as u8;
                    ax_print!("{}", ch as char);
                    // Advance RIP past the 3-byte VMMCALL instruction
                    let rip = vmcb.guest_rip();
                    vmcb.write_u64(SAVE_RIP, rip + 3);
                } else {
                    let rip = vmcb.guest_rip();
                    vmcb.write_u64(SAVE_RIP, rip + 3);
                }
            }
            VMEXIT_NPF => {
                let fault_addr = vmcb.exit_info2();
                let page_addr = (fault_addr & !0xFFF) as usize;

                // Check if this is the pflash region (0xFFC00000)
                // Emulate pflash by writing "pfld" magic into allocated page
                let is_pflash = page_addr >= 0xFFC0_0000 && page_addr < 0x1_0000_0000;

                let pages = Arc::new(
                    SharedPages::new(PAGE_SIZE_4K, PageSize::Size4K)
                        .expect("alloc page for NPF"),
                );
                npt.map(
                    page_addr.into(),
                    PAGE_SIZE_4K,
                    flags,
                    true,
                    Backend::new_shared(page_addr.into(), pages),
                ).expect("map NPF page");

                if is_pflash {
                    // Write pflash magic "pfld" = 0x646c6670 (little-endian)
                    npt.write(page_addr.into(), &0x646c6670u32.to_le_bytes())
                        .expect("write pflash magic");
                }
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

    ax_println!("Hypervisor ok!");

    // Shutdown QEMU via ACPI
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
