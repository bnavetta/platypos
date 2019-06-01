use core::fmt;

use bit_field::BitField;

/// Represents the Spurious-Interrupt Vector Register (SVR). See
/// section 10.9 of volume 3A of the Intel manual.
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct SpuriousInterruptVectorRegister(u32);

impl SpuriousInterruptVectorRegister {
    pub fn new(value: u32) -> SpuriousInterruptVectorRegister {
        debug_assert!(value.get_bits(13..32) == 0, "Reserved bits cannot be set");
        debug_assert!(value.get_bits(10..12) == 0, "Reserved bits cannot be set");
        SpuriousInterruptVectorRegister(value)
    }

    /// Get the spurious vector. This is the vector number which is used to deliver
    /// spurious interrupts.
    pub fn spurious_vector(&self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Set the spurious vector.
    pub fn set_spurious_vector(&mut self, vector: u8) {
        self.0.set_bits(0..8, vector.into());
    }

    /// Get the software APIC enable flag. This can be used by software to enable or disable
    /// the local APIC. See section 10.4.3 of volume 3A of the Intel manual.
    pub fn apic_enabled(&self) -> bool {
        self.0.get_bit(8)
    }

    /// Set the software APIC enable flag.
    pub fn set_apic_enabled(&mut self, enabled: bool) {
        self.0.set_bit(8, enabled);
    }

    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl fmt::Debug for SpuriousInterruptVectorRegister {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SpuriousInterruptVectorRegister")
            .field("spurious_vector", &self.spurious_vector())
            .field("apic_enabled", &self.apic_enabled())
            .field("raw", &self.0)
            .finish()
    }
}
