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

/// Number of messages the producer sends.
#[cfg(feature = "axstd")]
const LOOP_NUM: usize = 64;

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    #[cfg(feature = "axstd")]
    {
        use std::collections::VecDeque;
        use std::os::arceos::modules::axsync::spin::SpinNoIrq;
        use std::sync::Arc;
        use std::thread;

        println!("Multi-task message queue is starting ...");

        // First, verify PFlash MMIO access from the main task.
        let va = phys_to_virt(PFLASH_START.into()).as_usize();
        let ptr = va as *const u32;
        unsafe {
            let magic = (*ptr).to_ne_bytes();
            println!(
                "PFlash check: [{:#X}] -> {}",
                va,
                core::str::from_utf8(&magic).unwrap()
            );
        }

        // Shared message queue protected by SpinNoIrq (interrupt-safe spinlock).
        let q1 = Arc::new(SpinNoIrq::new(VecDeque::new()));
        let q2 = q1.clone();

        // Worker1: producer — pushes messages into the queue,
        // then yields to let the consumer run (cooperative scheduling).
        let worker1 = thread::spawn(move || {
            println!("worker1 (producer) ...");
            for i in 0..=LOOP_NUM {
                println!("worker1 [{i}]");
                q1.lock().push_back(i);
                // Cooperative scheduling: explicitly yield the CPU
                // so other tasks get a chance to run.
                thread::yield_now();
            }
            println!("worker1 ok!");
        });

        // Worker2: consumer — pops messages from the queue,
        // yields when the queue is empty (cooperative scheduling).
        let worker2 = thread::spawn(move || {
            println!("worker2 (consumer) ...");
            loop {
                if let Some(num) = q2.lock().pop_front() {
                    println!("worker2 [{num}]");
                    if num == LOOP_NUM {
                        break;
                    }
                } else {
                    println!("worker2: queue empty, yielding ...");
                    // Cooperative scheduling: yield and wait for
                    // the producer to push more data.
                    thread::yield_now();
                }
            }
            println!("worker2 ok!");
        });

        println!("Wait for workers to exit ...");
        let _ = worker1.join();
        let _ = worker2.join();

        println!("Multi-task message queue OK!");
    }
    #[cfg(not(feature = "axstd"))]
    {
        println!("This application requires the 'axstd' feature for multi-task and PFlash access.");
        println!("Run with: cargo xtask run [--arch <ARCH>]");
    }
}
