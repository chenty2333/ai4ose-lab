use core::arch::global_asm;
use core::mem::size_of;

use memoffset::offset_of;
use super::regs::GeneralPurposeRegisters;

/// Host (EL1 hypervisor) state saved/restored when entering/exiting the guest.
#[repr(C)]
pub struct HostState {
    /// Callee-saved registers x19-x30 (12 registers).
    pub regs: [u64; 12],
    /// Host stack pointer (SP_EL1).
    pub sp: u64,
    /// Saved VBAR_EL1 (restored after guest exit).
    pub vbar: u64,
    /// Saved DAIF (interrupt mask state).
    pub daif: u64,
}

impl Default for HostState {
    fn default() -> Self {
        Self {
            regs: [0u64; 12],
            sp: 0,
            vbar: 0,
            daif: 0,
        }
    }
}

/// Guest (EL0) state.
#[repr(C)]
pub struct GuestState {
    /// General-purpose registers x0-x30.
    pub gprs: GeneralPurposeRegisters,
    /// Guest stack pointer (SP_EL0).
    pub sp: u64,
    /// Guest program counter (ELR_EL1).
    pub elr: u64,
    /// Guest saved program status (SPSR_EL1).
    pub spsr: u64,
}

impl Default for GuestState {
    fn default() -> Self {
        Self {
            gprs: GeneralPurposeRegisters::default(),
            sp: 0,
            elr: 0,
            spsr: 0,
        }
    }
}

/// Trap state read on VM exit.
#[derive(Default, Clone)]
#[repr(C)]
pub struct TrapState {
    /// Exception Syndrome Register (ESR_EL1).
    pub esr: u64,
    /// Fault Address Register (FAR_EL1).
    pub far: u64,
    /// Non-zero if the exit was caused by an IRQ/FIQ/SError (not a synchronous exception).
    /// Synchronous exceptions (SVC, data abort) set this to 0.
    pub is_irq: u64,
}

/// Complete vCPU register state for guest entry/exit.
#[repr(C)]
pub struct VmCpuRegisters {
    /// Host hypervisor state (saved on entry, restored on exit).
    pub host: HostState,
    /// Guest state (restored on entry, saved on exit).
    pub guest: GuestState,
    /// Trap info (written on exit).
    pub trap: TrapState,
}

impl Default for VmCpuRegisters {
    fn default() -> Self {
        Self {
            host: HostState::default(),
            guest: GuestState::default(),
            trap: TrapState::default(),
        }
    }
}

// --- Offset computation for assembly ---

const fn host_reg_offset(index: usize) -> usize {
    offset_of!(VmCpuRegisters, host)
        + offset_of!(HostState, regs)
        + index * size_of::<u64>()
}

const fn guest_gpr_offset(index: usize) -> usize {
    offset_of!(VmCpuRegisters, guest)
        + offset_of!(GuestState, gprs)
        + index * size_of::<u64>()
}

macro_rules! host_field_offset {
    ($field:tt) => {
        offset_of!(VmCpuRegisters, host) + offset_of!(HostState, $field)
    };
}

macro_rules! guest_field_offset {
    ($field:tt) => {
        offset_of!(VmCpuRegisters, guest) + offset_of!(GuestState, $field)
    };
}

macro_rules! trap_field_offset {
    ($field:tt) => {
        offset_of!(VmCpuRegisters, trap) + offset_of!(TrapState, $field)
    };
}

global_asm!(
    include_str!("guest.S"),

    // Host callee-saved registers – only the first of each stp/ldp pair is
    // referenced; the second register is implicitly at (first + 8).
    host_x19  = const host_reg_offset(0),   // stp x19,x20
    host_x21  = const host_reg_offset(2),   // stp x21,x22
    host_x23  = const host_reg_offset(4),   // stp x23,x24
    host_x25  = const host_reg_offset(6),   // stp x25,x26
    host_x27  = const host_reg_offset(8),   // stp x27,x28
    host_x29  = const host_reg_offset(10),  // stp x29,x30
    host_sp   = const host_field_offset!(sp),
    host_vbar = const host_field_offset!(vbar),
    host_daif = const host_field_offset!(daif),

    // Guest GPRs
    guest_x0  = const guest_gpr_offset(0),
    guest_x1  = const guest_gpr_offset(1),
    guest_x2  = const guest_gpr_offset(2),   // ldp x2,x3
    guest_x4  = const guest_gpr_offset(4),
    guest_x6  = const guest_gpr_offset(6),
    guest_x8  = const guest_gpr_offset(8),
    guest_x10 = const guest_gpr_offset(10),
    guest_x12 = const guest_gpr_offset(12),
    guest_x14 = const guest_gpr_offset(14),
    guest_x16 = const guest_gpr_offset(16),
    guest_x18 = const guest_gpr_offset(18),
    guest_x20 = const guest_gpr_offset(20),
    guest_x22 = const guest_gpr_offset(22),
    guest_x24 = const guest_gpr_offset(24),
    guest_x26 = const guest_gpr_offset(26),
    guest_x28 = const guest_gpr_offset(28),
    guest_x30 = const guest_gpr_offset(30),

    // Guest system registers
    guest_sp   = const guest_field_offset!(sp),
    guest_elr  = const guest_field_offset!(elr),
    guest_spsr = const guest_field_offset!(spsr),

    // Trap state
    trap_esr    = const trap_field_offset!(esr),
    trap_far    = const trap_field_offset!(far),
    trap_is_irq = const trap_field_offset!(is_irq),
);

unsafe extern "C" {
    pub fn _run_guest(state: *mut VmCpuRegisters);
}
