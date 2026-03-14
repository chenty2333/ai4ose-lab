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

// x86_64: guest physical address for SVM real-mode entry
#[cfg(all(feature = "axstd", target_arch = "x86_64"))]
const VM_ENTRY: usize = 0x10000;

// Fallback for unsupported target archs (e.g. host-side builds with axstd)
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
//  RISC-V 64  (H-extension hypervisor)
// ════════════════════════════════════════════════════════════════

#[cfg(all(feature = "axstd", target_arch = "riscv64"))]
fn riscv64_main() {
    use vcpu::VmCpuRegisters;
    use riscv::register::scause;
    use csrs::defs::hstatus;
    use tock_registers::LocalRegisterCopy;
    use csrs::{RiscvCsrTrait, CSR};
    use vcpu::_run_guest;
    use sbi::SbiMessage;
    use loader::load_vm_image;
    use axhal::mem::PhysAddr;
    use memory_addr::va;

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

    ax_println!("Hypervisor ...");

    // A new address space for vm.
    let mut uspace = axmm::AddrSpace::new_empty(va!(0x8000_0000), 0x800_0000).unwrap();

    // Copy kernel page table entries so kernel code is accessible.
    uspace
        .copy_mappings_from(&axmm::kernel_aspace().lock())
        .unwrap();

    // Load vm binary file into address space.
    if let Err(e) = load_vm_image("/sbin/skernel", &mut uspace) {
        panic!("Cannot load app! {:?}", e);
    }

    // Setup context to prepare to enter guest mode.
    let mut ctx = VmCpuRegisters::default();
    prepare_guest_context(&mut ctx);

    // Setup pagetable for 2nd address mapping.
    let ept_root = uspace.page_table_root();
    prepare_vm_pgtable(ept_root);

    // Kick off vm and wait for it to exit.
    run_guest(&mut ctx);

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

    fn run_guest(ctx: &mut VmCpuRegisters) {
        unsafe {
            _run_guest(ctx);
        }

        vmexit_handler(ctx)
    }

    fn vmexit_handler(ctx: &VmCpuRegisters) {
        let scause = scause::read();
        // VirtualSupervisorEnvCall = exception code 10
        // riscv 0.11 does not include this variant, so match raw code
        if scause.is_exception() && scause.code() == 10 {
            let sbi_msg = SbiMessage::from_regs(ctx.guest_regs.gprs.a_regs()).ok();
            // ax_println!("VmExit Reason: VSuperEcall: {:?}", sbi_msg);
            if let Some(msg) = sbi_msg {
                match msg {
                    SbiMessage::Reset(_) => {
                        ax_println!("Guest: SBI SRST shutdown");
                        ax_println!("Shutdown vm normally!");
                    },
                    _ => todo!(),
                }
            } else {
                panic!("bad sbi message! ");
            }
        } else {
            panic!(
                "Unhandled trap: {:?}, sepc: {:#x}, stval: {:#x}",
                scause.cause(),
                ctx.guest_regs.sepc,
                ctx.trap_csrs.stval
            );
        }
    }

    fn prepare_guest_context(ctx: &mut VmCpuRegisters) {
        // Set hstatus via custom CSR access (riscv 0.11 lacks hstatus module)
        let hstatus_val: usize;
        unsafe {
            core::arch::asm!("csrr {}, hstatus", out(reg) hstatus_val);
        }
        let mut hstatus_reg = LocalRegisterCopy::<usize, hstatus::Register>::new(hstatus_val);
        // Set Guest bit in order to return to guest mode.
        hstatus_reg.modify(hstatus::spv::Guest);
        // Set SPVP bit in order to accessing VS-mode memory from HS-mode.
        hstatus_reg.modify(hstatus::spvp::Supervisor);
        CSR.hstatus.write_value(hstatus_reg.get());
        ctx.guest_regs.hstatus = hstatus_reg.get();

        // Set sstatus: set SPP to Supervisor mode
        unsafe {
            riscv::register::sstatus::set_spp(riscv::register::sstatus::SPP::Supervisor);
        }
        // Read back sstatus raw value
        let sstatus_val: usize;
        unsafe {
            core::arch::asm!("csrr {}, sstatus", out(reg) sstatus_val);
        }
        ctx.guest_regs.sstatus = sstatus_val;
        // Return to entry to start vm.
        ctx.guest_regs.sepc = VM_ENTRY;
    }
}

// ════════════════════════════════════════════════════════════════
//  AArch64  (EL1 hypervisor — guest runs at EL0)
// ════════════════════════════════════════════════════════════════

#[cfg(all(feature = "axstd", target_arch = "aarch64"))]
fn aarch64_main() {
    use aarch64::vcpu::VmCpuRegisters;
    use aarch64::hvc::GuestMessage;
    use loader::load_vm_image;
    use memory_addr::va;

    ax_println!("Hypervisor ...");

    // Create guest address space (user-mode VA range).
    // On aarch64 QEMU virt, physical RAM starts at 0x4000_0000.
    let mut uspace = axmm::AddrSpace::new_empty(va!(0x4000_0000), 0x800_0000).unwrap();

    // Load guest binary into the address space.
    if let Err(e) = load_vm_image("/sbin/skernel", &mut uspace) {
        panic!("Cannot load app! {:?}", e);
    }

    // ── Switch TTBR0_EL1 to guest page table ──
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

    // ── Prepare guest context ──
    let mut ctx = VmCpuRegisters::default();
    ctx.guest.elr = VM_ENTRY as u64;
    // SPSR_EL1: EL0t (M[3:0]=0b0000), DAIF masked (bits [9:6])
    ctx.guest.spsr = 0x3C0;

    // ── Enter guest (EL0) ──
    unsafe {
        aarch64::vcpu::_run_guest(&mut ctx);
    }

    // ── Restore TTBR0_EL1 ──
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

    // ── Handle VM exit ──
    vmexit_handler(&ctx);

    ax_println!("Hypervisor ok!");
    // Shutdown QEMU via PSCI SYSTEM_OFF.
    // With `-machine virt,virtualization=on`, PSCI is handled at EL3 via SMC
    // (the EL2 stub does not forward HVC-based PSCI calls).
    unsafe {
        core::arch::asm!(
            "movz x0, #0x0008",
            "movk x0, #0x8400, lsl #16",   // x0 = 0x84000008 (PSCI_SYSTEM_OFF)
            "smc  #0",
            options(noreturn)
        );
    }

    fn vmexit_handler(ctx: &VmCpuRegisters) {
        let esr = ctx.trap.esr;
        let ec = (esr >> 26) & 0x3F;
        match GuestMessage::from_esr_and_regs(esr, &ctx.guest.gprs.0) {
            Ok(msg) => {
                ax_println!("VmExit Reason: GuestSVC: Some({:?})", msg);
                match msg {
                    GuestMessage::PsciSystemOff | GuestMessage::PsciSystemReset => {
                        ax_println!("Shutdown vm normally!");
                    }
                    GuestMessage::Unknown(fid) => {
                        ax_println!("Unknown guest call: {:#x}", fid);
                    }
                }
            }
            Err(_) => {
                ax_println!(
                    "Unhandled trap: EC={:#x}, ESR={:#x}, ELR={:#x}, FAR={:#x}",
                    ec, esr, ctx.guest.elr, ctx.trap.far
                );
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════
//  x86_64  (AMD SVM hypervisor — real-mode guest)
// ════════════════════════════════════════════════════════════════

#[cfg(all(feature = "axstd", target_arch = "x86_64"))]
fn x86_64_main() {
    use alloc::boxed::Box;
    use x86_64_svm::vmcb::*;
    use x86_64_svm::svm::*;
    use loader::load_vm_image;
    use memory_addr::va;

    ax_println!("Hypervisor ...");

    // ── 1. Check AMD SVM support (CPUID 0x8000_0001, ECX bit 2) ──
    let (_, _, ecx, _) = unsafe { cpuid(0x8000_0001) };
    if ecx & (1 << 2) == 0 {
        panic!("CPU does not support AMD SVM!");
    }

    // ── 2. Enable SVM (set EFER.SVME) ──
    unsafe {
        let efer = rdmsr(MSR_EFER);
        wrmsr(MSR_EFER, efer | EFER_SVME);
    }

    // ── 3. Allocate host-save area and tell the CPU ──
    #[repr(C, align(4096))]
    struct Page4K([u8; 4096]);
    let host_save = Box::new(Page4K([0u8; 4096]));
    let host_save_pa = virt_to_phys_ptr(&host_save.0[0]);
    unsafe {
        wrmsr(MSR_VM_HSAVE_PA, host_save_pa);
    }

    // ── 3b. Allocate a scratch VMCB for host extra state (FS/GS/TR/LDTR) ──
    // VMEXIT does NOT restore host FS/GS/TR/LDTR — we use VMSAVE/VMLOAD
    // with this separate page to preserve them across VM entry/exit.
    let host_vmcb = Box::new(Page4K([0u8; 4096]));
    let host_vmcb_pa = virt_to_phys_ptr(&host_vmcb.0[0]);

    // ── 4. Allocate IOPM (12 KB) and MSRPM (8 KB) ──
    #[repr(C, align(4096))]
    struct Iopm([u8; 12288]);
    #[repr(C, align(4096))]
    struct Msrpm([u8; 8192]);
    let iopm = Box::new(Iopm([0u8; 12288]));
    let msrpm = Box::new(Msrpm([0u8; 8192]));
    let iopm_pa = virt_to_phys_ptr(&iopm.0[0]);
    let msrpm_pa = virt_to_phys_ptr(&msrpm.0[0]);

    // ── 5. Create NPT (nested page table) and load guest binary ──
    let mut npt = axmm::AddrSpace::new_empty(va!(VM_ENTRY), 0x100_0000).unwrap();
    if let Err(e) = load_vm_image("/sbin/skernel", &mut npt) {
        panic!("Cannot load app! {:?}", e);
    }
    let npt_root_pa: u64 = usize::from(npt.page_table_root()) as u64;

    // ── 6. Build VMCB ──
    let mut vmcb = Box::new(Vmcb::new());

    // Control area — VMRUN/VMMCALL intercepts are at offset 0x010 (misc2)
    vmcb.write_u32(CTRL_INTERCEPT_MISC2, INTERCEPT_VMRUN | INTERCEPT_VMMCALL);
    vmcb.write_u64(CTRL_IOPM_BASE, iopm_pa);
    vmcb.write_u64(CTRL_MSRPM_BASE, msrpm_pa);
    vmcb.write_u32(CTRL_GUEST_ASID, 1);
    vmcb.write_u64(CTRL_NP_ENABLE, 1);      // enable nested paging
    vmcb.write_u64(CTRL_NCR3, npt_root_pa);  // nested page table root

    // Save area — 16-bit real-mode guest
    // CS: base = VM_ENTRY, so RIP = 0 → first instruction at GPA VM_ENTRY
    vmcb.set_segment(SAVE_CS, (VM_ENTRY >> 4) as u16, 0x009B, 0xFFFF, VM_ENTRY as u64);
    vmcb.set_segment(SAVE_DS, 0, 0x0093, 0xFFFF, 0);
    vmcb.set_segment(SAVE_ES, 0, 0x0093, 0xFFFF, 0);
    vmcb.set_segment(SAVE_SS, 0, 0x0093, 0xFFFF, 0);
    vmcb.set_segment(SAVE_FS, 0, 0x0093, 0xFFFF, 0);
    vmcb.set_segment(SAVE_GS, 0, 0x0093, 0xFFFF, 0);
    // GDTR / IDTR
    vmcb.set_segment(SAVE_GDTR, 0, 0, 0xFFFF, 0);
    vmcb.set_segment(SAVE_IDTR, 0, 0, 0x3FF, 0);
    // TR: 32-bit TSS busy (type=0xB, S=0, P=1) — required by VMRUN
    vmcb.set_segment(SAVE_TR, 0, 0x008B, 0xFFFF, 0);
    // LDTR
    vmcb.set_segment(SAVE_LDTR, 0, 0x0082, 0, 0);

    // System registers
    vmcb.write_u64(SAVE_EFER, EFER_SVME); // guest EFER.SVME must be 1
    vmcb.write_u64(SAVE_CR0, 0x10);       // ET=1, real mode
    vmcb.write_u64(SAVE_DR6, 0xFFFF_0FF0);
    vmcb.write_u64(SAVE_DR7, 0x0400);
    vmcb.write_u64(SAVE_RFLAGS, 0x2);     // bit 1 always set, IF=0
    vmcb.write_u64(SAVE_RIP, 0);          // offset within CS (CS.base = VM_ENTRY)

    // ── 7. Execute VMRUN ──
    let vmcb_pa = virt_to_phys_ptr(&vmcb.data[0]);

    ax_println!("paddr: PA:{:#x}", vmcb_pa);

    unsafe {
        _run_guest(vmcb_pa, host_vmcb_pa);
    }

    // ── 8. Handle VMEXIT ──
    let exit_code = vmcb.exit_code();
    let guest_rax = vmcb.guest_rax();

    match exit_code {
        VMEXIT_VMMCALL => {
            ax_println!("VmExit Reason: VMMCALL");
            match guest_rax {
                0x84000008 => {
                    ax_println!("Shutdown vm normally!");
                }
                _ => {
                    ax_println!("Unknown VMMCALL function: {:#x}", guest_rax);
                }
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
        }
    }

    ax_println!("Hypervisor ok!");

    // Shutdown QEMU — use ACPI PM1a_CNT (I/O port 0x604, SLP_EN=1).
    // On QEMU q35/i440fx this powers off the virtual machine.
    unsafe {
        core::arch::asm!(
            "mov dx, 0x604",
            "mov ax, 0x2000",
            "out dx, ax",
        );
    }
    // If ACPI shutdown didn't work, fall through to panic
    panic!("Hypervisor ok!");

    /// Convert a kernel virtual address (pointer) to physical address.
    fn virt_to_phys_ptr(p: *const u8) -> u64 {
        use axhal::mem::virt_to_phys;
        let va = memory_addr::VirtAddr::from(p as usize);
        usize::from(virt_to_phys(va)) as u64
    }
}
