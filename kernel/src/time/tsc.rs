use core::arch::x86_64::{__rdtscp, _rdtsc};
use core::time::Duration;

use raw_cpuid::CpuId;

use super::WallClockTimer;

/// Time Stamp Counter driver. The TSC is a 64-bit register that counts the number of cycles since
/// reset. It has a somewhat troubled history, but on recent processors, the fixed rate is pretty
/// nice to work with.
///
/// See [Wikipedia](https://en.wikipedia.org/wiki/Time_Stamp_Counter)
pub struct Tsc {
    use_rdtscp: bool,
    frequency: u64,
}

impl Tsc {
    /// Check if the TSC is supported. This implementation only supports newer architectures
    /// with an invariant TSC and the rdtscp instruction.
    pub fn is_supported() -> bool {
        let cpuid = CpuId::new();

        cpuid
            .get_feature_info()
            .map(|f| f.has_tsc())
            .unwrap_or(false)
    }

    /// Check if the TSC is invariant. An invariant TSC runs at a constant rate in ACPI P-, C-, and
    /// T- states. See section 17.17.1 of the Intel manual.
    pub fn is_invariant() -> bool {
        // TODO: invariant TSC seems like something separate from being constant-rate, maybe we don't care as much about it?
        let cpuid = CpuId::new();

        cpuid
            .get_extended_function_info()
            .map(|f| f.has_invariant_tsc())
            .unwrap_or(false)
    }

    /// Create a new Tsc instance.
    /// TODO: this assumes all cores have the same frequency
    /// TODO: I _think_ that all cores start at 0 at the same time (on reset), so we don't need a per-core adjustment/synchronization
    pub fn new() -> Tsc {
        let cpuid = CpuId::new();
        let frequency = cpuid
            .get_tsc_info()
            .expect("EAX_TIME_STAMP_COUNTER_INFO leaf not supported")
            .tsc_frequency();

        let use_rdtscp = cpuid
            .get_extended_function_info()
            .map(|f| f.has_rdtscp())
            .unwrap_or(false);

        Tsc {
            frequency,
            use_rdtscp,
        }
    }

    /// Get the current TSC value
    fn current_count(&self) -> u64 {
        if self.use_rdtscp {
            let mut aux: u32 = 0;
            unsafe { __rdtscp(&mut aux) }
        } else {
            // TODO: cpuid to synchronize?
            unsafe { _rdtsc() }
        }
    }
}

pub struct TscTimer {
    tsc: Tsc, // TODO: make this per-processor
}

impl TscTimer {
    pub fn new() -> TscTimer {
        debug_assert!(Tsc::is_supported());
        TscTimer { tsc: Tsc::new() }
    }
}

impl WallClockTimer for TscTimer {
    fn current_timestamp(&self) -> Duration {
        let ticks = self.tsc.current_count();
        // Multiply ticks by 1e9 to do math in nanoseconds for higher precision
        Duration::from_nanos(ticks * 1000000000 / self.tsc.frequency)
    }
}
