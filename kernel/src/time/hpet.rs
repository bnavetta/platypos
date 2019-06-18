use core::hint::spin_loop;
use core::time::Duration;

use bit_field::BitField;
use log::debug;
use spin::Once;
use x86_64::PhysAddr;

use super::{DelayTimer, WallClockTimer};
use crate::memory::physical_to_virtual;

const GENERAL_CAPABILITIES_REGISTER: usize = 0x00;
const GENERAL_CONFIGURATION_REGISTER: usize = 0x010;
const MAIN_COUNTER_VALUE_REGISTER: usize = 0x0F0;

/// Driver for the High Precision Event Timer
pub struct Hpet {
    base: *mut u8, // Use a *mut u8 so we can do byte-level offsets
    tick_period: u64, // tick period of the main counter in femtoseconds
                   // TODO: will need a mutex for protecting against timer/comparator modifications
}

impl Hpet {
    fn new(base: *mut u8) -> Hpet {
        // get tick period out of general capabilities register
        let tick_period =
            unsafe { (base.add(GENERAL_CAPABILITIES_REGISTER) as *const u64).read_volatile() }
                .get_bits(32..64);

        let hpet = Hpet { base, tick_period };

        hpet.capabilities().log_capabilities();

        hpet
    }

    /// Read a HPET register
    ///
    /// # Unsafety
    /// The offset must be a valid register offset
    unsafe fn read(&self, offset: usize) -> u64 {
        (self.base.add(offset) as *const u64).read_volatile()
    }

    /// Write a HPET register
    ///
    /// # Unsafety
    /// The offset and value must both be valid. Configuring timers can start IRQs, which must have
    /// registered interrupt handlers.
    unsafe fn write(&mut self, offset: usize, value: u64) {
        (self.base.add(offset) as *mut u64).write_volatile(value)
    }

    pub fn capabilities(&self) -> Capabilities {
        Capabilities(unsafe { self.read(GENERAL_CAPABILITIES_REGISTER) })
    }

    pub fn main_counter(&self) -> u64 {
        unsafe { self.read(MAIN_COUNTER_VALUE_REGISTER) }
    }

    pub unsafe fn enable(&mut self) {
        let mut config = self.read(GENERAL_CONFIGURATION_REGISTER);
        config.set_bit(0, true); // bit 0 is the enable bit
        self.write(GENERAL_CONFIGURATION_REGISTER, config);
    }

    pub fn disable(&mut self) {
        // not unsafe because this'll stop interrupts, not start them
        let mut config = unsafe { self.read(GENERAL_CONFIGURATION_REGISTER) };
        config.set_bit(0, false); // bit 0 is the enable bit
        unsafe {
            self.write(GENERAL_CONFIGURATION_REGISTER, config);
        }
    }

    pub fn is_enabled(&self) -> bool {
        let config = unsafe { self.read(GENERAL_CONFIGURATION_REGISTER) };
        config.get_bit(0)
    }

    /// Enable or disable the legacy replacement mapping. If enabled, HPET timer 0 replaces PIT
    /// interrupts and HPET timer 1 replaces RTC interrupts.
    pub fn set_legacy_replacement(&mut self, enable: bool) {
        let mut config = unsafe { self.read(GENERAL_CONFIGURATION_REGISTER) };
        config.set_bit(1, enable);
        unsafe { self.write(GENERAL_CONFIGURATION_REGISTER, config) }
    }
}

impl WallClockTimer for Hpet {
    fn current_timestamp(&self) -> Duration {
        // TODO: overflow's gonna be a problem
        let femtoseconds = self.main_counter() * self.tick_period;
        Duration::from_nanos(femtoseconds / 1000000)
    }
}

// Needed because `base` is a raw pointer. It's OK to share Hpet across threads because multiple
// threads can concurrently read the counter and mutexes are used to prevent against concurrent
// modification of comparators. It can be sent across threads because that even further restricts
// which threads have access to the device.
unsafe impl Sync for Hpet {}
unsafe impl Send for Hpet {}

/// Representation of the HPET General Capabilities and ID register
pub struct Capabilities(u64);

impl Capabilities {
    /// The revision of the HPET function implemented, must be non-zero
    pub fn revision_id(&self) -> u8 {
        self.0.get_bits(0..8) as u8
    }

    /// The number of timers/counters supported
    pub fn num_timers(&self) -> u8 {
        self.0.get_bits(8..13) as u8 + 1
    }

    /// Can the main counter operate in 64-bit mode?
    pub fn is_64_bit_counter(&self) -> bool {
        self.0.get_bit(13)
    }

    /// True if the HPET supports the "legacy replacement" mapping
    pub fn legacy_replacement_mapping(&self) -> bool {
        self.0.get_bit(15)
    }

    /// The vendor ID, which can be interpreted like a PCI vendor ID
    pub fn vendor_id(&self) -> u16 {
        self.0.get_bits(16..32) as u16
    }

    /// The main counter tick period, in femtoseconds
    pub fn tick_period(&self) -> u32 {
        self.0.get_bits(32..64) as u32
    }

    pub fn log_capabilities(&self) {
        debug!(
            "HPET revision {}, vendor ID {:#x}",
            self.revision_id(),
            self.vendor_id()
        );
        debug!(
            "    - Main counter tick period is {} femtoseconds",
            self.tick_period()
        );
        if self.is_64_bit_counter() {
            debug!("    - Main counter supports 64-bit mode");
        } else {
            debug!("    - Main counter does not support 64-bit mode");
        }
        if self.legacy_replacement_mapping() {
            debug!("    - Supports legacy replacement mode");
        } else {
            debug!("    - Does not support legacy replacement mode");
        }
        debug!("    - Supports {} timers", self.num_timers());
    }
}

static HPET: Once<Hpet> = Once::new();

pub fn init(base_address: PhysAddr) {
    debug!(
        "Found HPET at physical address {:#x}",
        base_address.as_u64()
    );

    HPET.call_once(|| {
        let base = physical_to_virtual(base_address).as_mut_ptr();

        let mut hpet = Hpet::new(base);
        unsafe {
            hpet.enable();
        }
        hpet.set_legacy_replacement(false);

        assert!(hpet.is_enabled(), "Could not enable HPET");

        hpet
    });
}

/// Check if the HPET is supported
pub fn is_supported() -> bool {
    HPET.wait().is_some()
}

/// Forwards to global Hpet instance
pub struct HpetTimer;

impl WallClockTimer for HpetTimer {
    fn current_timestamp(&self) -> Duration {
        HPET.wait()
            .expect("HPET not configured")
            .current_timestamp()
    }
}
impl DelayTimer for HpetTimer {
    fn delay(&self, duration: Duration) {
        let hpet = HPET.wait().expect("HPET not configured");

        // duration in ns * (1000000 femtoseconds / 1ns) * (1 tick / hpet.tick_period femtoseconds)
        let ticks: u64 = (duration.as_nanos() * 1000000 / hpet.tick_period as u128) as u64;

        // TODO: overflow
        let target = hpet.main_counter() + ticks;
        while hpet.main_counter() < target {
            spin_loop();
        }
    }
}
