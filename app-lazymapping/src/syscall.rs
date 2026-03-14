use axhal::uspace::UserContext;

const SYS_EXIT: usize = 93;

/// Get the syscall number from the UserContext (architecture-specific register).
fn syscall_num(uctx: &UserContext) -> usize {
    #[cfg(any(target_arch = "riscv64", target_arch = "riscv32"))]
    {
        uctx.regs.a7
    }
    #[cfg(target_arch = "aarch64")]
    {
        uctx.x[8] as usize
    }
    #[cfg(target_arch = "x86_64")]
    {
        uctx.rax as usize
    }
    #[cfg(target_arch = "loongarch64")]
    {
        uctx.regs.a7
    }
}

/// Handle a syscall from user space.
/// Returns `Some(exit_code)` if the user process wants to exit,
/// or `None` to continue running.
pub fn handle_syscall(uctx: &mut UserContext) -> Option<i32> {
    ax_println!("handle_syscall ...");

    let num = syscall_num(uctx);
    match num {
        SYS_EXIT => {
            ax_println!("[SYS_EXIT]: system is exiting ..");
            let exit_code = uctx.arg0() as i32;
            Some(exit_code)
        }
        _ => {
            ax_println!("Unimplemented syscall: {}", num);
            uctx.set_retval(usize::MAX); // -ENOSYS equivalent
            None
        }
    }
}
