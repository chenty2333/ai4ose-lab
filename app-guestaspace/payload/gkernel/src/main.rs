//! Guest kernel payload for arceos-guestaspace hypervisor.
//!
//! - **riscv64**: Full ArceOS app using `axstd` with paging.
//!   Reads PFlash via kernel virtual mapping.
//! - **aarch64**: Bare-metal EL0 program using SVC hypercalls.
//!   Demonstrates nested page fault handling via TTBR0 page tables.
//! - **x86_64**: Bare-metal long-mode program using VMMCALL hypercalls.
//!   Demonstrates nested page fault handling via SVM NPT.

#![no_std]
#![no_main]

// ══════════════════════════════════════════════════════════════
//  RISC-V 64 — Full ArceOS guest with paging
// ══════════════════════════════════════════════════════════════

#[cfg(all(feature = "axstd", target_arch = "riscv64"))]
#[macro_use]
extern crate axstd as std;

#[cfg(all(feature = "axstd", target_arch = "riscv64"))]
use std::os::arceos::modules::axhal::mem::phys_to_virt;

#[cfg(target_arch = "riscv64")]
const PFLASH_START: usize = 0x2200_0000;

#[cfg(all(feature = "axstd", target_arch = "riscv64"))]
#[unsafe(no_mangle)]
fn main() {
    println!("Reading PFlash at physical address {:#X}...", PFLASH_START);
    let va = phys_to_virt(PFLASH_START.into()).as_usize();
    let ptr = va as *const u32;
    unsafe {
        println!(
            "Try to access pflash dev region [{:#X}], got {:#X}",
            va, *ptr
        );
        let magic = (*ptr).to_ne_bytes();
        println!(
            "Got pflash magic: {}",
            core::str::from_utf8(&magic).unwrap()
        );
    }
}

// ══════════════════════════════════════════════════════════════
//  AArch64 — Bare-metal EL0 guest, SVC hypercalls
//
//  Hypercall ABI (SVC #0):
//    x8 = function ID:
//      1 = putchar (x0 = character)
//      2 = exit
// ══════════════════════════════════════════════════════════════

#[cfg(target_arch = "aarch64")]
const PFLASH_START: usize = 0x0400_0000;

#[cfg(target_arch = "aarch64")]
mod aarch64_guest {
    use super::PFLASH_START;

    #[inline(always)]
    fn svc_putchar(c: u8) {
        unsafe {
            core::arch::asm!(
                "svc #0",
                in("x0") c as u64,
                in("x8") 1u64, // putchar
                options(nomem, nostack),
            );
        }
    }

    fn svc_exit() -> ! {
        unsafe {
            core::arch::asm!(
                "svc #0",
                in("x8") 2u64, // exit
                options(noreturn, nomem, nostack),
            );
        }
    }

    fn print_str(s: &str) {
        for &b in s.as_bytes() {
            svc_putchar(b);
        }
    }

    fn print_hex32(val: u32) {
        print_str("0x");
        for i in (0..8).rev() {
            let nibble = ((val >> (i * 4)) & 0xF) as u8;
            let c = if nibble < 10 {
                b'0' + nibble
            } else {
                b'a' + nibble - 10
            };
            svc_putchar(c);
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn _start() -> ! {
        print_str("\n       d8888                            .d88888b.   .d8888b.\n");
        print_str("      d88888                           d88P\" \"Y88b d88P  Y88b\n");
        print_str("     d88P888                           888     888 Y88b.\n");
        print_str("    d88P 888 888d888  .d8888b  .d88b.  888     888  \"Y888b.\n");
        print_str("   d88P  888 888P\"   d88P\"    d8P  Y8b 888     888     \"Y88b.\n");
        print_str("  d88P   888 888     888      88888888 888     888       \"888\n");
        print_str(" d8888888888 888     Y88b.    Y8b.     Y88b. .d88P Y88b  d88P\n");
        print_str("d88P     888 888      \"Y8888P  \"Y8888   \"Y88888P\"   \"Y8888P\"\n\n");
        print_str("arch = aarch64\nplatform = aarch64-qemu-virt\nsmp = 1\n\n");

        print_str("Reading PFlash at physical address 0x04000000...\n");
        print_str("Try to access pflash dev region [0x04000000], got ");

        let val = unsafe { core::ptr::read_volatile(PFLASH_START as *const u32) };
        print_hex32(val);
        print_str("\n");

        let magic = val.to_ne_bytes();
        print_str("Got pflash magic: ");
        if let Ok(s) = core::str::from_utf8(&magic) {
            print_str(s);
        } else {
            print_str("???");
        }
        print_str("\n");

        svc_exit();
    }
}

// ══════════════════════════════════════════════════════════════
//  x86_64 — Bare-metal long-mode guest, VMMCALL hypercalls
//
//  Hypercall ABI (VMMCALL):
//    rax encoding:
//      rax & 0xFF == 1  : putchar (char = (rax >> 8) & 0xFF)
//      rax == 0x84000008: exit (PSCI SYSTEM_OFF convention)
//
//  We encode everything in RAX because AMD SVM only saves RAX
//  in the VMCB; other GPRs are not accessible to the hypervisor
//  without extra assembly scaffolding.
// ══════════════════════════════════════════════════════════════

#[cfg(target_arch = "x86_64")]
const PFLASH_START: usize = 0xFFC0_0000;

#[cfg(target_arch = "x86_64")]
mod x86_64_guest {
    use super::PFLASH_START;

    #[inline(always)]
    fn vmmcall_putchar(c: u8) {
        unsafe {
            core::arch::asm!(
                "vmmcall",
                in("rax") 1u64 | ((c as u64) << 8), // func=1, char in bits [15:8]
                options(nomem, nostack),
            );
        }
    }

    fn vmmcall_exit() -> ! {
        unsafe {
            core::arch::asm!(
                "vmmcall",
                in("rax") 0x84000008u64, // exit
                options(noreturn, nomem, nostack),
            );
        }
    }

    fn print_str(s: &str) {
        for &b in s.as_bytes() {
            vmmcall_putchar(b);
        }
    }

    fn print_hex32(val: u32) {
        print_str("0x");
        for i in (0..8).rev() {
            let nibble = ((val >> (i * 4)) & 0xF) as u8;
            let c = if nibble < 10 {
                b'0' + nibble
            } else {
                b'a' + nibble - 10
            };
            vmmcall_putchar(c);
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn _start() -> ! {
        print_str("\n       d8888                            .d88888b.   .d8888b.\n");
        print_str("      d88888                           d88P\" \"Y88b d88P  Y88b\n");
        print_str("     d88P888                           888     888 Y88b.\n");
        print_str("    d88P 888 888d888  .d8888b  .d88b.  888     888  \"Y888b.\n");
        print_str("   d88P  888 888P\"   d88P\"    d8P  Y8b 888     888     \"Y88b.\n");
        print_str("  d88P   888 888     888      88888888 888     888       \"888\n");
        print_str(" d8888888888 888     Y88b.    Y8b.     Y88b. .d88P Y88b  d88P\n");
        print_str("d88P     888 888      \"Y8888P  \"Y8888   \"Y88888P\"   \"Y8888P\"\n\n");
        print_str("arch = x86_64\nplatform = x86-pc\nsmp = 1\n\n");

        print_str("Reading PFlash at physical address 0xFFC00000...\n");
        print_str("Try to access pflash dev region [0xFFC00000], got ");

        let val = unsafe { core::ptr::read_volatile(PFLASH_START as *const u32) };
        print_hex32(val);
        print_str("\n");

        let magic = val.to_ne_bytes();
        print_str("Got pflash magic: ");
        if let Ok(s) = core::str::from_utf8(&magic) {
            print_str(s);
        } else {
            print_str("???");
        }
        print_str("\n");

        vmmcall_exit();
    }
}

// ══════════════════════════════════════════════════════════════
//  Panic handler for bare-metal targets (aarch64, x86_64)
// ══════════════════════════════════════════════════════════════

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            core::arch::asm!("wfi");
        }
        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
