use alloc::boxed::Box;
use core::time::Duration;

use log::debug;
use spin::Once;

pub mod hpet;
pub mod pit;
mod tsc;

// See https://forum.osdev.org/viewtopic.php?f=1&t=29461&start=0 for a discussion of different
// timer sources.

/// Timer for keeping track of elapsed time since startup.
trait WallClockTimer: Send + Sync {
    /// Returns the amount of real, or wall-clock, time that has elapsed since system startup.
    fn current_timestamp(&self) -> Duration;
}

/// Timer for sleeps.
trait SleepTimer {
    /// Block for the specified duration. This may use the scheduler to yield, depending on the
    /// implementation.
    fn sleep(&self, duration: Duration);
}

/// Timer for delays. The difference between a delay and a sleep is that a delay does not require
/// interrupts or scheduler support.
trait DelayTimer: Send + Sync {
    /// Block for the specified duration without yielding to the scheduler.
    fn delay(&self, duration: Duration);
}

static WALL_CLOCK: Once<Box<dyn WallClockTimer>> = Once::new();
static DELAY: Once<Box<dyn DelayTimer>> = Once::new();

pub fn init() {
    pit::init();

    // TODO: have TscTimer keep track of current count. Then, can get time from RTC at init and
    // add the WallClockTimer duration to that to get an actual timestamp

    //    if tsc::Tsc::is_supported() {
    //        debug!("Using TSC for wall-clock timer");
    //        WALL_CLOCK.call_once(|| Box::new(tsc::TscTimer::new()));
    //    }

    if hpet::is_supported() {
        WALL_CLOCK.call_once(|| Box::new(hpet::HpetTimer));
        DELAY.call_once(|| Box::new(hpet::HpetTimer));
    }
}

pub fn current_timestamp() -> Duration {
    WALL_CLOCK
        .wait()
        .expect("No wall-clock timer configured")
        .current_timestamp()
}

pub fn delay(duration: Duration) {
    DELAY
        .wait()
        .expect("No delay timer configured")
        .delay(duration);
}
