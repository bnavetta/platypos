use core::{
    hint::spin_loop,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use spin::Once;

use super::{SleepTimer, SchedulerTimer};
use crate::system::apic::configure_apic_timer;

pub const TIMER_FREQUENCY_HZ: usize = 1000;
const NANOS_PER_SECOND: u128 = Duration::SECOND.as_nanos();

pub struct ApicTimer {
    /// Count of how many LAPIC timer interrupts have fired. Note that ticks could be dropped
    /// while interrupts are masked, so this isn't reliable as an absolute measure of time.
    count: AtomicUsize,
}

impl ApicTimer {
    fn new() -> ApicTimer {
        ApicTimer {
            count: AtomicUsize::init(0)
        }
    }

    pub fn tick(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }
}

impl SleepTimer for ApicTimer {
    fn sleep(&self, duration: Duration) {
        // ticks to wait = elapsed time in seconds * frequency
        // math done in nanoseconds for precision, but converted back to ticks since that's all the
        // resolution we have
        let ticks = (duration.as_nanos() * TIMER_FREQUENCY_HZ as u128) / NANOS_PER_SECOND;

        let initial = COUNTER.load(Ordering::SeqCst);
        while ((COUNTER.load(Ordering::SeqCst) - initial) as u128) < ticks {
            spin_loop()
        }
    }
}

impl SchedulerTimer for ApicTimer {
    fn set_time_slice(&mut self, time_slice: Duration) {
        unimplemented!()
    }
}

static APIC_TIMER: Once<ApicTimer> = Once::new();

pub fn init() -> &'static ApicTimer {
    configure_apic_timer(TIMER_FREQUENCY_HZ as u32);
    APIC_TIMER.call_once(|| {
        ApicTimer::new()
    })
}