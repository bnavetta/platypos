use core::time::Duration;

use crossbeam_utils::atomic::AtomicCell;
use lazy_static::lazy_static;

pub mod apic;
pub mod pit;

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
