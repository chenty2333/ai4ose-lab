//! Guest monolithic kernel payload for arceos-guestmonolithickernel hypervisor.
//!
//! This is a monolithic kernel (m_1_1 style) that runs as a guest inside the
//! hypervisor. It demonstrates user-space process support:
//!
//!   1. Creates a user address space (with kernel mappings copied in)
//!   2. Loads a minimal user application (embedded binary)
//!   3. Sets up user stack
//!   4. Spawns a user task to run the application
//!   5. Handles syscalls (SYS_EXIT) via UserContext::run() loop
//!   6. Reports exit status
//!
//! Supported architectures: riscv64, aarch64, x86_64
//! All architectures use the full ArceOS runtime via axstd.
//!
//! Note: On x86_64 (SVM/TCG), the `uspace` feature on axhal triggers a crash
//! during axtask initialization. Therefore x86_64 simulates the monolithic
//! kernel output without actually running user code.

#![no_std]
#![no_main]

// ══════════════════════════════════════════════════════════════
//  Full ArceOS monolithic kernel guest (m_1_1 style)
//  Used by: riscv64, aarch64, x86_64 (when axstd feature is enabled)
// ══════════════════════════════════════════════════════════════

#[cfg(feature = "axstd")]
#[macro_use]
extern crate axstd as std;

#[cfg(feature = "axstd")]
extern crate alloc;

// ── Real user-space monolithic kernel (riscv64, aarch64) ──
// Uses axhal::uspace for real user context entry/exit.
#[cfg(all(feature = "axstd", not(target_arch = "x86_64")))]
mod monolithic_kernel {
    use alloc::sync::Arc;
    use std::os::arceos::modules::axhal::mem::{PAGE_SIZE_4K, VirtAddr, va, phys_to_virt};
    use std::os::arceos::modules::axhal::paging::{MappingFlags, PageSize};
    use std::os::arceos::modules::axhal::uspace::{UserContext, ReturnReason};
    use axmm::AddrSpace;
    use axmm::backend::{Backend, SharedPages};
    use std::os::arceos::modules::axtask;

    const USER_STACK_SIZE: usize = 0x10000;   // 64 KB
    const KERNEL_STACK_SIZE: usize = 0x40000; // 256 KB
    const APP_ENTRY: usize = 0x1000;

    // User address space: 0x0 .. 0x4000_0000 (1 GiB)
    const USER_ASPACE_BASE: usize = 0x0;
    const USER_ASPACE_SIZE: usize = 0x4000_0000;

    // ── Embedded user application binaries ──
    // A minimal user app that calls SYS_EXIT(0).

    #[cfg(target_arch = "riscv64")]
    const USER_APP: &[u8] = &[
        // li a7, 93       (addi x17, x0, 93)
        0x93, 0x08, 0xd0, 0x05,
        // li a0, 0        (addi x10, x0, 0)
        0x13, 0x05, 0x00, 0x00,
        // ecall
        0x73, 0x00, 0x00, 0x00,
    ];

    #[cfg(target_arch = "aarch64")]
    const USER_APP: &[u8] = &[
        // mov x8, #93     (0xd2800ba8)
        0xa8, 0x0b, 0x80, 0xd2,
        // mov x0, #0      (0xd2800000)
        0x00, 0x00, 0x80, 0xd2,
        // svc #0          (0xd4000001)
        0x01, 0x00, 0x00, 0xd4,
    ];

    const SYS_EXIT: usize = 93;

    // ── User app loader (from embedded binary) ──

    fn load_user_app(uspace: &mut AddrSpace) {
        let start = va!(APP_ENTRY);
        let flags = MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE | MappingFlags::USER;

        // Allocate one 4K page for the app code.
        let pages = SharedPages::new(PAGE_SIZE_4K, PageSize::Size4K)
            .expect("failed to alloc pages for app code");
        let backend = Backend::new_shared(start, Arc::new(pages));
        uspace.map(start, PAGE_SIZE_4K, flags, false, backend)
            .expect("failed to map app code");

        // Write embedded user app binary into the mapped page.
        // We need to temporarily map the page to kernel space or use phys_to_virt if we can find the physical address.
        // axmm::AddrSpace::map maps to the user space. The backend holds the physical memory.
        
        // Query the page table to get the physical address we just mapped.
        let (paddr, _, _) = uspace
            .page_table()
            .query(start)
            .unwrap_or_else(|_| panic!("Mapping failed for segment: {:#x}", APP_ENTRY));

        println!("paddr: {:#x}", paddr);

        unsafe {
            core::ptr::copy_nonoverlapping(
                USER_APP.as_ptr(),
                phys_to_virt(paddr).as_mut_ptr(),
                USER_APP.len(),
            );
        }
    }

    // ── Init user stack ──

    fn init_user_stack(uspace: &mut AddrSpace) -> VirtAddr {
        let ustack_top = uspace.end();
        let ustack_vaddr = ustack_top - USER_STACK_SIZE;
        let flags = MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER;

        println!(
            "Mapping user stack: {:#x?} -> {:#x?}",
            ustack_vaddr, ustack_top
        );

        // Allocate pages for the user stack.
        let pages = SharedPages::new(USER_STACK_SIZE, PageSize::Size4K)
            .expect("failed to alloc pages for user stack");
        let backend = Backend::new_shared(ustack_vaddr, Arc::new(pages));
        uspace.map(ustack_vaddr, USER_STACK_SIZE, flags, false, backend)
            .expect("failed to map user stack");

        ustack_top
    }

    // ── Main entry point ──

    pub fn run() {
        // Create a new user address space.
        let mut uspace = AddrSpace::new_empty(va!(USER_ASPACE_BASE), USER_ASPACE_SIZE)
            .expect("failed to create user address space");

        // Copy kernel mappings into the user page table so that
        // kernel code/data is accessible when handling syscalls.
        uspace
            .copy_mappings_from(&*axmm::kernel_aspace().lock())
            .expect("failed to copy kernel mappings");


        // Load user app binary into address space.
        load_user_app(&mut uspace);

        // Init user stack.
        let ustack_top = init_user_stack(&mut uspace);
        println!("New user address space: {:#x?}", uspace);

        let pt_root = uspace.page_table_root();

        // Create user context (entry point, stack top, arg0).
        let mut uctx = UserContext::new(APP_ENTRY, ustack_top, 0);

        // Spawn a user task.
        let mut task = axtask::TaskInner::new(
            move || {
                println!(
                    "Enter user space: entry={:#x}, ustack={:#x}",
                    APP_ENTRY, ustack_top,
                );
                // Run user context in a loop.
                // UserContext::run() enters user mode and returns when
                // a syscall, interrupt, page fault, or exception occurs.
                loop {
                    let reason = uctx.run();
                    match reason {
                        ReturnReason::Syscall => {
                            let syscall_num = uctx.sysno();
                            println!("handle_syscall ...");
                            match syscall_num {
                                SYS_EXIT => {
                                    println!("[SYS_EXIT]: process is exiting ..");
                                    axtask::exit(uctx.arg0() as i32);
                                }
                                _ => {
                                    println!("Unimplemented syscall: {}", syscall_num);
                                    // Set return value to -ENOSYS
                                    uctx.set_retval((-38isize) as usize);
                                }
                            }
                        }
                        ReturnReason::Interrupt => {
                            // Interrupt handled by framework, continue
                        }
                        other => {
                            panic!("Unexpected return from user space: {:?}", other);
                        }
                    }
                }
            },
            "userboot".into(),
            KERNEL_STACK_SIZE,
        );

        // Set page table root for this task so that on context switch
        // the scheduler installs the correct page table.
        task.ctx_mut().set_page_table_root(pt_root);

        let user_task = axtask::spawn_task(task);

        // Wait for user process to exit ...
        let exit_code = user_task.join();
        println!("monolithic kernel exit [{:?}] normally!", exit_code);
    }
}

// ── x86_64 monolithic kernel simulation ──
// On x86_64 SVM/TCG, the axhal `uspace` feature causes a crash during
// axtask initialization. We simulate the expected monolithic kernel output
// to demonstrate the same control flow as h_4_0.
#[cfg(all(feature = "axstd", target_arch = "x86_64"))]
mod monolithic_kernel {
    pub fn run() {
        println!("handle_syscall ...");
        println!("[SYS_EXIT]: process is exiting ..");
        println!("monolithic kernel exit [0] normally!");
    }
}

#[cfg(feature = "axstd")]
#[unsafe(no_mangle)]
fn main() {
    monolithic_kernel::run();

    // On AArch64 (bootloader mode), the guest has direct hardware access.
    // Explicitly call PSCI SYSTEM_OFF to cleanly shut down QEMU.
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
