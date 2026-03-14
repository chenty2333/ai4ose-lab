//! AMD SVM helpers: CPUID / MSR wrappers and the VMRUN entry point.
//!
//! Guest GPRs (except RAX which lives in the VMCB save-area) are saved
//! and restored through an `SvmGuestGprs` structure that the caller
//! passes by reference.

#![allow(dead_code)]

use core::arch::global_asm;

// ── MSR numbers ─────────────────────────────────────────────────
pub const MSR_EFER: u32 = 0xC000_0080;
pub const MSR_VM_HSAVE_PA: u32 = 0xC001_0117;

pub const EFER_SVME: u64 = 1 << 12;

// ── Guest GPR save area ─────────────────────────────────────────

/// Guest general-purpose registers that are NOT saved/restored by
/// the VMCB (everything except RAX, RSP, RIP, RFLAGS which are in
/// the VMCB save-area).
///
/// Field order matches the assembly offsets used in `_run_guest`.
#[repr(C)]
pub struct SvmGuestGprs {
    pub rcx: u64,  // offset 0x00
    pub rdx: u64,  // offset 0x08
    pub rbx: u64,  // offset 0x10
    pub rsi: u64,  // offset 0x18
    pub rdi: u64,  // offset 0x20
    pub rbp: u64,  // offset 0x28
    pub r8:  u64,  // offset 0x30
    pub r9:  u64,  // offset 0x38
    pub r10: u64,  // offset 0x40
    pub r11: u64,  // offset 0x48
    pub r12: u64,  // offset 0x50
    pub r13: u64,  // offset 0x58
    pub r14: u64,  // offset 0x60
    pub r15: u64,  // offset 0x68
}

impl SvmGuestGprs {
    pub const fn new() -> Self {
        Self {
            rcx: 0, rdx: 0, rbx: 0, rsi: 0, rdi: 0, rbp: 0,
            r8: 0, r9: 0, r10: 0, r11: 0, r12: 0, r13: 0, r14: 0, r15: 0,
        }
    }
}

// ── Low-level helpers ───────────────────────────────────────────

#[inline]
pub unsafe fn cpuid(func: u32) -> (u32, u32, u32, u32) {
    let eax: u32;
    let ebx: u32;
    let ecx: u32;
    let edx: u32;
    unsafe {
        core::arch::asm!(
            "push rbx",
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
// `_run_guest(guest_vmcb_pa: u64, host_vmcb_pa: u64, gprs: &mut SvmGuestGprs)`
//
// System V AMD64 ABI: RDI = guest_vmcb_pa, RSI = host_vmcb_pa,
//                     RDX = pointer to SvmGuestGprs.
//
// Before VMRUN the wrapper loads guest GPRs from the save area.
// After VMEXIT the wrapper saves guest GPRs back to the save area.
// RAX is handled by the VMCB save-area (hardware).
//
// Stack layout before VMRUN (top = low address = RSP):
//
//   [RSP+ 0]  guest_vmcb_pa   (pushed last)
//   [RSP+ 8]  gprs_ptr        (pushed)
//   [RSP+16]  host_vmcb_pa    (pushed)
//   [RSP+24]  saved r15
//   [RSP+32]  saved r14
//   [RSP+40]  saved r13
//   [RSP+48]  saved r12
//   [RSP+56]  saved rbp
//   [RSP+64]  saved rbx
//   [RSP+72]  return address

global_asm!(
    ".global _run_guest",
    "_run_guest:",
    // ── Save callee-saved host GPRs ──
    "push rbx",
    "push rbp",
    "push r12",
    "push r13",
    "push r14",
    "push r15",

    // Save parameters we need after VMEXIT
    "push rsi",         // [RSP+16] = host_vmcb_pa
    "push rdx",         // [RSP+ 8] = gprs_ptr
    "push rdi",         // [RSP+ 0] = guest_vmcb_pa

    // ── Disable interrupts ──
    "cli",

    // ── VMSAVE host FS/GS/TR/LDTR ──
    "mov rax, rsi",     // RAX = host_vmcb_pa
    "vmsave",

    // ── Load guest GPRs from save area ──
    // RDX still holds the gprs pointer (not yet clobbered)
    "mov rcx, [rdx + 0x00]",
    "mov rbx, [rdx + 0x10]",
    "mov rsi, [rdx + 0x18]",
    "mov rbp, [rdx + 0x28]",
    "mov r8,  [rdx + 0x30]",
    "mov r9,  [rdx + 0x38]",
    "mov r10, [rdx + 0x40]",
    "mov r11, [rdx + 0x48]",
    "mov r12, [rdx + 0x50]",
    "mov r13, [rdx + 0x58]",
    "mov r14, [rdx + 0x60]",
    "mov r15, [rdx + 0x68]",
    "mov rdi, [rdx + 0x20]",    // guest RDI  — clobbers RDI param
    "mov rdx, [rdx + 0x08]",    // guest RDX  — clobbers gprs ptr, LAST!

    // ── Enter guest ──
    // RAX = guest_vmcb_pa from stack
    "mov rax, [rsp]",
    "vmrun",

    // ═══════════════════════════════════════════════════════════
    //  VMEXIT — host RSP restored by hardware.
    //
    //  Stack:  [RSP+0]=guest_vmcb_pa  [RSP+8]=gprs_ptr
    //          [RSP+16]=host_vmcb_pa  [RSP+24..]=saved regs
    //
    //  CPU GPRs (except RAX) hold guest values.
    //  RAX is saved in the VMCB save-area by hardware.
    // ═══════════════════════════════════════════════════════════

    // Get gprs pointer: swap RDI with [RSP+8] (gprs_ptr).
    // This atomically saves guest RDI and loads gprs_ptr.
    "xchg rdi, [rsp + 8]",     // RDI = gprs_ptr, [RSP+8] = guest RDI

    // ── Save guest GPRs to the save area ──
    "mov [rdi + 0x00], rcx",
    "mov [rdi + 0x08], rdx",
    "mov [rdi + 0x10], rbx",
    "mov [rdi + 0x18], rsi",
    // guest RDI is at [RSP+8] (stored by xchg)
    "mov rax, [rsp + 8]",
    "mov [rdi + 0x20], rax",
    "mov [rdi + 0x28], rbp",
    "mov [rdi + 0x30], r8",
    "mov [rdi + 0x38], r9",
    "mov [rdi + 0x40], r10",
    "mov [rdi + 0x48], r11",
    "mov [rdi + 0x50], r12",
    "mov [rdi + 0x58], r13",
    "mov [rdi + 0x60], r14",
    "mov [rdi + 0x68], r15",

    // ── VMLOAD host FS/GS/TR/LDTR ──
    "mov rax, [rsp + 16]",     // RAX = host_vmcb_pa
    "vmload",

    // ── Re-enable interrupts ──
    "sti",

    // ── Clean up stack (pop the 3 saved parameters) ──
    "add rsp, 24",

    // ── Restore callee-saved host GPRs ──
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
    /// * `gprs`          – mutable reference to the guest GPR save area.
    ///   On entry the saved values are loaded into registers; on exit the
    ///   guest register values are written back.
    pub fn _run_guest(guest_vmcb_pa: u64, host_vmcb_pa: u64, gprs: &mut SvmGuestGprs);
}
