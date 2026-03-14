#![no_std]
#![no_main]

use core::panic::PanicInfo;

// Minimal guest kernel entry point.
//
// On RISC-V: performs an SBI legacy shutdown (a7 = 8, ecall).
// On AArch64: performs a PSCI SYSTEM_OFF via SVC (x0 = 0x84000008, svc #0).
// On x86_64: performs a VMMCALL with function ID 0x84000008 in EAX.
//            Assembled as 16-bit real-mode code (guest runs in real mode
//            under AMD SVM). Uses global_asm to avoid compiler-generated
//            function prologue (push rax) which would cause a stack access
//            fault in a real-mode guest with no mapped stack.

// x86_64: define _start via global_asm! to avoid function prologue
#[cfg(target_arch = "x86_64")]
core::arch::global_asm!(
    ".code16",
    ".global _start",
    "_start:",
    "mov eax, 0x84000008",   // function ID = PSCI-style SYSTEM_OFF
    "vmmcall",                // AMD SVM hypercall → VMEXIT to hypervisor
    "2: jmp 2b",              // spin if vmmcall returns
    ".code64",
);

// Non-x86_64 architectures: define _start as a Rust function
#[cfg(not(target_arch = "x86_64"))]
#[unsafe(no_mangle)]
unsafe extern "C" fn _start() -> ! {
    #[cfg(target_arch = "riscv64")]
    unsafe {
        core::arch::asm!(
            "li a7, 8",
            "ecall",
            options(noreturn)
        );
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!(
            "movz x0, #0x0008",
            "movk x0, #0x8400, lsl #16",
            "svc #0",
            "b .",
            options(noreturn)
        );
    }

    #[cfg(not(any(target_arch = "riscv64", target_arch = "aarch64")))]
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
