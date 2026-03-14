//! Meta crate for the componentized rCore Tutorial workspace.
//!
//! This crate is a distribution bundle so learners can fetch the full
//! teaching workspace via crates.io tooling (for example, cargo-clone).
//!
//! # Usage
//!
//! After cloning this crate via `cargo clone tg-rcore-tutorial`, you can
//! extract all the submodules by running:
//!
//! ```bash
//! bash scripts/extract_submodules.sh
//! ```
//!
//! This will extract all chapter crates and component crates from the
//! bundle directory, enabling you to build and develop the full workspace.
//!
//! # Components
//!
//! The bundle contains:
//! - **Chapter crates** (ch1-ch8): Progressive OS kernel implementations
//! - **Component crates**: Reusable kernel modules (console, syscall, memory, etc.)
//! - **Test utilities**: User programs and output checkers

/// Crate identifier for the workspace bundle.
pub const BUNDLE_NAME: &str = "tg-rcore-tutorial";

/// Version of the bundle.
pub const BUNDLE_VERSION: &str = "0.4.5";

/// List of all included submodule crates.
pub const SUBMODULE_CRATES: &[&str] = &[
    "tg-rcore-tutorial-ch1",
    "tg-rcore-tutorial-ch2",
    "tg-rcore-tutorial-ch3",
    "tg-rcore-tutorial-ch4",
    "tg-rcore-tutorial-ch5",
    "tg-rcore-tutorial-ch6",
    "tg-rcore-tutorial-ch7",
    "tg-rcore-tutorial-ch8",
    "tg-rcore-tutorial-checker",
    "tg-rcore-tutorial-console",
    "tg-rcore-tutorial-easy-fs",
    "tg-rcore-tutorial-kernel-alloc",
    "tg-rcore-tutorial-kernel-context",
    "tg-rcore-tutorial-kernel-vm",
    "tg-rcore-tutorial-linker",
    "tg-rcore-tutorial-sbi",
    "tg-rcore-tutorial-signal",
    "tg-rcore-tutorial-signal-defs",
    "tg-rcore-tutorial-signal-impl",
    "tg-rcore-tutorial-sync",
    "tg-rcore-tutorial-syscall",
    "tg-rcore-tutorial-task-manage",
    "tg-rcore-tutorial-user",
];
