//! 信号模块的具体实现。
//!
//! 本模块实现了 [`tg_signal::Signal`] trait，提供完整的信号处理功能。
//!
//! 教程阅读建议：
//!
//! - 先看 `SignalImpl` 的四个核心字段：`received/mask/handling/actions`；
//! - 再看 `handle_signals` 的状态机分支（Frozen、用户处理函数、默认动作）。

#![no_std]
#![deny(warnings, missing_docs)]

extern crate alloc;
use alloc::{boxed::Box, vec::Vec};
use tg_kernel_context::LocalContext;
use tg_signal::{Signal, SignalAction, SignalNo, SignalResult, MAX_SIG};

mod default_action;
use default_action::DefaultAction;
mod signal_set;
use signal_set::SignalSet;

/// 管理一个进程中的信号
pub struct SignalImpl {
    /// 已收到的信号
    received: SignalSet,
    /// 屏蔽的信号掩码
    mask: SignalSet,
    /// 进程是否因 SIGSTOP 处于冻结态
    frozen: bool,
    /// 处理用户信号时保存被打断的上下文，支持嵌套 signal handler
    handling: Vec<LocalContext>,
    /// 当前任务的信号处理函数集
    actions: [Option<SignalAction>; MAX_SIG + 1],
}

impl SignalImpl {
    /// 创建一个新的信号管理器。
    pub fn new() -> Self {
        Self {
            received: SignalSet::empty(),
            mask: SignalSet::empty(),
            frozen: false,
            handling: Vec::new(),
            actions: [None; MAX_SIG + 1],
        }
    }
}

impl Default for SignalImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalImpl {
    /// 获取一个没有被 mask 屏蔽的信号，并从已收到的信号集合中删除它。如果没有这样的信号，则返回空
    fn fetch_signal(&mut self) -> Option<SignalNo> {
        // 在已收到的信号中，寻找一个没有被 mask 屏蔽的信号
        self.received.find_first_one(self.mask).map(|num| {
            self.received.remove_bit(num);
            num.into()
        })
    }

    /// 检查是否收到一个信号，如果是，则接收并删除它
    fn fetch_and_remove(&mut self, signal_no: SignalNo) -> bool {
        if self.received.contain_bit(signal_no as usize)
            && !self.mask.contain_bit(signal_no as usize)
        {
            self.received.remove_bit(signal_no as usize);
            true
        } else {
            false
        }
    }
}

impl Signal for SignalImpl {
    fn from_fork(&mut self) -> Box<dyn Signal> {
        Box::new(Self {
            received: SignalSet::empty(),
            mask: self.mask,
            frozen: false,
            handling: Vec::new(),
            actions: {
                let mut actions = [None; MAX_SIG + 1];
                actions.copy_from_slice(&self.actions);
                actions
            },
        })
    }

    fn clear(&mut self) {
        self.received.clear();
        self.mask.clear();
        self.frozen = false;
        self.handling.clear();
        for action in &mut self.actions {
            action.take();
        }
    }

    /// 添加一个信号
    fn add_signal(&mut self, signal: SignalNo) {
        self.received.add_bit(signal as usize)
    }

    /// 是否当前正在处理信号
    fn is_handling_signal(&self) -> bool {
        self.frozen || !self.handling.is_empty()
    }

    /// 设置一个信号处理函数。`sys_sigaction` 会使用
    fn set_action(&mut self, signum: SignalNo, action: &SignalAction) -> bool {
        if signum == SignalNo::SIGKILL || signum == SignalNo::SIGSTOP {
            false
        } else {
            self.actions[signum as usize] = Some(*action);
            true
        }
    }

    /// 获取一个信号处理函数的值。`sys_sigaction` 会使用
    fn get_action_ref(&self, signum: SignalNo) -> Option<SignalAction> {
        if signum == SignalNo::SIGKILL || signum == SignalNo::SIGSTOP {
            None
        } else {
            Some(self.actions[signum as usize].unwrap_or(SignalAction::default()))
        }
    }

    /// 设置信号掩码，并获取旧的信号掩码，`sys_procmask` 会使用
    fn update_mask(&mut self, mask: usize) -> usize {
        self.mask.set_new(mask.into())
    }

    fn handle_signals(&mut self, current_context: &mut LocalContext) -> SignalResult {
        // 状态机入口：
        // A. 若已在处理信号，则只处理可恢复情形（例如 Frozen + SIGCONT）；
        // B. 否则从 pending 集合提取一个可投递信号并执行默认或用户动作。
        if self.fetch_and_remove(SignalNo::SIGKILL) {
            return SignalResult::ProcessKilled(-(SignalNo::SIGKILL as i32));
        }

        if self.frozen {
            if self.fetch_and_remove(SignalNo::SIGCONT) {
                self.frozen = false;
                return SignalResult::Handled;
            }
            return SignalResult::ProcessSuspended;
        }

        if let Some(signal) = self.fetch_signal() {
            match signal {
                SignalNo::SIGKILL => SignalResult::ProcessKilled(-(signal as i32)),
                SignalNo::SIGSTOP => {
                    self.frozen = true;
                    SignalResult::ProcessSuspended
                }
                _ => {
                    if let Some(action) = self.actions[signal as usize] {
                        // 用户态 handler 可以被更高层后续信号再次打断，因此用栈保存上下文。
                        self.handling.push(current_context.clone());
                        *current_context.pc_mut() = action.handler;
                        *current_context.a_mut(0) = signal as usize;
                        SignalResult::Handled
                    } else {
                        DefaultAction::from(signal).into()
                    }
                }
            }
        } else if self.is_handling_signal() {
            SignalResult::IsHandlingSignal
        } else {
            SignalResult::NoSignal
        }
    }

    fn sig_return(&mut self, current_context: &mut LocalContext) -> bool {
        if let Some(old_ctx) = self.handling.pop() {
            // 用户 handler 执行完毕，恢复被信号打断前的最近一层上下文。
            *current_context = old_ctx;
            true
        } else {
            false
        }
    }
}
