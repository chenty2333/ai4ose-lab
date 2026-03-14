use alloc::sync::Arc;
use axfs::ROOT_FS_CONTEXT;
use axhal::paging::{MappingFlags, PageSize};
use axhal::mem::phys_to_virt;
use axmm::backend::{Backend, SharedPages};
use axmm::AddrSpace;
use memory_addr::PAGE_SIZE_4K;
use crate::VM_ENTRY;

/// Load a guest binary from the filesystem into the given address space.
///
/// Supports binaries of any size (multi-page loading).
/// Each page is allocated via SharedPages and mapped at VM_ENTRY + offset.
pub fn load_vm_image(fname: &str, uspace: &mut AddrSpace) -> axio::Result<()> {
    ax_println!("app: {}", fname);
    let ctx = ROOT_FS_CONTEXT.get().expect("Root FS not initialized");
    let file = axfs::File::open(ctx, fname).map_err(|_| axio::Error::NotFound)?;

    let flags = MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE | MappingFlags::USER;

    let mut page_offset = 0usize;
    let mut total_bytes = 0usize;

    loop {
        let mut buf = [0u8; 4096];
        let n = axio::Read::read(&mut &file, &mut buf)?;
        if n == 0 {
            break;
        }
        total_bytes += n;

        let va = VM_ENTRY + page_offset;

        // Allocate a fresh page and map it
        let pages = Arc::new(
            SharedPages::new(PAGE_SIZE_4K, PageSize::Size4K)
                .map_err(|_| axio::Error::NoMemory)?,
        );
        uspace
            .map(
                va.into(),
                PAGE_SIZE_4K,
                flags,
                true,
                Backend::new_shared(va.into(), pages),
            )
            .map_err(|_| axio::Error::NoMemory)?;

        // Query the physical address and copy data
        let (paddr, _, _) = uspace
            .page_table()
            .query(va.into())
            .unwrap_or_else(|_| panic!("Mapping failed for segment: {:#x}", va));

        unsafe {
            core::ptr::copy_nonoverlapping(
                buf.as_ptr(),
                phys_to_virt(paddr).as_mut_ptr(),
                n,
            );
        }

        // AArch64: flush D-cache per page so I-cache sees fresh data
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let cache_va = phys_to_virt(paddr).as_usize();
            let mut off = 0usize;
            while off < PAGE_SIZE_4K {
                core::arch::asm!("dc cvau, {}", in(reg) (cache_va + off));
                off += 64;
            }
        }

        page_offset += PAGE_SIZE_4K;

        if n < PAGE_SIZE_4K {
            break; // Partial page = end of file
        }
    }

    // Final I-cache invalidation for aarch64
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!("dsb ish");
        core::arch::asm!("ic iallu");
        core::arch::asm!("dsb ish");
        core::arch::asm!("isb");
    }

    // Print summary
    let first_paddr = uspace
        .page_table()
        .query(VM_ENTRY.into())
        .map(|(pa, _, _)| pa)
        .unwrap();
    ax_println!("paddr: PA:{:#x}", first_paddr);
    ax_println!("Loaded {} bytes ({} pages) from {}", total_bytes, page_offset / PAGE_SIZE_4K, fname);

    Ok(())
}
