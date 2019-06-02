use core::{
    hint::spin_loop,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

pub const TIMER_FREQUENCY_HZ: usize = 1000;
const NANOS_PER_SECOND: u128 = Duration::SECOND.as_nanos();

/// Counter of how many LAPIC timer interrupts have fired. Note that ticks could be dropped
/// while interrupts are masked, so this isn't reliable as an absolute measure of time.
static COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn apic_timer_callback() {
    COUNTER.fetch_add(1, Ordering::SeqCst);
}

/// Sleep for `duration` using the LAPIC timer. This assumes the timer has been configured to fire
/// at `TIMER_FREQUENCY_HZ`.
pub fn apic_sleep(duration: Duration) {
    // ticks to wait = elapsed time in seconds * frequency
    // math done in nanoseconds for precision, but converted back to ticks since that's all the
    // resolution we have
    let ticks = (duration.as_nanos() * TIMER_FREQUENCY_HZ as u128) / NANOS_PER_SECOND;

    let initial = COUNTER.load(Ordering::SeqCst);
    while ((COUNTER.load(Ordering::SeqCst) - initial) as u128) < ticks {
        spin_loop()
    }
}
