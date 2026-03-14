//! Syscall handler for user-space Linux apps.
//!
//! Handles the subset of Linux syscalls needed by a minimal musl-libc
//! "hello world" program (set_tid_address, ioctl, writev, exit, exit_group).
//!
//! Syscall numbers differ between architectures:
//! - riscv64, aarch64, loongarch64 use the "generic" Linux syscall numbers
//! - x86_64 uses its own legacy numbering

use axhal::uspace::UserContext;

// ---- Architecture-specific syscall numbers ----

// Generic Linux syscall numbers (riscv64, aarch64, loongarch64)
#[cfg(not(target_arch = "x86_64"))]
mod nums {
    pub const SYS_IOCTL: usize = 29;
    pub const SYS_WRITEV: usize = 66;
    pub const SYS_EXIT: usize = 93;
    pub const SYS_EXIT_GROUP: usize = 94;
    pub const SYS_SET_TID_ADDRESS: usize = 96;
}

// x86_64 Linux syscall numbers
#[cfg(target_arch = "x86_64")]
mod nums {
    pub const SYS_IOCTL: usize = 16;
    pub const SYS_WRITEV: usize = 20;
    pub const SYS_EXIT: usize = 60;
    pub const SYS_EXIT_GROUP: usize = 231;
    pub const SYS_SET_TID_ADDRESS: usize = 218;
    pub const SYS_ARCH_PRCTL: usize = 158;
    pub const ARCH_SET_FS: usize = 0x1002;
}

use nums::*;

// ---- iovec structure for writev ----

#[repr(C)]
struct IoVec {
    iov_base: usize,
    iov_len: usize,
}

// ---- Architecture-specific register access ----

/// Get the syscall number from the user context.
#[cfg(any(target_arch = "riscv64", target_arch = "riscv32", target_arch = "loongarch64"))]
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

/// Set the syscall return value in the user context.
fn set_syscall_ret(uctx: &mut UserContext, ret: usize) {
    // On all architectures, the return value goes into the first argument
    // register (a0/x0/rax), but on x86_64 it's RAX (not RDI).
    #[cfg(target_arch = "x86_64")]
    {
        uctx.rax = ret as u64;
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        uctx.set_arg0(ret);
    }
}

/// Handle a syscall from user space.
///
/// Returns `Some(exit_code)` if the process should exit, `None` to continue.
pub fn handle_syscall(uctx: &mut UserContext) -> Option<i32> {
    let syscall_num = get_syscall_num(uctx);
    let args = [uctx.arg0(), uctx.arg1(), uctx.arg2()];
    ax_println!("handle_syscall [{}] ...", syscall_num);

    let ret: isize = match syscall_num {
        SYS_IOCTL => {
            ax_println!("Unimplemented syscall: SYS_IOCTL");
            0
        }
        SYS_SET_TID_ADDRESS => {
            // Return a fake thread ID; the actual tid pointer is ignored
            // for this simple single-threaded scenario.
            axtask::current().id().as_u64() as isize
        }
        SYS_WRITEV => sys_writev(args[0] as i32, args[1] as *const IoVec, args[2] as i32),
        SYS_EXIT_GROUP => {
            ax_println!("[SYS_EXIT_GROUP]: system is exiting ..");
            return Some(args[0] as i32);
        }
        SYS_EXIT => {
            ax_println!("[SYS_EXIT]: system is exiting ..");
            return Some(args[0] as i32);
        }
        #[cfg(target_arch = "x86_64")]
        SYS_ARCH_PRCTL => sys_arch_prctl(uctx, args[0], args[1]),
        _ => {
            ax_println!("Unimplemented syscall: {}", syscall_num);
            -(axerrno::LinuxError::ENOSYS.code() as isize)
        }
    };

    set_syscall_ret(uctx, ret as usize);
    None
}

/// Write data from an iovec array to a file descriptor.
///
/// For fd 1 (stdout) and fd 2 (stderr), output goes to the kernel console.
fn sys_writev(fd: i32, iov: *const IoVec, iovcnt: i32) -> isize {
    if fd != 1 && fd != 2 {
        return -(axerrno::LinuxError::EBADF.code() as isize);
    }

    let mut total: isize = 0;
    for i in 0..iovcnt as usize {
        let entry = unsafe { &*iov.add(i) };
        if entry.iov_len == 0 || entry.iov_base == 0 {
            continue;
        }
        let slice =
            unsafe { core::slice::from_raw_parts(entry.iov_base as *const u8, entry.iov_len) };
        // Write to console byte-by-byte (safe for any encoding)
        for &b in slice {
            ax_print!("{}", b as char);
        }
        total += entry.iov_len as isize;
    }
    total
}

/// Handle x86_64 arch_prctl for TLS setup.
#[cfg(target_arch = "x86_64")]
fn sys_arch_prctl(uctx: &mut UserContext, op: usize, addr: usize) -> isize {
    match op {
        ARCH_SET_FS => {
            uctx.fs_base = addr as u64;
            0
        }
        _ => {
            ax_println!("Unimplemented arch_prctl op: {:#x}", op);
            -(axerrno::LinuxError::EINVAL.code() as isize)
        }
    }
}
