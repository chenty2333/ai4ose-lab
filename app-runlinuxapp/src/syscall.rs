//! Syscall handler for the minimal Linux user-space runtime.

use alloc::sync::Arc;
use alloc::vec::Vec;
use axfs::{CachedFile, File, FileBackend, OpenOptions, ROOT_FS_CONTEXT};
use axhal::paging::MappingFlags;
use axhal::uspace::UserContext;
use axmm::{AddrSpace, backend::Backend};
use axsync::{Mutex, spin::SpinNoIrq};
use core::ffi::{CStr, c_char, c_void};
use memory_addr::{PAGE_SIZE_4K, VirtAddr, VirtAddrRange};

// ---- Architecture-specific syscall numbers ----

#[cfg(not(target_arch = "x86_64"))]
mod nums {
    pub const SYS_IOCTL: usize = 29;
    pub const SYS_CLOSE: usize = 57;
    pub const SYS_OPENAT: usize = 56;
    pub const SYS_READ: usize = 63;
    pub const SYS_WRITE: usize = 64;
    pub const SYS_WRITEV: usize = 66;
    pub const SYS_EXIT: usize = 93;
    pub const SYS_EXIT_GROUP: usize = 94;
    pub const SYS_SET_TID_ADDRESS: usize = 96;
    pub const SYS_MMAP: usize = 222;
}

#[cfg(target_arch = "x86_64")]
mod nums {
    pub const SYS_READ: usize = 0;
    pub const SYS_WRITE: usize = 1;
    pub const SYS_CLOSE: usize = 3;
    pub const SYS_MMAP: usize = 9;
    pub const SYS_IOCTL: usize = 16;
    pub const SYS_WRITEV: usize = 20;
    pub const SYS_EXIT: usize = 60;
    pub const SYS_EXIT_GROUP: usize = 231;
    pub const SYS_SET_TID_ADDRESS: usize = 218;
    pub const SYS_OPENAT: usize = 257;
    pub const SYS_ARCH_PRCTL: usize = 158;
    pub const ARCH_SET_FS: usize = 0x1002;
}

use nums::*;

const AT_FDCWD: i32 = -100;

const O_WRONLY: i32 = 0x1;
const O_RDWR: i32 = 0x2;
const O_CREAT: i32 = 0x40;
const O_EXCL: i32 = 0x80;
const O_TRUNC: i32 = 0x200;
const O_APPEND: i32 = 0x400;

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug)]
    struct MmapProt: i32 {
        const PROT_READ = 1 << 0;
        const PROT_WRITE = 1 << 1;
        const PROT_EXEC = 1 << 2;
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug)]
    struct MmapReqFlags: i32 {
        const MAP_SHARED = 1 << 0;
        const MAP_PRIVATE = 1 << 1;
        const MAP_FIXED = 1 << 4;
        const MAP_ANONYMOUS = 1 << 5;
    }
}

impl From<MmapProt> for MappingFlags {
    fn from(value: MmapProt) -> Self {
        let mut flags = MappingFlags::USER;
        if value.contains(MmapProt::PROT_READ) {
            flags |= MappingFlags::READ;
        }
        if value.contains(MmapProt::PROT_WRITE) {
            flags |= MappingFlags::WRITE;
        }
        if value.contains(MmapProt::PROT_EXEC) {
            flags |= MappingFlags::EXECUTE;
        }
        flags
    }
}

#[repr(C)]
struct IoVec {
    iov_base: usize,
    iov_len: usize,
}

static USER_ASPACE: SpinNoIrq<Option<Arc<Mutex<AddrSpace>>>> = SpinNoIrq::new(None);
static OPEN_FILES: SpinNoIrq<Vec<Option<Arc<File>>>> = SpinNoIrq::new(Vec::new());

pub fn install_user_aspace(aspace: Arc<Mutex<AddrSpace>>) {
    *USER_ASPACE.lock() = Some(aspace);
    OPEN_FILES.lock().clear();
}

#[cfg(any(
    target_arch = "riscv64",
    target_arch = "riscv32",
    target_arch = "loongarch64"
))]
fn get_syscall_num(uctx: &UserContext) -> usize {
    uctx.regs.a7
}

#[cfg(target_arch = "aarch64")]
fn get_syscall_num(uctx: &UserContext) -> usize {
    uctx.x[8] as usize
}

#[cfg(target_arch = "x86_64")]
fn get_syscall_num(uctx: &UserContext) -> usize {
    uctx.rax as usize
}

fn set_syscall_ret(uctx: &mut UserContext, ret: usize) {
    #[cfg(target_arch = "x86_64")]
    {
        uctx.rax = ret as u64;
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        uctx.set_arg0(ret);
    }
}

pub fn handle_syscall(uctx: &mut UserContext) -> Option<i32> {
    let syscall_num = get_syscall_num(uctx);
    ax_println!("handle_syscall [{}] ...", syscall_num);

    let ret: isize = match syscall_num {
        SYS_IOCTL => {
            ax_println!("Ignore SYS_IOCTL");
            0
        }
        SYS_SET_TID_ADDRESS => axtask::current().id().as_u64() as isize,
        SYS_OPENAT => sys_openat(
            uctx.arg0() as i32,
            uctx.arg1() as *const c_char,
            uctx.arg2() as i32,
            uctx.arg3() as u32,
        ),
        SYS_CLOSE => sys_close(uctx.arg0() as i32),
        SYS_READ => sys_read(uctx.arg0() as i32, uctx.arg1() as *mut c_void, uctx.arg2()),
        SYS_WRITE => sys_write(
            uctx.arg0() as i32,
            uctx.arg1() as *const c_void,
            uctx.arg2(),
        ),
        SYS_WRITEV => sys_writev(
            uctx.arg0() as i32,
            uctx.arg1() as *const IoVec,
            uctx.arg2() as i32,
        ),
        SYS_MMAP => sys_mmap(
            uctx.arg0() as *mut usize,
            uctx.arg1(),
            uctx.arg2() as i32,
            uctx.arg3() as i32,
            uctx.arg4() as i32,
            uctx.arg5() as isize,
        ),
        SYS_EXIT_GROUP => {
            ax_println!("[SYS_EXIT_GROUP]: system is exiting ..");
            return Some(uctx.arg0() as i32);
        }
        SYS_EXIT => {
            ax_println!("[SYS_EXIT]: system is exiting ..");
            return Some(uctx.arg0() as i32);
        }
        #[cfg(target_arch = "x86_64")]
        SYS_ARCH_PRCTL => sys_arch_prctl(uctx, uctx.arg0(), uctx.arg1()),
        _ => {
            ax_println!("Unimplemented syscall: {}", syscall_num);
            -(axerrno::LinuxError::ENOSYS.code() as isize)
        }
    };

    set_syscall_ret(uctx, ret as usize);
    None
}

fn sys_openat(dfd: i32, fname: *const c_char, flags: i32, mode: u32) -> isize {
    if dfd != AT_FDCWD || fname.is_null() {
        return linux_errno(axerrno::LinuxError::EINVAL);
    }

    let path = match unsafe { CStr::from_ptr(fname) }.to_str() {
        Ok(path) => path,
        Err(_) => return linux_errno(axerrno::LinuxError::EINVAL),
    };

    let ctx = match ROOT_FS_CONTEXT.get() {
        Some(ctx) => ctx,
        None => return linux_errno(axerrno::LinuxError::ENOENT),
    };

    let access_mode = flags & 0x3;
    let read = access_mode == 0 || access_mode == O_RDWR;
    let write = access_mode == O_WRONLY || access_mode == O_RDWR;
    let append = flags & O_APPEND != 0;
    let create = flags & O_CREAT != 0;
    let truncate = flags & O_TRUNC != 0;
    let create_new = create && flags & O_EXCL != 0;

    let file = match OpenOptions::new()
        .read(read)
        .write(write)
        .append(append)
        .create(create)
        .truncate(truncate)
        .create_new(create_new)
        .mode(mode)
        .open(ctx, path)
        .and_then(axfs::OpenResult::into_file)
    {
        Ok(file) => Arc::new(file),
        Err(err) => return linux_errno(vfs_to_linux(err)),
    };

    alloc_fd(file) as isize
}

fn sys_close(fd: i32) -> isize {
    if fd < 3 {
        return 0;
    }
    if take_fd(fd).is_some() {
        0
    } else {
        linux_errno(axerrno::LinuxError::EBADF)
    }
}

fn sys_read(fd: i32, buf: *mut c_void, count: usize) -> isize {
    if count == 0 {
        return 0;
    }
    if buf.is_null() {
        return linux_errno(axerrno::LinuxError::EFAULT);
    }
    if fd == 0 {
        return 0;
    }
    let file = match get_fd(fd) {
        Some(file) => file,
        None => return linux_errno(axerrno::LinuxError::EBADF),
    };
    let dst = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, count) };
    match file.read(dst) {
        Ok(n) => n as isize,
        Err(err) => linux_errno(axerrno::LinuxError::from(err)),
    }
}

fn sys_write(fd: i32, buf: *const c_void, count: usize) -> isize {
    if count == 0 {
        return 0;
    }
    if buf.is_null() {
        return linux_errno(axerrno::LinuxError::EFAULT);
    }
    let src = unsafe { core::slice::from_raw_parts(buf as *const u8, count) };
    match fd {
        1 | 2 => {
            for &b in src {
                ax_print!("{}", b as char);
            }
            count as isize
        }
        _ => {
            let file = match get_fd(fd) {
                Some(file) => file,
                None => return linux_errno(axerrno::LinuxError::EBADF),
            };
            match file.write(src) {
                Ok(n) => n as isize,
                Err(err) => linux_errno(axerrno::LinuxError::from(err)),
            }
        }
    }
}

fn sys_writev(fd: i32, iov: *const IoVec, iovcnt: i32) -> isize {
    if iovcnt < 0 {
        return linux_errno(axerrno::LinuxError::EINVAL);
    }

    let mut total = 0isize;
    for idx in 0..iovcnt as usize {
        let entry = unsafe { &*iov.add(idx) };
        if entry.iov_len == 0 || entry.iov_base == 0 {
            continue;
        }
        let ret = sys_write(fd, entry.iov_base as *const c_void, entry.iov_len);
        if ret < 0 {
            return if total == 0 { ret } else { total };
        }
        total += ret;
    }
    total
}

fn sys_mmap(
    addr: *mut usize,
    length: usize,
    prot: i32,
    flags: i32,
    fd: i32,
    offset: isize,
) -> isize {
    if length == 0 || offset < 0 || offset as usize % PAGE_SIZE_4K != 0 {
        return linux_errno(axerrno::LinuxError::EINVAL);
    }

    let req_flags = match MmapReqFlags::from_bits(flags) {
        Some(flags) => flags,
        None => return linux_errno(axerrno::LinuxError::EINVAL),
    };
    if req_flags.contains(MmapReqFlags::MAP_FIXED)
        || req_flags.contains(MmapReqFlags::MAP_ANONYMOUS)
    {
        return linux_errno(axerrno::LinuxError::ENOSYS);
    }
    if !req_flags.intersects(MmapReqFlags::MAP_SHARED | MmapReqFlags::MAP_PRIVATE) {
        return linux_errno(axerrno::LinuxError::EINVAL);
    }
    if !addr.is_null() {
        return linux_errno(axerrno::LinuxError::ENOSYS);
    }

    let mmap_flags = MappingFlags::from(MmapProt::from_bits_truncate(prot));
    if !mmap_flags.contains(MappingFlags::READ) {
        return linux_errno(axerrno::LinuxError::EINVAL);
    }

    let file = match get_fd(fd) {
        Some(file) => file,
        None => return linux_errno(axerrno::LinuxError::EBADF),
    };
    let cached = match file.backend() {
        Ok(FileBackend::Cached(cached)) => cached.clone(),
        Ok(FileBackend::Direct(loc)) => CachedFile::get_or_create(loc.clone()),
        Err(_) => return linux_errno(axerrno::LinuxError::EBADF),
    };

    let aspace = match USER_ASPACE.lock().clone() {
        Some(aspace) => aspace,
        None => return linux_errno(axerrno::LinuxError::ENOMEM),
    };

    let map_size = align_up_4k(length);
    let start = {
        let guard = aspace.lock();
        let limit =
            VirtAddrRange::from_start_size(guard.base(), guard.size() - crate::USER_STACK_SIZE);
        match guard.find_free_area(
            VirtAddr::from(0x1000_0000usize),
            map_size,
            limit,
            PAGE_SIZE_4K,
        ) {
            Some(start) => start,
            None => return linux_errno(axerrno::LinuxError::ENOMEM),
        }
    };

    let backend = Backend::new_file(start, cached, file.flags(), offset as usize, &aspace);
    let mut guard = aspace.lock();
    match guard.map(start, map_size, mmap_flags, true, backend) {
        Ok(()) => start.as_usize() as isize,
        Err(err) => linux_errno(axerrno::LinuxError::from(err)),
    }
}

#[cfg(target_arch = "x86_64")]
fn sys_arch_prctl(uctx: &mut UserContext, op: usize, addr: usize) -> isize {
    match op {
        ARCH_SET_FS => {
            uctx.fs_base = addr as u64;
            0
        }
        _ => linux_errno(axerrno::LinuxError::EINVAL),
    }
}

fn alloc_fd(file: Arc<File>) -> usize {
    let mut table = OPEN_FILES.lock();
    for (idx, slot) in table.iter_mut().enumerate() {
        if slot.is_none() {
            *slot = Some(file);
            return idx + 3;
        }
    }
    table.push(Some(file));
    table.len() + 2
}

fn get_fd(fd: i32) -> Option<Arc<File>> {
    if fd < 3 {
        return None;
    }
    OPEN_FILES
        .lock()
        .get((fd - 3) as usize)
        .and_then(|file| file.as_ref())
        .cloned()
}

fn take_fd(fd: i32) -> Option<Arc<File>> {
    if fd < 3 {
        return None;
    }
    OPEN_FILES
        .lock()
        .get_mut((fd - 3) as usize)
        .and_then(Option::take)
}

fn align_up_4k(value: usize) -> usize {
    value.div_ceil(PAGE_SIZE_4K) * PAGE_SIZE_4K
}

fn linux_errno(err: axerrno::LinuxError) -> isize {
    -(err.code() as isize)
}

fn vfs_to_linux(err: axfs_ng_vfs::VfsError) -> axerrno::LinuxError {
    use axfs_ng_vfs::VfsError;
    match err {
        VfsError::AlreadyExists => axerrno::LinuxError::EEXIST,
        VfsError::BadAddress => axerrno::LinuxError::EFAULT,
        VfsError::BadFileDescriptor => axerrno::LinuxError::EBADF,
        VfsError::CrossesDevices => axerrno::LinuxError::EXDEV,
        VfsError::DirectoryNotEmpty => axerrno::LinuxError::ENOTEMPTY,
        VfsError::InvalidData => axerrno::LinuxError::EINVAL,
        VfsError::InvalidInput => axerrno::LinuxError::EINVAL,
        VfsError::IsADirectory => axerrno::LinuxError::EISDIR,
        VfsError::NoMemory => axerrno::LinuxError::ENOMEM,
        VfsError::NotADirectory => axerrno::LinuxError::ENOTDIR,
        VfsError::NotFound => axerrno::LinuxError::ENOENT,
        VfsError::PermissionDenied => axerrno::LinuxError::EACCES,
        VfsError::ReadOnlyFilesystem => axerrno::LinuxError::EROFS,
        VfsError::ResourceBusy => axerrno::LinuxError::EBUSY,
        VfsError::StorageFull => axerrno::LinuxError::ENOSPC,
        VfsError::Unsupported => axerrno::LinuxError::ENOSYS,
        _ => axerrno::LinuxError::EIO,
    }
}
