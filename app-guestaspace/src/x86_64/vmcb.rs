//! AMD SVM Virtual Machine Control Block (VMCB).
//!
//! The VMCB is a 4 KB page-aligned structure used by VMRUN/VMEXIT to
//! describe the guest state and intercept configuration.

#![allow(dead_code)]

// ── VMCB Control Area offsets (0x000 – 0x3FF) ───────────────────
// The intercept fields use 16-bit (2-byte) widths for CR/DR:
pub const CTRL_INTERCEPT_CR_READS: usize = 0x000;  // u16
pub const CTRL_INTERCEPT_CR_WRITES: usize = 0x002;  // u16
pub const CTRL_INTERCEPT_DR_READS: usize = 0x004;   // u16
pub const CTRL_INTERCEPT_DR_WRITES: usize = 0x006;  // u16
pub const CTRL_INTERCEPT_EXCEPTIONS: usize = 0x008;  // u32
pub const CTRL_INTERCEPT_MISC1: usize = 0x00C;      // u32 (INTR, NMI, CPUID, HLT, …)
pub const CTRL_INTERCEPT_MISC2: usize = 0x010;      // u32 (VMRUN[0], VMMCALL[1], …)
pub const CTRL_INTERCEPT_MISC3: usize = 0x014;      // u32 (XSETBV, …)
pub const CTRL_IOPM_BASE: usize = 0x040;
pub const CTRL_MSRPM_BASE: usize = 0x048;
pub const CTRL_GUEST_ASID: usize = 0x058;
pub const CTRL_EXIT_CODE: usize = 0x070;
pub const CTRL_EXIT_INFO1: usize = 0x078;
pub const CTRL_EXIT_INFO2: usize = 0x080;
pub const CTRL_NP_ENABLE: usize = 0x090;
pub const CTRL_NCR3: usize = 0x0B0;

// ── VMCB Save Area offsets (0x400 – 0xFFF) ──────────────────────
pub const SAVE_ES: usize = 0x400;
pub const SAVE_CS: usize = 0x410;
pub const SAVE_SS: usize = 0x420;
pub const SAVE_DS: usize = 0x430;
pub const SAVE_FS: usize = 0x440;
pub const SAVE_GS: usize = 0x450;
pub const SAVE_GDTR: usize = 0x460;
pub const SAVE_LDTR: usize = 0x470;
pub const SAVE_IDTR: usize = 0x480;
pub const SAVE_TR: usize = 0x490;
pub const SAVE_EFER: usize = 0x4D0;
pub const SAVE_CR4: usize = 0x548;
pub const SAVE_CR3: usize = 0x550;
pub const SAVE_CR0: usize = 0x558;
pub const SAVE_DR7: usize = 0x560;
pub const SAVE_DR6: usize = 0x568;
pub const SAVE_RFLAGS: usize = 0x570;
pub const SAVE_RIP: usize = 0x578;
pub const SAVE_RSP: usize = 0x5D8;
pub const SAVE_RAX: usize = 0x5F8;

// ── Intercept bits ──────────────────────────────────────────────
/// Bit in CTRL_INTERCEPT_MISC3 for VMRUN intercept (must be set).
pub const INTERCEPT_VMRUN: u32 = 1 << 0;
/// Bit in CTRL_INTERCEPT_MISC3 for VMMCALL intercept.
pub const INTERCEPT_VMMCALL: u32 = 1 << 1;
/// Bit in CTRL_INTERCEPT_MISC2 for HLT intercept.
pub const INTERCEPT_HLT: u32 = 1 << 24;

// ── VMEXIT codes ────────────────────────────────────────────────
pub const VMEXIT_HLT: u64 = 0x78;
pub const VMEXIT_VMMCALL: u64 = 0x81;
pub const VMEXIT_NPF: u64 = 0x400;
pub const VMEXIT_INVALID: u64 = u64::MAX; // -1

/// A 4 KB page-aligned VMCB accessed as a raw byte buffer.
#[repr(C, align(4096))]
pub struct Vmcb {
    pub data: [u8; 4096],
}

impl Vmcb {
    pub const fn new() -> Self {
        Self { data: [0u8; 4096] }
    }

    // ── primitive accessors ──────────────────────────────────────

    #[inline]
    pub fn read_u16(&self, off: usize) -> u16 {
        u16::from_le_bytes([self.data[off], self.data[off + 1]])
    }
    #[inline]
    pub fn write_u16(&mut self, off: usize, v: u16) {
        let b = v.to_le_bytes();
        self.data[off] = b[0];
        self.data[off + 1] = b[1];
    }
    #[inline]
    pub fn read_u32(&self, off: usize) -> u32 {
        u32::from_le_bytes([
            self.data[off],
            self.data[off + 1],
            self.data[off + 2],
            self.data[off + 3],
        ])
    }
    #[inline]
    pub fn write_u32(&mut self, off: usize, v: u32) {
        let b = v.to_le_bytes();
        self.data[off..off + 4].copy_from_slice(&b);
    }
    #[inline]
    pub fn read_u64(&self, off: usize) -> u64 {
        let mut b = [0u8; 8];
        b.copy_from_slice(&self.data[off..off + 8]);
        u64::from_le_bytes(b)
    }
    #[inline]
    pub fn write_u64(&mut self, off: usize, v: u64) {
        let b = v.to_le_bytes();
        self.data[off..off + 8].copy_from_slice(&b);
    }

    // ── segment descriptor helper ───────────────────────────────

    /// Write a VMCB segment descriptor (16 bytes).
    ///
    /// Layout at `off`: selector(u16) attrib(u16) limit(u32) base(u64)
    pub fn set_segment(&mut self, off: usize, sel: u16, attr: u16, limit: u32, base: u64) {
        self.write_u16(off, sel);
        self.write_u16(off + 2, attr);
        self.write_u32(off + 4, limit);
        self.write_u64(off + 8, base);
    }

    // ── convenience accessors ───────────────────────────────────

    pub fn exit_code(&self) -> u64 {
        self.read_u64(CTRL_EXIT_CODE)
    }
    pub fn exit_info1(&self) -> u64 {
        self.read_u64(CTRL_EXIT_INFO1)
    }
    pub fn exit_info2(&self) -> u64 {
        self.read_u64(CTRL_EXIT_INFO2)
    }
    pub fn guest_rax(&self) -> u64 {
        self.read_u64(SAVE_RAX)
    }
    pub fn guest_rip(&self) -> u64 {
        self.read_u64(SAVE_RIP)
    }
}
