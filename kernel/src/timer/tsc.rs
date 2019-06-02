use core::arch::x86_64::{__rdtscp, _rdtsc};
use core::time::Duration;

use log::warn;
use raw_cpuid::CpuId;

use super::RealTimeTimer;

/// Time Stamp Counter driver. The TSC is a 64-bit register that counts the number of cycles since
/// reset. It has a somewhat troubled history, but on recent processors, the fixed rate is pretty
/// nice to work with.
///
/// See [Wikipedia](https://en.wikipedia.org/wiki/Time_Stamp_Counter)
pub struct Tsc {
    frequency: u64,
}

impl Tsc {
    /// Check if the TSC is supported. This implementation only supports newer architectures
    /// with an invariant TSC and the rdtscp instruction.
    pub fn is_supported() -> bool {
        let cpuid = CpuId::new();

        let has_tsc = cpuid.get_feature_info().map(|f| f.has_tsc()).unwrap_or(false);
        if !has_tsc {
            warn!("TSC is not supported");
            return false;
        }

        if let Some(function_info) = cpuid.get_extended_function_info() {
            if !function_info.has_rdtscp() {
                warn!("rdtscp instruction is not supported");
                return false;
            }

            if !function_info.has_invariant_tsc() {
                warn!("Invariant TSC is not supported");
                return false;
            }

            return true;
        } else {
            return false;
        }
    }

    /// Create a new Tsc instance.
    /// TODO: this assumes all cores have the same frequency
    /// TODO: I _think_ that all cores start at 0 at the same time (on reset), so we don't need a per-core adjustment/synchronization
    pub fn new() -> Tsc {
        let cpuid = CpuId::new();
        let frequency = cpuid.get_tsc_info().expect("EAX_TIME_STAMP_COUNTER_INFO leaf not supported").tsc_frequency();

        Tsc {
            frequency
        }
    }

    /// Get the current TSC value
    fn current_count(&self) -> u64 {
        let mut aux: u32 = 0;
        unsafe { __rdtscp(&mut aux) }
    }
}

impl RealTimeTimer for Tsc {
    fn current_timestamp(&self) -> Duration {
        let ticks = self.current_count();
        // Multiply ticks by 1e9 to do math in nanoseconds for higher precision
        Duration::from_nanos(ticks * 1000000000 / self.frequency)
    }
}
