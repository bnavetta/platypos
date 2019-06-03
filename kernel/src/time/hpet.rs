use core::time::Duration;

use bit_field::BitField;
use log::debug;
use spin::Once;
use x86_64::{PhysAddr, VirtAddr};
use x86_64::structures::paging::{Page, PhysFrame, Mapper, PageTableFlags};

use crate::kernel_state;
use super::WallClockTimer;

const MAIN_COUNTER_VALUE_REGISTER: usize = 0x0F0;

/// Driver for the High Precision Event Timer
pub struct Hpet {
    base: *mut u64,
    tick_period: u64, // tick period of the main counter in femtoseconds
    // TODO: will need a mutex for protecting against timer/comparator modifications
}

impl Hpet {
    fn new(base: *mut u64) -> Hpet {
        // Get the tick period out of the general capabilities and ID register
        let register = unsafe { base.read_volatile() };
        let tick_period = register.get_bits(32..64);
        let vendor_id = register.get_bits(16..32);
        let revision = register.get_bits(0..8);

        debug!("HPET revision {}, vendor ID {:#x}", revision, vendor_id);

        Hpet {
            base,
            tick_period
        }
    }

    /// Read a HPET register
    ///
    /// # Unsafety
    /// The offset must be a valid register offset
    unsafe fn read(&self, offset: usize) -> u64 {
        self.base.add(offset).read_volatile()
    }

    pub fn main_counter(&self) -> u64 {
        unsafe { self.read(MAIN_COUNTER_VALUE_REGISTER) }
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

const HPET_ADDRESS: u64 = 0xfffffa0000040000;
static HPET: Once<Hpet> = Once::new();

pub fn init(base_address: PhysAddr) {
    debug!("Found HPET at physical address {:#x}", base_address.as_u64());

    HPET.call_once(|| {
        let virtual_start = VirtAddr::new(HPET_ADDRESS);
        let page = Page::from_start_address(virtual_start).expect("HPET virtual start address is not page-aligned");
        let frame = PhysFrame::containing_address(base_address);

        kernel_state().with_page_table(|table| {
            let mut allocator = kernel_state().frame_allocator().page_table_allocator();
            unsafe { table.active_4kib_mapper().map_to(page, frame, PageTableFlags::PRESENT | PageTableFlags::WRITABLE, &mut allocator).expect("Failed to map HPET").flush() };
        });

        // The HPET start address isn't necessarily page-aligned, so we might need to offset it within the mapping
        let base = unsafe { virtual_start.as_mut_ptr::<u64>().add((base_address.as_u64() - frame.start_address().as_u64()) as usize) };

        Hpet::new(base)
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
        HPET.wait().expect("HPET not configured").current_timestamp()
    }
}