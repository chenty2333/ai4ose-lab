//! User task spawning and trap dispatch loop.
//!
//! Runs the user-space ELF binary in a loop, dispatching syscalls to
//! the handler in `syscall.rs`.

use alloc::sync::Arc;
use axhal::uspace::{ReturnReason, UserContext};
use axmm::AddrSpace;
use axsync::Mutex;
use axtask::{AxTaskRef, TaskInner};
use memory_addr::VirtAddr;

use crate::syscall;

/// Wrapper to ensure `UserContext` is 16-byte aligned on the stack.
///
/// On x86_64, the CPU aligns RSP to 16 bytes when delivering interrupts
/// from user mode (ring 3 → ring 0).  `TSS.RSP0` is set to
/// `&uctx + sizeof(TrapFrame)`.  If that address is not 16-byte aligned,
/// the CPU adjusts RSP, causing all pushed values to be offset by 8 bytes
/// and the kernel-RSP restore to read the wrong value (triple fault).
///
/// `sizeof(TrapFrame) == 176 == 11 × 16`, so TSS.RSP0 inherits the
/// alignment of `&uctx`.  Forcing 16-byte alignment here guarantees
/// TSS.RSP0 is also 16-byte aligned on every architecture.
#[repr(C, align(16))]
struct AlignedUserContext(UserContext);

/// Spawn a user task that enters user space and handles traps.
///
/// The task runs a loop: enter user mode → receive trap → dispatch
/// (syscall or page fault) → re-enter user mode.
pub fn spawn_user_task(
    uspace: Arc<Mutex<AddrSpace>>,
    entry: usize,
    ustack_pointer: VirtAddr,
) -> AxTaskRef {
    let page_table_root = uspace.lock().page_table_root();
    let task_uspace = uspace.clone();

    let mut task = TaskInner::new(
        move || {
            // Keep uspace alive for the lifetime of this task.
            let _uspace = task_uspace;

            let mut aligned_uctx = AlignedUserContext(UserContext::new(entry, ustack_pointer, 0));

            // Enable FP/SIMD access from user mode (EL0) on aarch64.
            // Musl-compiled binaries use FP instructions even for simple programs.
            #[cfg(target_arch = "aarch64")]
            axhal::asm::enable_fp();

            ax_println!(
                "Enter user space: entry={:#x}, ustack={:#x}, kstack={:#x}",
                entry,
                ustack_pointer,
                axtask::current().kernel_stack_top().unwrap(),
            );

            loop {
                let reason = aligned_uctx.0.run();
                match reason {
                    ReturnReason::Syscall => {
                        if let Some(exit_code) = syscall::handle_syscall(&mut aligned_uctx.0) {
                            axtask::exit(exit_code as _);
                        }
                    }
                    ReturnReason::Interrupt => {
                        // Timer or other interrupt from user mode —
                        // just re-enter user space.
                    }
                    _ => {
                        ax_println!("Unexpected trap from user space: {:?}", reason);
                        axtask::exit(-1);
                    }
                }
            }
        },
        "userboot".into(),
        crate::KERNEL_STACK_SIZE,
    );

    // Set the page table root so the scheduler switches to user space
    // page table when this task is scheduled.
    task.ctx_mut().set_page_table_root(page_table_root);

    axtask::spawn_task(task)
}
