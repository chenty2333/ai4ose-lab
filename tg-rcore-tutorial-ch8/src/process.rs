//! 进程与线程管理模块
//!
//! ## 与第七章的区别
//!
//! 第七章中 `Process` 既是资源容器又是执行单元。
//! 第八章将两者分离：
//! - **Process**：资源容器，管理地址空间、文件描述符、**同步原语列表**、信号
//! - **Thread**：执行单元，管理 TID 和上下文
//!
//! 同一进程的所有线程共享 `Process` 中的资源。
//!
//! ## 新增字段
//!
//! | 字段 | 说明 |
//! |------|------|
//! | `semaphore_list` | 信号量列表（进程内所有线程共享） |
//! | `mutex_list` | 互斥锁列表 |
//! | `condvar_list` | 条件变量列表 |
//!
//! 教程阅读建议：
//!
//! - 先看 `Process` 与 `Thread` 的字段分工：明确“资源归进程、执行归线程”；
//! - 再看 `fork/exec/from_elf`：理解跨线程模型后，进程复制与替换语义如何变化；
//! - 最后结合 `processor.rs` 看线程生命周期与进程资源回收的关系。

use crate::{
    build_flags, fs::Fd, map_portal, parse_flags, processor::ProcessorInner, Sv39, Sv39Manager,
    PROCESSOR,
};
use alloc::{alloc::alloc_zeroed, boxed::Box, collections::BTreeMap, sync::Arc, vec::Vec};
use core::alloc::Layout;
use spin::Mutex;
use tg_kernel_context::{foreign::ForeignContext, LocalContext};
use tg_kernel_vm::{
    page_table::{MmuMeta, VAddr, PPN, VPN},
    AddressSpace,
};
use tg_signal::Signal;
use tg_signal_impl::SignalImpl;
use tg_sync::{Condvar, Mutex as MutexTrait, Semaphore};
use tg_task_manage::{ProcId, ThreadId};
use xmas_elf::{
    header::{self, HeaderPt2, Machine},
    program, ElfFile,
};

/// 死锁检测状态（按进程维护）。
///
/// 练习要求允许 mutex 和 semaphore 分别检测，无需考虑二者混用。
#[derive(Default)]
pub struct DeadlockState {
    /// 当前进程是否启用死锁检测。
    pub enabled: bool,
    mutex_owners: Vec<Option<ThreadId>>,
    mutex_waiting: BTreeMap<ThreadId, usize>,
    sem_available: Vec<usize>,
    sem_allocation: BTreeMap<ThreadId, Vec<usize>>,
    sem_need: BTreeMap<ThreadId, Vec<usize>>,
}

impl DeadlockState {
    /// 开关死锁检测。
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// 注册一把 mutex 资源。
    pub fn register_mutex(&mut self, mutex_id: usize) {
        if self.mutex_owners.len() <= mutex_id {
            self.mutex_owners.resize(mutex_id + 1, None);
        }
    }

    /// 注册一个 semaphore 资源，并更新初始可用数量。
    pub fn register_semaphore(&mut self, sem_id: usize, count: usize) {
        if self.sem_available.len() <= sem_id {
            self.sem_available.resize(sem_id + 1, 0);
            for allocation in self.sem_allocation.values_mut() {
                allocation.resize(sem_id + 1, 0);
            }
            for need in self.sem_need.values_mut() {
                need.resize(sem_id + 1, 0);
            }
        }
        self.sem_available[sem_id] = count;
    }

    fn ensure_sem_thread(&mut self, tid: ThreadId) {
        let resource_count = self.sem_available.len();
        let allocation = self.sem_allocation.entry(tid).or_default();
        if allocation.len() < resource_count {
            allocation.resize(resource_count, 0);
        }
        let need = self.sem_need.entry(tid).or_default();
        if need.len() < resource_count {
            need.resize(resource_count, 0);
        }
    }

    /// 检查 mutex 请求是否会形成等待环。
    pub fn mutex_should_deadlock(&self, tid: ThreadId, mutex_id: usize) -> bool {
        if !self.enabled {
            return false;
        }
        let Some(Some(owner)) = self.mutex_owners.get(mutex_id) else {
            return false;
        };
        if *owner == tid {
            return true;
        }
        let mut cursor = *owner;
        while let Some(waiting_mutex) = self.mutex_waiting.get(&cursor) {
            let Some(Some(next_owner)) = self.mutex_owners.get(*waiting_mutex) else {
                return false;
            };
            if *next_owner == tid {
                return true;
            }
            if *next_owner == cursor {
                return false;
            }
            cursor = *next_owner;
        }
        false
    }

    /// 当前线程真正拿到了 mutex。
    pub fn on_mutex_acquired(&mut self, tid: ThreadId, mutex_id: usize) {
        self.register_mutex(mutex_id);
        self.mutex_waiting.remove(&tid);
        self.mutex_owners[mutex_id] = Some(tid);
    }

    /// 当前线程因 mutex 被占用而阻塞。
    pub fn on_mutex_blocked(&mut self, tid: ThreadId, mutex_id: usize) {
        self.register_mutex(mutex_id);
        self.mutex_waiting.insert(tid, mutex_id);
    }

    /// mutex 解锁后的所有权转移。
    pub fn on_mutex_unlocked(&mut self, mutex_id: usize, waking_tid: Option<ThreadId>) {
        self.register_mutex(mutex_id);
        if let Some(tid) = waking_tid {
            self.mutex_waiting.remove(&tid);
            self.mutex_owners[mutex_id] = Some(tid);
        } else {
            self.mutex_owners[mutex_id] = None;
        }
    }

    /// 检查 semaphore 请求是否会让系统进入不安全状态。
    pub fn semaphore_should_deadlock(&mut self, tid: ThreadId, sem_id: usize) -> bool {
        if !self.enabled {
            return false;
        }
        self.ensure_sem_thread(tid);
        if self.sem_available.get(sem_id).copied().unwrap_or(0) > 0 {
            return false;
        }
        self.sem_need.get_mut(&tid).unwrap()[sem_id] += 1;
        let safe = self.semaphore_safe();
        self.sem_need.get_mut(&tid).unwrap()[sem_id] -= 1;
        !safe
    }

    fn semaphore_safe(&self) -> bool {
        let mut work = self.sem_available.clone();
        let mut tids: Vec<ThreadId> = self
            .sem_allocation
            .keys()
            .chain(self.sem_need.keys())
            .copied()
            .collect();
        tids.sort_unstable();
        tids.dedup();

        let mut finish: BTreeMap<ThreadId, bool> =
            tids.iter().copied().map(|tid| (tid, false)).collect();
        loop {
            let mut progressed = false;
            for tid in tids.iter().copied() {
                if finish[&tid] {
                    continue;
                }
                let need = self.sem_need.get(&tid);
                let allocation = self.sem_allocation.get(&tid);
                let can_finish = (0..work.len()).all(|idx| {
                    let need_count = need.and_then(|v| v.get(idx)).copied().unwrap_or(0);
                    need_count <= work[idx]
                });
                if can_finish {
                    if let Some(allocation) = allocation {
                        for (idx, count) in allocation.iter().enumerate() {
                            work[idx] += count;
                        }
                    }
                    finish.insert(tid, true);
                    progressed = true;
                }
            }
            if !progressed {
                break;
            }
        }
        finish.values().all(|done| *done)
    }

    /// semaphore 立即获取成功。
    pub fn on_semaphore_down_granted(&mut self, tid: ThreadId, sem_id: usize) {
        self.ensure_sem_thread(tid);
        if self.sem_available[sem_id] > 0 {
            self.sem_available[sem_id] -= 1;
        }
        self.sem_allocation.get_mut(&tid).unwrap()[sem_id] += 1;
        let need = &mut self.sem_need.get_mut(&tid).unwrap()[sem_id];
        if *need > 0 {
            *need -= 1;
        }
    }

    /// semaphore 资源不足，线程进入等待。
    pub fn on_semaphore_blocked(&mut self, tid: ThreadId, sem_id: usize) {
        self.ensure_sem_thread(tid);
        self.sem_need.get_mut(&tid).unwrap()[sem_id] += 1;
    }

    /// semaphore 释放后，资源要么回到 Available，要么直接转交给唤醒线程。
    pub fn on_semaphore_up(
        &mut self,
        holder_tid: ThreadId,
        sem_id: usize,
        waking_tid: Option<ThreadId>,
    ) {
        self.ensure_sem_thread(holder_tid);
        let holder_allocation = &mut self.sem_allocation.get_mut(&holder_tid).unwrap()[sem_id];
        if *holder_allocation > 0 {
            *holder_allocation -= 1;
        }
        if let Some(tid) = waking_tid {
            self.ensure_sem_thread(tid);
            let need = &mut self.sem_need.get_mut(&tid).unwrap()[sem_id];
            if *need > 0 {
                *need -= 1;
            }
            self.sem_allocation.get_mut(&tid).unwrap()[sem_id] += 1;
        } else {
            self.sem_available[sem_id] += 1;
        }
    }
}

/// 线程（执行单元）
///
/// 每个线程有独立的 TID 和上下文（寄存器状态、satp）。
/// 同一进程的多个线程共享地址空间。
pub struct Thread {
    /// 线程 ID（不可变）
    pub tid: ThreadId,
    /// 执行上下文（包含 LocalContext + satp）
    pub context: ForeignContext,
}

impl Thread {
    /// 创建新线程
    pub fn new(satp: usize, context: LocalContext) -> Self {
        Self {
            tid: ThreadId::new(),
            context: ForeignContext { context, satp },
        }
    }
}

/// 进程（资源容器）
///
/// 管理地址空间、文件描述符、同步原语、信号等共享资源。
/// 一个进程可以包含多个线程。
pub struct Process {
    /// 进程 ID
    pub pid: ProcId,
    /// 地址空间（所有线程共享）
    pub address_space: AddressSpace<Sv39, Sv39Manager>,
    /// 文件描述符表（所有线程共享）
    pub fd_table: Vec<Option<Mutex<Fd>>>,
    /// 信号处理器
    pub signal: Box<dyn Signal>,
    /// 信号量列表（**本章新增**，所有线程共享）
    pub semaphore_list: Vec<Option<Arc<Semaphore>>>,
    /// 互斥锁列表（**本章新增**，所有线程共享）
    pub mutex_list: Vec<Option<Arc<dyn MutexTrait>>>,
    /// 条件变量列表（**本章新增**，所有线程共享）
    pub condvar_list: Vec<Option<Arc<Condvar>>>,
    /// 练习题：当前进程的死锁检测状态
    pub deadlock: DeadlockState,
}

impl Process {
    /// exec：替换当前进程的地址空间和主线程上下文
    ///
    /// 注意：只支持单线程进程执行 exec
    pub fn exec(&mut self, elf: ElfFile) {
        let (proc, thread) = Process::from_elf(elf).unwrap();
        self.address_space = proc.address_space;
        self.signal.clear();
        self.semaphore_list.clear();
        self.mutex_list.clear();
        self.condvar_list.clear();
        self.deadlock = DeadlockState::default();
        let processor: *mut ProcessorInner = PROCESSOR.get_mut() as *mut ProcessorInner;
        unsafe {
            let pthreads = (*processor).get_thread(self.pid).unwrap();
            (*processor).get_task(pthreads[0]).unwrap().context = thread.context;
        }
    }

    /// fork：创建子进程（复制地址空间和主线程上下文）
    ///
    /// 子进程继承父进程的地址空间（深拷贝）、文件描述符和信号配置。
    /// 同步原语列表不继承（子进程创建空的列表）。
    pub fn fork(&mut self) -> Option<(Self, Thread)> {
        let pid = ProcId::new();
        // 深拷贝地址空间
        let parent_addr_space = &self.address_space;
        let mut address_space: AddressSpace<Sv39, Sv39Manager> = AddressSpace::new();
        parent_addr_space.cloneself(&mut address_space);
        map_portal(&address_space);
        // 复制主线程上下文
        let processor: *mut ProcessorInner = PROCESSOR.get_mut() as *mut ProcessorInner;
        let pthreads = unsafe { (*processor).get_thread(self.pid).unwrap() };
        let context = unsafe {
            (*processor).get_task(pthreads[0]).unwrap().context.context.clone()
        };
        let satp = (8 << 60) | address_space.root_ppn().val();
        let thread = Thread::new(satp, context);
        // 复制文件描述符表
        let new_fd_table: Vec<Option<Mutex<Fd>>> = self.fd_table
            .iter()
            .map(|fd| fd.as_ref().map(|f| Mutex::new(f.lock().clone())))
            .collect();
        Some((
            Self {
                pid,
                address_space,
                fd_table: new_fd_table,
                signal: self.signal.from_fork(),
                // 子进程的同步原语列表初始为空
                semaphore_list: Vec::new(),
                mutex_list: Vec::new(),
                condvar_list: Vec::new(),
                deadlock: DeadlockState::default(),
            },
            thread,
        ))
    }

    /// 从 ELF 文件创建进程和主线程
    ///
    /// 解析 ELF 段，建立地址空间，分配用户栈，创建初始上下文。
    pub fn from_elf(elf: ElfFile) -> Option<(Self, Thread)> {
        let entry = match elf.header.pt2 {
            HeaderPt2::Header64(pt2)
                if pt2.type_.as_type() == header::Type::Executable
                    && pt2.machine.as_machine() == Machine::RISC_V =>
            { pt2.entry_point as usize }
            _ => None?,
        };

        const PAGE_SIZE: usize = 1 << Sv39::PAGE_BITS;
        const PAGE_MASK: usize = PAGE_SIZE - 1;

        let mut address_space = AddressSpace::new();
        for program in elf.program_iter() {
            if !matches!(program.get_type(), Ok(program::Type::Load)) { continue; }
            let off_file = program.offset() as usize;
            let len_file = program.file_size() as usize;
            let off_mem = program.virtual_addr() as usize;
            let end_mem = off_mem + program.mem_size() as usize;
            assert_eq!(off_file & PAGE_MASK, off_mem & PAGE_MASK);
            let mut flags: [u8; 5] = *b"U___V";
            if program.flags().is_execute() { flags[1] = b'X'; }
            if program.flags().is_write() { flags[2] = b'W'; }
            if program.flags().is_read() { flags[3] = b'R'; }
            address_space.map(
                VAddr::new(off_mem).floor()..VAddr::new(end_mem).ceil(),
                &elf.input[off_file..][..len_file],
                off_mem & PAGE_MASK,
                parse_flags(unsafe { core::str::from_utf8_unchecked(&flags) }).unwrap(),
            );
        }
        // 分配 2 页用户栈
        let stack = unsafe {
            alloc_zeroed(Layout::from_size_align_unchecked(
                2 << Sv39::PAGE_BITS, 1 << Sv39::PAGE_BITS,
            ))
        };
        address_space.map_extern(
            VPN::new((1 << 26) - 2)..VPN::new(1 << 26),
            PPN::new(stack as usize >> Sv39::PAGE_BITS),
            build_flags("U_WRV"),
        );
        map_portal(&address_space);
        let satp = (8 << 60) | address_space.root_ppn().val();
        let mut context = LocalContext::user(entry);
        *context.sp_mut() = 1 << 38;
        let thread = Thread::new(satp, context);

        Some((
            Self {
                pid: ProcId::new(),
                address_space,
                fd_table: vec![
                    // stdin
                    Some(Mutex::new(Fd::Empty { read: true, write: false })),
                    // stdout
                    Some(Mutex::new(Fd::Empty { read: false, write: true })),
                    // stderr
                    Some(Mutex::new(Fd::Empty { read: false, write: true })),
                ],
                signal: Box::new(SignalImpl::new()),
                semaphore_list: Vec::new(),
                mutex_list: Vec::new(),
                condvar_list: Vec::new(),
                deadlock: DeadlockState::default(),
            },
            thread,
        ))
    }
}
