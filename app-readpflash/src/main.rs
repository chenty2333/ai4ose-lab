#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[cfg(feature = "axstd")]
#[macro_use]
extern crate axstd as std;

#[cfg(feature = "axstd")]
use std::os::arceos::modules::axhal::mem::phys_to_virt;

/// PFlash1 physical address on RISC-V 64 QEMU virt machine.
/// pflash0 @ 0x20000000 (32MB), pflash1 @ 0x22000000 (32MB).
#[cfg(target_arch = "riscv64")]
const PFLASH_START: usize = 0x2200_0000;

/// PFlash1 physical address on AArch64 QEMU virt machine.
/// pflash0 @ 0x00000000 (64MB), pflash1 @ 0x04000000 (64MB).
#[cfg(target_arch = "aarch64")]
const PFLASH_START: usize = 0x0400_0000;

/// PFlash0 physical address on x86_64 QEMU Q35 machine.
/// 4MB flash image mapped at 4GB - 4MB = 0xFFC00000.
#[cfg(target_arch = "x86_64")]
const PFLASH_START: usize = 0xFFC0_0000;

/// PFlash1 physical address on LoongArch64 QEMU virt machine.
/// VIRT_FLASH region starts at 0x1d000000. pflash0 is reserved for
/// firmware, so we use pflash1. When pflash0 is absent, pflash1 maps
/// at the base of the flash region: 0x1d000000.
#[cfg(target_arch = "loongarch64")]
const PFLASH_START: usize = 0x1d00_0000;

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    #[cfg(feature = "axstd")]
    {
        println!("Reading PFlash at physical address {:#X}...", PFLASH_START);

        // Convert physical address to virtual address via linear mapping.
        // The paging feature ensures MMIO regions (including PFlash) are
        // mapped in the kernel page tables.
        let va = phys_to_virt(PFLASH_START.into()).as_usize();
        let ptr = va as *const u32;
        unsafe {
            println!("Try to access pflash dev region [{:#X}], got {:#X}", va, *ptr);
            let magic = (*ptr).to_ne_bytes();
            println!("Got pflash magic: {}", core::str::from_utf8(&magic).unwrap());
        }
    }
    #[cfg(not(feature = "axstd"))]
    {
        println!("This application requires the 'axstd' feature to access PFlash hardware.");
        println!("Run with: cargo xtask run [--arch <ARCH>]");
    }
}
