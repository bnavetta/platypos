//! APIC support, using x2APIC mode.
use bitvec::prelude::*;
use raw_cpuid::CpuId;
use x86_64::registers::model_specific::Msr;

// Using deku would be nice, but it can only serialize into heap-allocated
// vectors, which would negate all the performance gains of x2APIC mode

macro_rules! apic_msr {
    (
        $(#[$meta:meta])*
        $typ:ident => $reg:ident = $addr:literal
    ) => {
        static mut $reg: Msr = Msr::new($addr);

        $(#[$meta])*
        struct $typ(BitArray<[u64; 1], Lsb0>);

        impl $typ {
            fn read() -> Self {
                // Safety: reading APIC MSRs has no side effects
                $typ(BitArray::new([unsafe { $reg.read() }; 1]))
            }

            unsafe fn write(value: &Self) {
                $reg.write(value.0.data[0])
            }
        }
    };
}

// static mut IA32_APIC_BASE_MSR: Msr = Msr::new(0x01b);

apic_msr!(
/// The IA32_APIC_BASE MSR
///
/// See Intel SDM volume 3A, 10.12.1
IA32ApicBaseMsr => IA32_APIC_BASE_MSR = 0x01b
);

impl IA32ApicBaseMsr {
    /// Is the current processor the bootstrap processor?
    fn is_bsp(&self) -> bool {
        self.0[8]
    }

    /// The xAPIC global enable/disable flag
    fn apic_enabled(&self) -> bool {
        self.0[11]
    }

    /// Set the XAPIC global enable/disable flag
    fn set_apic_enabled(&mut self, enabled: bool) {
        self.0.set(11, enabled);
    }

    /// Whether or not x2APIC mode is enabled
    fn x2apic_enabled(&self) -> bool {
        self.0[10]
    }

    /// Set whether or not x2APIC mode is enabled
    fn set_x2apic_enabled(&mut self, enabled: bool) {
        self.0.set(10, enabled);
    }
}

/// Initialize the local APIC on this core
#[tracing::instrument(level = "debug")]
pub fn init_local() {
    if !supports_x2apic() {
        panic!("Processor does not support x2APIC mode!");
    }

    let mut base = IA32ApicBaseMsr::read();
    tracing::debug!(
        "Initial xAPIC state: {} BSP, xAPIC {}, x2APIC {}",
        if base.is_bsp() { "is" } else { "is not" },
        if base.apic_enabled() {
            "enabled"
        } else {
            "disabled"
        },
        if base.x2apic_enabled() {
            "enabled"
        } else {
            "disabled"
        }
    );

    base.set_apic_enabled(true);
    base.set_x2apic_enabled(true);
    // SAFETY: yes, we do actually want to enable x2APIC mode
    // Per table 10-5 of Intel SDM volume 3, this is a valid configuration
    unsafe { IA32ApicBaseMsr::write(&base) };
    tracing::debug!("Enabled x2APIC mode");
}

/// Checks if the current processor supports x2APIC mode. It's unlikely that
/// this will vary across processors, but is possible.
pub fn supports_x2apic() -> bool {
    let cpuid = CpuId::new();
    cpuid.get_feature_info().map_or(false, |f| f.has_x2apic())
}
