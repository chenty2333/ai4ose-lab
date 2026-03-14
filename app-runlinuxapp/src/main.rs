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

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    #[cfg(feature = "axstd")]
    {
        use alloc::string::String;
        use alloc::sync::Arc;
        use axhal::paging::{MappingFlags, PageSize};
        use axmm::backend::{Backend, SharedPages};
        use memory_addr::va;

        // A new address space for user app.
        // User space: [0x0, 0x40_0000_0000) — 256GB, below kernel space.
        let mut uspace = axmm::AddrSpace::new_empty(va!(0x0), 0x40_0000_0000).unwrap();

        // Copy kernel page table entries so kernel code is accessible in user tasks.
        uspace
            .copy_mappings_from(&axmm::kernel_aspace().lock())
            .unwrap();

        // Load user app ELF binary file into address space.
        let entry = match loader::load_user_app("/sbin/hello", &mut uspace) {
            Ok(e) => e,
            Err(err) => panic!("Cannot load app! {:?}", err),
        };
        ax_println!("entry: {:#x}", entry);

        // Init user stack.
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
        uspace
            .map(
                ustack_vaddr,
                USER_STACK_SIZE,
                MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
                true,
                Backend::new_shared(ustack_vaddr, stack_pages),
            )
            .unwrap();

        // Set up initial user stack with argc, argv, envp, auxv layout
        // as required by Linux ELF ABI.
        let app_name = "hello";
        let stack_data = kernel_elf_parser::app_stack_region(
            &[String::from(app_name)],
            &[],
            &[],
            ustack_top.as_usize(),
        );
        let ustack_pointer = ustack_top.as_usize() - stack_data.len();
        uspace
            .write(ustack_pointer.into(), stack_data.as_slice())
            .unwrap();

        ax_println!("New user address space: {:#x?}", uspace);

        // Let's kick off the user process.
        let user_task = task::spawn_user_task(uspace, entry, ustack_pointer.into());

        // Wait for user process to exit ...
        let exit_code = user_task.join();
        ax_println!("monolithic kernel exit [{:?}] normally!", Some(exit_code));
    }
    #[cfg(not(feature = "axstd"))]
    {
        println!("This application requires the 'axstd' feature for running Linux apps.");
        println!("Run with: cargo xtask run [--arch <ARCH>]");
    }
}
