use core::fmt;

use bit_field::BitField;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerMode {
    OneShot = 0,
    Periodic = 1,
    TscDeadline = 2,
}

/// Local Vector Table representation for the local APIC timer. See sections 10.5.1 and 10.5.4 of
/// volume 3A of the Intel manual.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TimerVectorTable(u32);

impl TimerVectorTable {
    pub fn new(value: u32) -> TimerVectorTable {
        // check reserved bits, based on figure 10-8
        debug_assert!(value.get_bits(19..32) == 0, "Reserved bits cannot be set");
        debug_assert!(value.get_bits(13..16) == 0, "Reserved bits cannot be set");
        debug_assert!(value.get_bits(8..12) == 0, "reserved bits cannot be set");
        TimerVectorTable(value)
    }

    /// Get the vector number which timer interrupts are delivered with
    pub fn vector(&self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Set the vector number used for timer interrupts
    pub fn set_vector(&mut self, vector: u8) {
        self.0.set_bits(0..8, vector as u32);
    }

    /// Determine if timer interrupts are masked
    pub fn is_masked(&self) -> bool {
        self.0.get_bit(16)
    }

    /// Set if timer interrupts are masked
    pub fn set_masked(&mut self, masked: bool) {
        self.0.set_bit(16, masked);
    }

    /// Get the mode the timer will operate in (essentially, when it will fire interrupts)
    pub fn timer_mode(&self) -> TimerMode {
        match self.0.get_bits(17..19) {
            0 => TimerMode::OneShot,
            1 => TimerMode::Periodic,
            2 => TimerMode::TscDeadline,
            _ => panic!("Invalid timer mode"),
        }
    }

    /// Set the mode the timer will operate in
    pub fn set_timer_mode(&mut self, mode: TimerMode) {
        self.0.set_bits(
            17..19,
            match mode {
                TimerMode::OneShot => 0,
                TimerMode::Periodic => 1,
                TimerMode::TscDeadline => 2,
            },
        );
    }

    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl fmt::Debug for TimerVectorTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TimerVectorTable")
            .field("vector", &self.vector())
            .field("masked", &self.is_masked())
            .field("timer_mode", &self.timer_mode())
            .field("raw", &self.0)
            .finish()
    }
}

/// Valid Divide Configuration Register values. See figure 10-10 in volume 3A of the Intel manual.
/// An enumeration is used because only certain divisors are allowed, and their representation
/// in the register is... odd.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DivideConfiguration {
    Divide2 = 0b000,
    Divide4 = 0b001,
    Divide8 = 0b010,
    Divide16 = 0b011,
    Divide32 = 0b100,
    Divide64 = 0b101,
    Divide128 = 0b110,
    Divide1 = 0b111,
}

impl DivideConfiguration {
    /// Get the corresponding `DivideConfiguration` for a divisor value. The value must be either 1
    /// or a power of 2 between 2 and 128 (inclusive).
    ///
    /// # Panics
    /// If the divisor value is invalid
    pub fn from_divisor(divisor: u8) -> DivideConfiguration {
        use DivideConfiguration::*;
        match divisor {
            1 => Divide1,
            2 => Divide2,
            4 => Divide4,
            8 => Divide8,
            16 => Divide16,
            32 => Divide32,
            64 => Divide64,
            128 => Divide128,
            other => panic!("Invalid APIC timer divisor: {}", other),
        }
    }
}
