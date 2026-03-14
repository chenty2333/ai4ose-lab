//! ELF loader: parses ELF headers and loads PT_LOAD segments into user space.
//!
//! Uses manual ELF parsing (no external `elf` crate) and `axfs` for file I/O.

use alloc::sync::Arc;
use alloc::vec;
use axfs::ROOT_FS_CONTEXT;
use axhal::paging::{MappingFlags, PageSize};
#[allow(unused_imports)]
use axio::{Read, Seek, SeekFrom};
use axmm::AddrSpace;
use axmm::backend::{Backend, SharedPages};
use memory_addr::{MemoryAddr, VirtAddr};

// ---- Minimal ELF64 structures ----

const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];
const PT_LOAD: u32 = 1;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Elf64Ehdr {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Elf64Phdr {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
}

const EHDR_SIZE: usize = core::mem::size_of::<Elf64Ehdr>();
const PHDR_SIZE: usize = core::mem::size_of::<Elf64Phdr>();

fn read_exact(file: &axfs::File, buf: &mut [u8]) -> Result<(), axio::Error> {
    let mut offset = 0;
    while offset < buf.len() {
        let n = (&*file).read(&mut buf[offset..])?;
        if n == 0 {
            return Err(axio::Error::UnexpectedEof);
        }
        offset += n;
    }
    Ok(())
}

pub fn load_user_app(fname: &str, uspace: &mut AddrSpace) -> Result<usize, axio::Error> {
    ax_println!("app: {}", fname);
    let ctx = ROOT_FS_CONTEXT.get().expect("Root FS not initialized");
    let file = axfs::File::open(ctx, fname).map_err(|_| axio::Error::NotFound)?;

    // Read ELF header
    let mut ehdr_buf = [0u8; EHDR_SIZE];
    read_exact(&file, &mut ehdr_buf)?;
    let ehdr: Elf64Ehdr = unsafe { core::ptr::read_unaligned(ehdr_buf.as_ptr() as *const _) };

    // Validate ELF magic
    if ehdr.e_ident[..4] != ELF_MAGIC {
        ax_println!("Error: not a valid ELF file");
        return Err(axio::Error::InvalidData);
    }

    let entry = ehdr.e_entry as usize;
    let phnum = ehdr.e_phnum as usize;
    let phoff = ehdr.e_phoff;

    // Read program headers
    let phdr_total = phnum * PHDR_SIZE;
    let mut phdr_buf = vec![0u8; phdr_total];
    (&file).seek(SeekFrom::Start(phoff))?;
    read_exact(&file, &mut phdr_buf)?;

    // Process PT_LOAD segments
    for i in 0..phnum {
        let phdr: Elf64Phdr =
            unsafe { core::ptr::read_unaligned(phdr_buf.as_ptr().add(i * PHDR_SIZE) as *const _) };

        if phdr.p_type != PT_LOAD {
            continue;
        }

        ax_println!(
            "phdr: offset: {:#X}=>{:#X} size: {:#X}=>{:#X}",
            phdr.p_offset,
            phdr.p_vaddr,
            phdr.p_filesz,
            phdr.p_memsz
        );

        let vaddr = VirtAddr::from(phdr.p_vaddr as usize).align_down_4k();
        let vaddr_end = VirtAddr::from((phdr.p_vaddr + phdr.p_memsz) as usize).align_up_4k();
        let map_size = vaddr_end - vaddr;

        ax_println!("{:#x} - {:#x}", vaddr, vaddr_end);

        // Allocate pages and map them
        let pages = Arc::new(
            SharedPages::new(map_size, PageSize::Size4K).map_err(|_| axio::Error::NoMemory)?,
        );
        uspace
            .map(
                vaddr,
                map_size,
                MappingFlags::READ
                    | MappingFlags::WRITE
                    | MappingFlags::EXECUTE
                    | MappingFlags::USER,
                true,
                Backend::new_shared(vaddr, pages),
            )
            .map_err(|_| axio::Error::NoMemory)?;

        // Read segment data from file and write to user space
        let mut data = vec![0u8; phdr.p_memsz as usize];
        (&file).seek(SeekFrom::Start(phdr.p_offset))?;
        let filesz = phdr.p_filesz as usize;
        let mut index = 0;
        while index < filesz {
            let n = (&file).read(&mut data[index..filesz])?;
            if n == 0 {
                break;
            }
            index += n;
        }
        // BSS region (p_memsz > p_filesz) is already zero-filled
        uspace
            .write(VirtAddr::from(phdr.p_vaddr as usize), &data)
            .map_err(|_| axio::Error::WriteZero)?;
    }

    Ok(entry)
}
