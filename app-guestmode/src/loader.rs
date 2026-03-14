use alloc::sync::Arc;
use axfs::ROOT_FS_CONTEXT;
use axhal::paging::{MappingFlags, PageSize};
use axhal::mem::phys_to_virt;
use axmm::backend::{Backend, SharedPages};
use axmm::AddrSpace;
use memory_addr::PAGE_SIZE_4K;
use crate::VM_ENTRY;

pub fn load_vm_image(fname: &str, uspace: &mut AddrSpace) -> axio::Result<()> {
    let mut buf = [0u8; 4096];
    load_file(fname, &mut buf)?;

    let flags = MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE | MappingFlags::USER;

    let pages = Arc::new(
        SharedPages::new(PAGE_SIZE_4K, PageSize::Size4K)
            .map_err(|_| axio::Error::NoMemory)?,
    );
    uspace
        .map(
            VM_ENTRY.into(),
            PAGE_SIZE_4K,
            flags,
            true,
            Backend::new_shared(VM_ENTRY.into(), pages),
        )
        .map_err(|_| axio::Error::NoMemory)?;

    let (paddr, _, _) = uspace
        .page_table()
        .query(VM_ENTRY.into())
        .unwrap_or_else(|_| panic!("Mapping failed for segment: {:#x}", VM_ENTRY));

    ax_println!("paddr: {:#x}", paddr);

    unsafe {
        core::ptr::copy_nonoverlapping(
            buf.as_ptr(),
            phys_to_virt(paddr).as_mut_ptr(),
            PAGE_SIZE_4K,
        );
    }

    // AArch64: ensure D-cache is flushed and I-cache is invalidated so the
    // CPU fetches the freshly-written guest instructions, not stale zeros.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        let va = phys_to_virt(paddr).as_usize();
        // Clean every cache line (64 bytes) to Point of Unification
        let mut off = 0usize;
        while off < PAGE_SIZE_4K {
            core::arch::asm!("dc cvau, {}", in(reg) (va + off));
            off += 64;
        }
        core::arch::asm!("dsb ish");
        // Invalidate entire I-cache
        core::arch::asm!("ic iallu");
        core::arch::asm!("dsb ish");
        core::arch::asm!("isb");
    }

    Ok(())
}

fn load_file(fname: &str, buf: &mut [u8]) -> axio::Result<usize> {
    ax_println!("app: {}", fname);
    let ctx = ROOT_FS_CONTEXT.get().expect("Root FS not initialized");
    let file = axfs::File::open(ctx, fname).map_err(|_| axio::Error::NotFound)?;
    let n = axio::Read::read(&mut &file, buf)?;
    Ok(n)
}
