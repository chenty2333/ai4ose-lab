#![allow(dead_code)]

use axerrno::{AxError, AxResult};

const ESR_EC_SHIFT: u32 = 26;
const ESR_EC_MASK: u64 = 0x3F;

/// Exception Class: SVC instruction execution from AArch64 (EL0 → EL1)
const ESR_EC_SVC64: u64 = 0x15;
/// Exception Class: HVC instruction execution from AArch64 (EL1 → EL2)
const ESR_EC_HVC64: u64 = 0x16;

/// PSCI function IDs (SMC32 calling convention)
const PSCI_SYSTEM_OFF: u64 = 0x84000008;
const PSCI_SYSTEM_RESET: u64 = 0x84000009;

/// Guest message parsed from registers on VM exit.
#[derive(Clone, Copy, Debug)]
pub enum GuestMessage {
    /// PSCI SYSTEM_OFF request.
    PsciSystemOff,
    /// PSCI SYSTEM_RESET request.
    PsciSystemReset,
    /// Unknown function ID.
    Unknown(u64),
}

impl GuestMessage {
    /// Parse a guest message from ESR_EL1 and guest GPRs.
    ///
    /// Accepts both SVC64 (EC=0x15) and HVC64 (EC=0x16) exception classes.
    /// Returns `Err` if the exception class is neither.
    pub fn from_esr_and_regs(esr: u64, gprs: &[u64; 31]) -> AxResult<Self> {
        let ec = (esr >> ESR_EC_SHIFT) & ESR_EC_MASK;
        if ec != ESR_EC_SVC64 && ec != ESR_EC_HVC64 {
            return Err(AxError::Unsupported);
        }

        let func_id = gprs[0]; // x0 = function ID
        match func_id {
            PSCI_SYSTEM_OFF => Ok(GuestMessage::PsciSystemOff),
            PSCI_SYSTEM_RESET => Ok(GuestMessage::PsciSystemReset),
            _ => Ok(GuestMessage::Unknown(func_id)),
        }
    }
}
