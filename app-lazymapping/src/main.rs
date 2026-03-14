#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[cfg(feature = "axstd")]
extern crate axstd as std;

#[cfg(feature = "axstd")]
extern crate alloc;

#[cfg(feature = "axstd")]
#[macro_use]
extern crate axlog;

#[cfg(feature = "axstd")]
extern crate axfs;
#[cfg(feature = "axstd")]
extern crate axio;

#[cfg(feature = "axstd")]
mod loader;
#[cfg(feature = "axstd")]
mod syscall;
#[cfg(feature = "axstd")]
mod task;

#[cfg(feature = "axstd")]
const USER_STACK_SIZE: usize = 0x10000;
#[cfg(feature = "axstd")]
const KERNEL_STACK_SIZE: usize = 0x40000; // 256 KiB
#[cfg(feature = "axstd")]
const APP_ENTRY: usize = 0x1000;

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    #[cfg(feature = "axstd")]
    {
        use alloc::sync::Arc;
        use axhal::paging::{MappingFlags, PageSize};
        use axmm::backend::SharedPages;
        use memory_addr::va;

        // A new address space for user app (equivalent to axmm::new_user_aspace()).
        // User space: [0x0, 0x40_0000_0000) — 256GB, below kernel space.
        let mut uspace = axmm::AddrSpace::new_empty(va!(0x0), 0x40_0000_0000).unwrap();

        // Copy kernel page table entries so kernel code is accessible in user tasks.
        uspace
            .copy_mappings_from(&axmm::kernel_aspace().lock())
            .unwrap();

        // Load user app binary file into address space.
        if let Err(e) = loader::load_user_app("/sbin/origin", &mut uspace) {
            panic!("Cannot load app! {:?}", e);
        }

        // Init user stack with LAZY mapping:
        // 1. Pre-allocate physical pages via SharedPages
        // 2. Map the area with SharedBackend (which eagerly maps page table entries)
        // 3. Unmap all page table entries to enable demand paging
        // When user touches the stack, a page fault occurs, and the handler
        // re-maps the page from the pre-allocated pool.
        let ustack_top = uspace.end();
        let ustack_vaddr = ustack_top - USER_STACK_SIZE;
        ax_println!(
            "Mapping user stack: {:#x?} -> {:#x?}",
            ustack_vaddr,
            ustack_top
        );

        let stack_pages = Arc::new(
            SharedPages::new(USER_STACK_SIZE, PageSize::Size4K).unwrap(),
        );
        let stack_pages_for_fault = stack_pages.clone();

        uspace
            .map(
                ustack_vaddr,
                USER_STACK_SIZE,
                MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
                true,
                axmm::backend::Backend::new_shared(ustack_vaddr, stack_pages),
            )
            .unwrap();

        // Unmap all stack page table entries for lazy/demand paging.
        // The area remains registered so the address range is reserved.
        // NOTE: The actual unmap is done inside the task closure (in task.rs)
        // because on x86_64, the TLB flush must happen while the user page
        // table is active (after task scheduling).

        ax_println!("New user address space: {:#x?}", uspace);

        // Let's kick off the user process.
        let user_task = task::spawn_user_task(
            uspace,
            ustack_top,
            ustack_vaddr,
            stack_pages_for_fault,
        );

        // Wait for user process to exit ...
        let exit_code = user_task.join();
        ax_println!("monolithic kernel exit [{:?}] normally!", exit_code);
    }
    #[cfg(not(feature = "axstd"))]
    {
        println!("This application requires the 'axstd' feature for lazy mapping execution.");
        println!("Run with: cargo xtask run [--arch <ARCH>]");
    }
}
