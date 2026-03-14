#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[cfg(feature = "axstd")]
#[macro_use]
extern crate axstd as std;

#[cfg(feature = "axstd")]
use std::os::arceos::modules::axhal::mem::phys_to_virt;

/// PFlash1 physical address on RISC-V 64 QEMU virt machine.
/// pflash0 @ 0x20000000 (32MB), pflash1 @ 0x22000000 (32MB).
#[cfg(all(feature = "axstd", target_arch = "riscv64"))]
const PFLASH_START: usize = 0x2200_0000;

/// PFlash1 physical address on AArch64 QEMU virt machine.
/// pflash0 @ 0x00000000 (64MB), pflash1 @ 0x04000000 (64MB).
#[cfg(all(feature = "axstd", target_arch = "aarch64"))]
const PFLASH_START: usize = 0x0400_0000;

/// PFlash0 physical address on x86_64 QEMU Q35 machine.
/// 4MB flash image mapped at 4GB - 4MB = 0xFFC00000.
#[cfg(all(feature = "axstd", target_arch = "x86_64"))]
const PFLASH_START: usize = 0xFFC0_0000;

/// PFlash1 physical address on LoongArch64 QEMU virt machine.
/// VIRT_FLASH region starts at 0x1d000000. pflash0 is reserved for
/// firmware, so we use pflash1. When pflash0 is absent, pflash1 maps
/// at the base of the flash region: 0x1d000000.
#[cfg(all(feature = "axstd", target_arch = "loongarch64"))]
const PFLASH_START: usize = 0x1d00_0000;

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    #[cfg(all(feature = "axstd", feature = "multitask"))]
    {
        use std::thread;

        println!("Multi-task is starting ...");

        let worker = thread::spawn(move || {
            println!("Spawned-thread is running ...");

            // Access the PFlash MMIO region from the child task.
            // The paging feature ensures MMIO regions (including PFlash) are
            // mapped in the kernel page tables.
            let va = phys_to_virt(PFLASH_START.into()).as_usize();
            let ptr = va as *const u32;
            let magic = unsafe {
                println!("Try to access pflash dev region [{:#X}], got {:#X}", va, *ptr);
                (*ptr).to_ne_bytes()
            };
            if let Ok(s) = core::str::from_utf8(&magic) {
                println!("Got pflash magic: {s}");
                0
            } else {
                -1
            }
        });

        let ret = worker.join();
        // Make sure that worker has finished its work.
        assert_eq!(ret, Ok(0));

        println!("Multi-task OK!");
    }
    #[cfg(not(all(feature = "axstd", feature = "multitask")))]
    {
        println!("This application requires the 'axstd' and 'multitask' features for multi-task and PFlash access.");
        println!("Run with: cargo xtask run [--arch <ARCH>]");
    }
}
