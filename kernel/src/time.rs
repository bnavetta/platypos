use alloc::boxed::Box;
use core::time::Duration;
use log::debug;

use crossbeam_utils::atomic::AtomicCell;
use lazy_static::lazy_static;
use spin::Once;

pub mod apic;
pub mod pit;
mod tsc;

// See https://forum.osdev.org/viewtopic.php?f=1&t=29461&start=0 for a discussion of different
// timer sources.

/// A timer usable for scheduler preemption. Ideally, it is inherently interrupt-driven and
/// processor-local. The timer guarantees that it will generate a preemption event at the configured
/// time slice duration, independently on each processor. This trait does not specify what happens
/// on a preemption event because it is expected that implementations will be configured with a
/// callback on initialization.
trait SchedulerTimer {
    /// Configure the time slice for clock interrupts (preemption events).
    fn set_time_slice(&mut self, time_slice: Duration);
}

/// Timer for keeping track of "real time" since startup.
pub trait RealTimeTimer {
    /// Returns the amount of real, or wall-clock, time that has elapsed since this timer was
    /// created.
    fn current_timestamp(&self) -> Duration;
}

/// Timer for delays/sleeps.
/// TODO: this should integrate with the scheduler instead of blocking the current processor
trait SleepTimer {
    /// Block for the specified duration.
    fn sleep(&self, duration: Duration);
}

static REAL_TIME_TIMER: Once<Box<dyn RealTimeTimer + Sync + Send>> = Once::new();

pub fn init() {
    if tsc::Tsc::is_supported() {
        debug!("Using invariant TSC as real-time timer");
        REAL_TIME_TIMER.call_once(|| Box::new(tsc::Tsc::new()));
    }

    self::pit::init();
    crate::system::apic::configure_apic_timer(crate::time::apic::TIMER_FREQUENCY_HZ as u32);
    set_source(crate::time::TimerSource::LocalApicTimer);
}

pub fn real_time_timer() -> &'static dyn RealTimeTimer {
    &**REAL_TIME_TIMER
        .wait()
        .expect("Real-time timer not configured")
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TimerSource {
    ProgrammableIntervalTimer,
    LocalApicTimer,
}

lazy_static! {
    static ref SOURCE: AtomicCell<TimerSource> =
        AtomicCell::new(TimerSource::ProgrammableIntervalTimer);
}

pub fn sleep(duration: Duration) {
    match SOURCE.load() {
        TimerSource::ProgrammableIntervalTimer => pit::pit_sleep(duration),
        TimerSource::LocalApicTimer => apic::apic_sleep(duration),
    }
}

pub fn set_source(source: TimerSource) {
    SOURCE.store(source);
}
