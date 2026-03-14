/*
 * AArch64 hypervisor guest entry/exit  (EL1 → EL0 mode)
 *
 * Because ArceOS drops from EL2 to EL1 during boot, the hypervisor runs at
 * EL1 and the guest runs at EL0 (user mode).  The caller sets TTBR0_EL1 to
 * the guest's page table before calling _run_guest and restores it after.
 *
 * Entry: _run_guest(state: *mut VmCpuRegisters)
 *   x0 = pointer to VmCpuRegisters
 * Returns when a VM exit occurs (SVC, IRQ, etc. from EL0).
 */

.section .text
.global _run_guest
_run_guest:
    /* ===== Save host DAIF and mask all interrupts ===== */
    mrs  x1, daif
    str  x1, [x0, #{host_daif}]
    msr  daifset, #0xf

    /* ===== Save host callee-saved registers ===== */
    stp x19, x20, [x0, #{host_x19}]
    stp x21, x22, [x0, #{host_x21}]
    stp x23, x24, [x0, #{host_x23}]
    stp x25, x26, [x0, #{host_x25}]
    stp x27, x28, [x0, #{host_x27}]
    stp x29, x30, [x0, #{host_x29}]
    mov  x1, sp
    str  x1, [x0, #{host_sp}]

    /* ===== Save original VBAR_EL1 ===== */
    mrs  x1, vbar_el1
    str  x1, [x0, #{host_vbar}]

    /* ===== Install guest exception vectors ===== */
    adr  x1, _guest_vectors
    msr  vbar_el1, x1
    isb

    /* ===== Save VmCpuRegisters pointer on stack ===== */
    sub  sp, sp, #16
    str  x0, [sp]

    /* ===== Load guest system state ===== */
    ldr  x1, [x0, #{guest_elr}]
    msr  elr_el1, x1
    ldr  x1, [x0, #{guest_spsr}]
    msr  spsr_el1, x1
    ldr  x1, [x0, #{guest_sp}]
    msr  sp_el0, x1

    /* ===== Load guest GPRs ===== */
    ldp  x2,  x3, [x0, #{guest_x2}]
    ldp  x4,  x5, [x0, #{guest_x4}]
    ldp  x6,  x7, [x0, #{guest_x6}]
    ldp  x8,  x9, [x0, #{guest_x8}]
    ldp x10, x11, [x0, #{guest_x10}]
    ldp x12, x13, [x0, #{guest_x12}]
    ldp x14, x15, [x0, #{guest_x14}]
    ldp x16, x17, [x0, #{guest_x16}]
    ldp x18, x19, [x0, #{guest_x18}]
    ldp x20, x21, [x0, #{guest_x20}]
    ldp x22, x23, [x0, #{guest_x22}]
    ldp x24, x25, [x0, #{guest_x24}]
    ldp x26, x27, [x0, #{guest_x26}]
    ldp x28, x29, [x0, #{guest_x28}]
    ldr x30, [x0, #{guest_x30}]
    /* Load x1 before x0 (x0 is the base pointer) */
    ldr  x1, [x0, #{guest_x1}]
    ldr  x0, [x0, #{guest_x0}]

    /* ===== Enter guest at EL0 ===== */
    eret

/*
 * AArch64 Exception Vector Table for EL1
 *
 * Must be 2048-byte (0x800) aligned.
 * 16 entries, each 128 bytes (32 instructions max).
 *
 * We only care about "Lower EL using AArch64" entries (guest at EL0).
 * All "Current EL" entries go to _unhandled (should never hit since
 * interrupts are masked while VBAR points here).
 */
.balign 0x800
_guest_vectors:
    /* Current EL with SP_EL0 — Synchronous */
    b _unhandled_exception
    .balign 0x80
    /* Current EL with SP_EL0 — IRQ */
    b _unhandled_exception
    .balign 0x80
    /* Current EL with SP_EL0 — FIQ */
    b _unhandled_exception
    .balign 0x80
    /* Current EL with SP_EL0 — SError */
    b _unhandled_exception
    .balign 0x80

    /* Current EL with SP_ELx — Synchronous */
    b _unhandled_exception
    .balign 0x80
    /* Current EL with SP_ELx — IRQ */
    b _unhandled_exception
    .balign 0x80
    /* Current EL with SP_ELx — FIQ */
    b _unhandled_exception
    .balign 0x80
    /* Current EL with SP_ELx — SError */
    b _unhandled_exception
    .balign 0x80

    /* Lower EL using AArch64 — Synchronous (guest SVC traps here) */
    b _guest_exit
    .balign 0x80
    /* Lower EL using AArch64 — IRQ */
    b _guest_exit
    .balign 0x80
    /* Lower EL using AArch64 — FIQ */
    b _guest_exit
    .balign 0x80
    /* Lower EL using AArch64 — SError */
    b _guest_exit
    .balign 0x80

    /* Lower EL using AArch32 — Synchronous */
    b _unhandled_exception
    .balign 0x80
    /* Lower EL using AArch32 — IRQ */
    b _unhandled_exception
    .balign 0x80
    /* Lower EL using AArch32 — FIQ */
    b _unhandled_exception
    .balign 0x80
    /* Lower EL using AArch32 — SError */
    b _unhandled_exception
    .balign 0x80

_unhandled_exception:
    b _unhandled_exception      /* Spin forever on unhandled */

/*
 * Guest exit handler.
 *
 * On entry we are back at EL1 (from EL0 trap).
 * SP = SP_EL1 (the kernel stack, same as before eret).
 * x0-x30 still hold guest register values.
 */
_guest_exit:
    /* Step 1: Save guest x0, x1 onto the EL1 stack */
    stp x0, x1, [sp, #-16]!

    /* Step 2: Load VmCpuRegisters pointer (saved before eret) */
    ldr x0, [sp, #16]

    /* Step 3: Save guest x2-x30 (these still hold guest values) */
    stp  x2,  x3, [x0, #{guest_x2}]
    stp  x4,  x5, [x0, #{guest_x4}]
    stp  x6,  x7, [x0, #{guest_x6}]
    stp  x8,  x9, [x0, #{guest_x8}]
    stp x10, x11, [x0, #{guest_x10}]
    stp x12, x13, [x0, #{guest_x12}]
    stp x14, x15, [x0, #{guest_x14}]
    stp x16, x17, [x0, #{guest_x16}]
    stp x18, x19, [x0, #{guest_x18}]
    stp x20, x21, [x0, #{guest_x20}]
    stp x22, x23, [x0, #{guest_x22}]
    stp x24, x25, [x0, #{guest_x24}]
    stp x26, x27, [x0, #{guest_x26}]
    stp x28, x29, [x0, #{guest_x28}]
    str x30, [x0, #{guest_x30}]

    /* Step 4: Recover guest x0, x1 from stack and save */
    ldp x1, x2, [sp], #16          /* x1 = guest_x0, x2 = guest_x1 */
    str x1, [x0, #{guest_x0}]
    str x2, [x0, #{guest_x1}]

    /* Step 5: Save guest system registers */
    mrs x1, elr_el1
    str x1, [x0, #{guest_elr}]
    mrs x1, spsr_el1
    str x1, [x0, #{guest_spsr}]
    mrs x1, sp_el0
    str x1, [x0, #{guest_sp}]

    /* Step 6: Save trap info */
    mrs x1, esr_el1
    str x1, [x0, #{trap_esr}]
    mrs x1, far_el1
    str x1, [x0, #{trap_far}]

    /* Step 7: Pop VmCpuRegisters pointer from stack */
    add sp, sp, #16

    /* Step 8: Restore original VBAR_EL1 */
    ldr x1, [x0, #{host_vbar}]
    msr vbar_el1, x1
    isb

    /* Step 9: Restore host callee-saved registers */
    ldp x19, x20, [x0, #{host_x19}]
    ldp x21, x22, [x0, #{host_x21}]
    ldp x23, x24, [x0, #{host_x23}]
    ldp x25, x26, [x0, #{host_x25}]
    ldp x27, x28, [x0, #{host_x27}]
    ldp x29, x30, [x0, #{host_x29}]
    ldr x1, [x0, #{host_sp}]
    mov sp, x1

    /* Step 10: Restore host DAIF */
    ldr x1, [x0, #{host_daif}]
    msr daif, x1

    /* Step 11: Return to _run_guest caller */
    ret
