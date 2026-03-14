//! Guest kernel payload for arceos-guestvdev hypervisor.
//!
//! - **riscv64**: Full ArceOS app using `axstd` with multitasking.
//!   Runs a preemptive multi-task demo with CFS scheduler (u_6_0 style).
//!   Requires timer virtualization from the hypervisor.
//! - **aarch64**: Full ArceOS app using `axstd` with multitasking (when axstd enabled).
//!   Runs the same preemptive multi-task demo with CFS scheduler.
//!   The hypervisor uses a bootloader approach (EL1 handoff) — guest has
//!   direct hardware access for timer, UART, and GIC.
//!   Falls back to bare-metal EL0 mode if axstd is not enabled.
//! - **x86_64**: Full ArceOS app using `axstd` with multitasking (when axstd enabled).
//!   Runs the same preemptive multi-task demo with CFS scheduler.
//!   The hypervisor uses SVM with hardware passthrough — guest has direct
//!   access to APIC timer, serial port, and I/O ports via NPT + IOPM.
//!   Falls back to bare-metal long-mode VMMCALL demo if axstd is not enabled.

#![no_std]
#![no_main]

// ══════════════════════════════════════════════════════════════
//  Full ArceOS guest with multitasking (u_6_0 style)
//  Used by: riscv64, aarch64, x86_64 (when axstd feature is enabled)
//
//  This guest exercises virtual device / hardware support:
//  - Timer (for preemptive scheduling via CFS)
//  - Console I/O (println!)
//  - Preemptive scheduling (CFS scheduler with timer interrupts)
// ══════════════════════════════════════════════════════════════

#[cfg(all(feature = "axstd", any(target_arch = "riscv64", target_arch = "aarch64", target_arch = "x86_64")))]
#[macro_use]
extern crate axstd as std;

#[cfg(all(feature = "axstd", any(target_arch = "riscv64", target_arch = "aarch64", target_arch = "x86_64")))]
mod multitask_guest {
    use std::collections::VecDeque;
    use std::os::arceos::modules::axsync::spin::SpinNoIrq;
    use std::sync::Arc;
    use std::thread;

    const LOOP_NUM: usize = 256;

    pub fn run() {
        println!("Multi-task(Preemptible) is starting ...");

        let q1 = Arc::new(SpinNoIrq::new(VecDeque::new()));
        let q2 = q1.clone();

        let worker1 = thread::spawn(move || {
            println!("worker1 ... {:?}", thread::current().id());
            for i in 0..=LOOP_NUM {
                println!("worker1 [{i}]");
                q1.lock().push_back(i);
            }
            println!("worker1 ok!");
        });

        let worker2 = thread::spawn(move || {
            println!("worker2 ... {:?}", thread::current().id());
            loop {
                if let Some(num) = q2.lock().pop_front() {
                    println!("worker2 [{num}]");
                    if num == LOOP_NUM {
                        break;
                    }
                } else {
                    println!("worker2: nothing to do!");
                    // TODO: it should sleep and wait for notify!
                    thread::yield_now();
                }
            }
            println!("worker2 ok!");
        });

        println!("Wait for workers to exit ...");
        let _ = worker1.join();
        let _ = worker2.join();

        println!("Multi-task(Preemptible) ok!");
    }
}

#[cfg(all(feature = "axstd", any(target_arch = "riscv64", target_arch = "aarch64", target_arch = "x86_64")))]
#[unsafe(no_mangle)]
fn main() {
    multitask_guest::run();

    // On AArch64 (bootloader mode), the guest has direct hardware access.
    // Explicitly call PSCI SYSTEM_OFF to cleanly shut down QEMU.
    //
    // With `-machine virt,virtualization=on`, PSCI is handled at EL3 via SMC
    // (the EL2 stub does not forward HVC-based PSCI calls).
    // This matches the shutdown approach used by app-guestmode and app-guestaspace.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!(
            "movz x0, #0x0008",
            "movk x0, #0x8400, lsl #16",   // x0 = 0x84000008 (PSCI_SYSTEM_OFF)
            "smc  #0",
            options(noreturn)
        );
    }

    // On x86_64 (SVM mode), the guest runs inside an AMD SVM container.
    // Use VMMCALL to signal shutdown to the hypervisor.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::asm!(
            "vmmcall",
            in("rax") 0x84000008u64,
            options(noreturn, nomem, nostack),
        );
    }
}

#[cfg(not(feature = "axstd"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[cfg(not(feature = "axstd"))]
#[unsafe(no_mangle)]
extern "C" fn _start() -> ! {
    loop {}
}

// ══════════════════════════════════════════════════════════════
//  AArch64 — Bare-metal EL0 guest (fallback when axstd is NOT enabled)
//
//  Hypercall ABI (SVC #0):
//    x8 = function ID:
//      1 = putchar (x0 = character)
//      2 = exit
//
//  Demonstrates virtual device handling:
//  - Console output via SVC hypercalls (virtual UART)
//  - PFlash read via NPF (virtual pflash device)
// ══════════════════════════════════════════════════════════════

#[cfg(all(not(feature = "axstd"), target_arch = "aarch64"))]
const PFLASH_START: usize = 0x0400_0000;

#[cfg(all(not(feature = "axstd"), target_arch = "aarch64"))]
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

        print_str("Virtual Device (vdev) Test\n");
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
//  (fallback when axstd is NOT enabled)
//
//  Hypercall ABI (VMMCALL):
//    rax encoding:
//      rax & 0xFF == 1  : putchar (char = (rax >> 8) & 0xFF)
//      rax == 0x84000008: exit (PSCI SYSTEM_OFF convention)
//
//  Demonstrates virtual device handling:
//  - Console output via VMMCALL hypercalls (virtual UART)
//  - PFlash read via NPF (virtual pflash device)
// ══════════════════════════════════════════════════════════════

#[cfg(all(not(feature = "axstd"), target_arch = "x86_64"))]
const PFLASH_START: usize = 0xFFC0_0000;

#[cfg(all(not(feature = "axstd"), target_arch = "x86_64"))]
mod x86_64_guest {
    use super::PFLASH_START;

    #[inline(always)]
    fn vmmcall_putchar(c: u8) {
        unsafe {
            core::arch::asm!(
                "vmmcall",
                in("rax") 1u64 | ((c as u64) << 8),
                options(nomem, nostack),
            );
        }
    }

    fn vmmcall_exit() -> ! {
        unsafe {
            core::arch::asm!(
                "vmmcall",
                in("rax") 0x84000008u64,
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

        print_str("Virtual Device (vdev) Test\n");
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
//  Panic handler for bare-metal targets
//  (only needed when axstd is NOT providing one)
// ══════════════════════════════════════════════════════════════

#[cfg(any(
    all(not(feature = "axstd"), target_arch = "aarch64"),
    all(not(feature = "axstd"), target_arch = "x86_64"),
))]
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
