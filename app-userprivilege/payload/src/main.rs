//! Minimal user-space binary that calls SYS_EXIT(0).
//!
//! This is the "payload" loaded by the monolithic kernel into user space.
//! It simply invokes the exit syscall (number 93) with exit code 0.

#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[unsafe(no_mangle)]
unsafe extern "C" fn _start() -> ! {
    unsafe {
        #[cfg(any(target_arch = "riscv64", target_arch = "riscv32"))]
        core::arch::asm!(
            "li a7, 93",
            "ecall",
            options(noreturn)
        );

        #[cfg(target_arch = "aarch64")]
        core::arch::asm!(
            "mov x8, #93",
            "svc #0",
            options(noreturn)
        );

        #[cfg(target_arch = "x86_64")]
        core::arch::asm!(
            "mov rax, 93",
            "syscall",
            options(noreturn)
        );

        #[cfg(target_arch = "loongarch64")]
        core::arch::asm!(
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
