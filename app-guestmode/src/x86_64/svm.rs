//! AMD SVM helpers: CPUID / MSR wrappers and the VMRUN entry point.

#![allow(dead_code)]

use core::arch::global_asm;

// ── MSR numbers ─────────────────────────────────────────────────
pub const MSR_EFER: u32 = 0xC000_0080;
pub const MSR_VM_HSAVE_PA: u32 = 0xC001_0117;

pub const EFER_SVME: u64 = 1 << 12;

// ── Low-level helpers ───────────────────────────────────────────

#[inline]
pub unsafe fn cpuid(func: u32) -> (u32, u32, u32, u32) {
    let eax: u32;
    let ebx: u32;
    let ecx: u32;
    let edx: u32;
    unsafe {
        core::arch::asm!(
            "push rbx",        // rbx is callee-saved; cpuid clobbers it
            "cpuid",
            "mov {ebx_out:e}, ebx",
            "pop rbx",
            inout("eax") func => eax,
            ebx_out = out(reg) ebx,
            out("ecx") ecx,
            out("edx") edx,
        );
    }
    (eax, ebx, ecx, edx)
}

#[inline]
pub unsafe fn rdmsr(msr: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        core::arch::asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") lo,
            out("edx") hi,
        );
    }
    ((hi as u64) << 32) | (lo as u64)
}

#[inline]
pub unsafe fn wrmsr(msr: u32, val: u64) {
    let lo = val as u32;
    let hi = (val >> 32) as u32;
    unsafe {
        core::arch::asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") lo,
            in("edx") hi,
        );
    }
}

// ── VMRUN wrapper ───────────────────────────────────────────────
//
// `_run_guest(guest_vmcb_pa: u64, host_vmcb_pa: u64)`
//
// System V AMD64 ABI: RDI = guest_vmcb_pa, RSI = host_vmcb_pa.
//
// VMRUN uses RAX = physical address of the guest VMCB.
// On VMEXIT the hardware restores host RSP/RIP/RAX/CS/SS/DS/ES/
// GDTR/IDTR/CR0-4/EFER from the VM_HSAVE_PA area, but does NOT
// restore FS/GS/TR/LDTR.  We use VMSAVE/VMLOAD with a *separate*
// host VMCB page to save and restore those registers.

global_asm!(
    ".global _run_guest",
    "_run_guest:",
    // Save callee-saved GPRs
    "push rbx",
    "push rbp",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    // Save host_vmcb_pa for use after VMEXIT (RSI will be clobbered)
    "push rsi",
    // Disable interrupts so no interrupt fires between VMEXIT and
    // vmload (host FS/GS/TR are still guest values at that point).
    "cli",
    // Save host FS/GS/TR/LDTR/KernelGsBase/STAR/… to host VMCB
    "mov rax, rsi",
    "vmsave",
    // Enter guest
    "mov rax, rdi",
    "vmrun",
    // ── VMEXIT ──
    // RSP is restored by hardware; all other GPRs (except RAX)
    // hold guest values.  Pop host_vmcb_pa from the (restored) stack.
    "pop rax",
    // Restore host FS/GS/TR/LDTR/KernelGsBase/STAR/…
    "vmload",
    // Re-enable interrupts
    "sti",
    // Restore callee-saved GPRs
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop rbp",
    "pop rbx",
    "ret",
);

unsafe extern "C" {
    /// Enter the SVM guest.
    ///
    /// * `guest_vmcb_pa` – physical address of the guest VMCB (4 KB aligned).
    /// * `host_vmcb_pa`  – physical address of a scratch VMCB page used to
    ///   save/restore the host's FS/GS/TR/LDTR via VMSAVE/VMLOAD.
    pub fn _run_guest(guest_vmcb_pa: u64, host_vmcb_pa: u64);
}
