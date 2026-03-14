#[derive(Clone)]
#[repr(C)]
pub struct GeneralPurposeRegisters(pub [u64; 31]); // x0-x30

impl Default for GeneralPurposeRegisters {
    fn default() -> Self {
        Self([0u64; 31])
    }
}

#[allow(dead_code)]
impl GeneralPurposeRegisters {
    /// Returns the value of register xN.
    pub fn x(&self, n: usize) -> u64 {
        assert!(n < 31, "GPR index out of range");
        self.0[n]
    }

    /// Sets the value of register xN.
    pub fn set_x(&mut self, n: usize, val: u64) {
        assert!(n < 31, "GPR index out of range");
        self.0[n] = val;
    }
}
