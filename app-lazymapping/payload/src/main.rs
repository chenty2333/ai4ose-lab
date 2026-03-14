//! Minimal user-space binary that touches the stack (triggering a page fault
//! for lazy mapping) and then calls SYS_EXIT(0).
//!
//! This is the "payload" loaded by the monolithic kernel into user space.
//! It first writes to the stack to trigger demand paging, then invokes the
//! exit syscall (number 93) with exit code 0.

#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[unsafe(no_mangle)]
unsafe extern "C" fn _start() -> ! {
    unsafe {
        // Each architecture first touches the stack (triggering a page fault
        // when pages are lazily mapped), then issues SYS_EXIT(0).
        #[cfg(any(target_arch = "riscv64", target_arch = "riscv32"))]
        core::arch::asm!(
            "addi sp, sp, -4",
            "sw a0, (sp)",
            "li a7, 93",
            "ecall",
            options(noreturn)
        );

        #[cfg(target_arch = "aarch64")]
        core::arch::asm!(
            "sub sp, sp, #16",
            "str x0, [sp]",
            "mov x8, #93",
            "svc #0",
            options(noreturn)
        );

        #[cfg(target_arch = "x86_64")]
        core::arch::asm!(
            "push rax",
            "mov rax, 93",
            "syscall",
            options(noreturn)
        );

        #[cfg(target_arch = "loongarch64")]
        core::arch::asm!(
            "addi.d $sp, $sp, -8",
            "st.d $a0, $sp, 0",
            "ori $a7, $zero, 93",
            "syscall 0",
            options(noreturn)
        );
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
