use alloc::sync::Arc;
use axfs::ROOT_FS_CONTEXT;
use axhal::mem::{PAGE_SIZE_4K, phys_to_virt};
use axhal::paging::{MappingFlags, PageSize};
#[allow(unused_imports)]
use axio::Read;
use axmm::backend::{Backend, SharedPages};
use axmm::AddrSpace;

use crate::APP_ENTRY;

pub fn load_user_app(fname: &str, uspace: &mut AddrSpace) -> Result<(), axio::Error> {
    let mut buf = [0u8; PAGE_SIZE_4K];
    load_file(fname, &mut buf)?;

    // Allocate a physical page for the user code.
    let code_pages = Arc::new(
        SharedPages::new(PAGE_SIZE_4K, PageSize::Size4K)
            .map_err(|_| axio::Error::NoMemory)?,
    );
    uspace
        .map(
            (APP_ENTRY).into(),
            PAGE_SIZE_4K,
            MappingFlags::READ
                | MappingFlags::WRITE
                | MappingFlags::EXECUTE
                | MappingFlags::USER,
            true,
            Backend::new_shared((APP_ENTRY).into(), code_pages),
        )
        .map_err(|_| axio::Error::NoMemory)?;

    let (paddr, _, _) = uspace
        .page_table()
        .query((APP_ENTRY).into())
        .unwrap_or_else(|_| panic!("Mapping failed for segment: {:#x}", APP_ENTRY));

    ax_println!("paddr: {:#x}", paddr);

    unsafe {
        core::ptr::copy_nonoverlapping(
            buf.as_ptr(),
            phys_to_virt(paddr).as_mut_ptr(),
            PAGE_SIZE_4K,
        );
    }

    Ok(())
}

fn load_file(fname: &str, buf: &mut [u8]) -> Result<usize, axio::Error> {
    ax_println!("app: {}", fname);
    let ctx = ROOT_FS_CONTEXT.get().expect("Root FS not initialized");
    let file = axfs::File::open(ctx, fname).map_err(|_| axio::Error::NotFound)?;
    let n = (&file).read(buf)?;
    Ok(n)
}
