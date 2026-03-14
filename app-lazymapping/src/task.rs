use alloc::sync::Arc;
use axhal::paging::{MappingFlags, PageSize};
use axhal::uspace::{ReturnReason, UserContext};
use axmm::AddrSpace;
use axmm::backend::SharedPages;
use axtask::{AxTaskRef, TaskInner};
use memory_addr::{MemoryAddr, VirtAddr};

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
/// This task implements **lazy (demand) paging** for the user stack:
/// - The stack area is registered in the address space, but page table
///   entries are removed before entering user space.
/// - When the user first touches the stack, a page fault occurs.
/// - The handler looks up the pre-allocated physical page from `stack_pages`
///   and maps it into the page table, then resumes execution.
pub fn spawn_user_task(
    mut uspace: AddrSpace,
    ustack_top: VirtAddr,
    ustack_vaddr: VirtAddr,
    stack_pages: Arc<SharedPages>,
) -> AxTaskRef {
    let page_table_root = uspace.page_table_root();

    // Create the user context: entry point, stack top, arg0=0
    let entry = crate::APP_ENTRY;
    let sp = ustack_top;

    let mut task = TaskInner::new(
        move || {
            // Unmap ALL stack page table entries for lazy/demand paging.
            // The pages remain pre-allocated in `stack_pages` and will be
            // mapped on first access via the page fault handler below.
            {
                let mut ptmod = uspace.page_table_mut().modify();
                let n_pages = (ustack_top - ustack_vaddr) / axhal::mem::PAGE_SIZE_4K;
                for i in 0..n_pages {
                    let va = ustack_vaddr + i * axhal::mem::PAGE_SIZE_4K;
                    let _ = ptmod.unmap(va);
                }
            }

            let mut aligned_uctx = AlignedUserContext(UserContext::new(entry, sp, 0));

            ax_println!(
                "Enter user space: entry={:#x}, ustack={:#x}, kstack={:#x}",
                entry,
                sp,
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
                    ReturnReason::PageFault(vaddr, _flags) => {
                        // Check if the faulting address is in the user stack range.
                        if vaddr >= ustack_vaddr && vaddr < ustack_top {
                            let page_size: usize = PageSize::Size4K.into();
                            let aligned_va = vaddr.align_down_4k();
                            let page_idx =
                                (aligned_va.as_usize() - ustack_vaddr.as_usize()) / page_size;
                            let paddr = stack_pages.phys_pages[page_idx];

                            // Map the pre-allocated physical page into the page table.
                            uspace
                                .page_table_mut()
                                .modify()
                                .map(
                                    aligned_va,
                                    paddr,
                                    PageSize::Size4K,
                                    MappingFlags::READ
                                        | MappingFlags::WRITE
                                        | MappingFlags::USER,
                                )
                                .unwrap();

                            ax_println!("handle page fault OK!");
                        } else {
                            ax_println!(
                                "{}: segmentation fault at {:#x}, exit!",
                                axtask::current().id_name(),
                                vaddr
                            );
                            axtask::exit(-1);
                        }
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
