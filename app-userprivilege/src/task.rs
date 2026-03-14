use axhal::uspace::{ReturnReason, UserContext};
use axmm::AddrSpace;
use axtask::{AxTaskRef, TaskInner};
use memory_addr::VirtAddr;

use crate::syscall;

/// Spawn a user task that enters user space and handles traps.
///
/// The task:
/// 1. Switches to the user address space page table (via scheduler)
/// 2. Creates a UserContext and enters user mode
/// 3. Handles syscalls and other traps
/// 4. Exits when SYS_EXIT is received
pub fn spawn_user_task(uspace: AddrSpace, ustack_top: VirtAddr) -> AxTaskRef {
    let page_table_root = uspace.page_table_root();

    // Create the user context: entry point, stack top, arg0=0
    let entry = crate::APP_ENTRY;
    let sp = ustack_top;

    let mut task = TaskInner::new(
        move || {
            // Keep uspace alive for the duration of this task.
            let _uspace = uspace;

            let mut uctx = UserContext::new(entry, sp, 0);

            ax_println!(
                "Enter user space: entry={:#x}, ustack={:#x}, kstack={:#x}",
                entry,
                sp,
                axtask::current().kernel_stack_top().unwrap(),
            );

            loop {
                let reason = uctx.run();
                match reason {
                    ReturnReason::Syscall => {
                        if let Some(exit_code) = syscall::handle_syscall(&mut uctx) {
                            axtask::exit(exit_code as _);
                        }
                    }
                    ReturnReason::PageFault(vaddr, flags) => {
                        ax_println!(
                            "User page fault at {:#x}, flags: {:?}",
                            vaddr, flags
                        );
                        axtask::exit(-1);
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
