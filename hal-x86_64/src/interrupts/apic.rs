//! APIC support, using x2APIC mode.
use bitvec::prelude::*;
use paste::paste;
use raw_cpuid::CpuId;
use x86_64::registers::model_specific::Msr;

// Using deku would be nice, but it can only serialize into heap-allocated
// vectors, which would negate all the performance gains of x2APIC mode

// Macros for dealing with x2APIC MSRs. This would be nicer as a proc_macro, or
// even better, a proc_macro from crates.io

macro_rules! apic_msr {
    (
        $(#[$meta:meta])*
        $reg:ident @ $addr:literal:
        struct $typ:ident {

        }
    ) => {
        $(#[$meta])*
        static mut $reg: Msr = Msr::new($addr);

        $(#[$meta])*
        struct $typ(BitArray<[u64; 1], Lsb0>);

        impl $typ {
            /// Read the current value of this MSR
            fn read() -> Self {
                // Safety: reading APIC MSRs has no side effects
                $typ(BitArray::new([unsafe { $reg.read() }; 1]))
            }

            /// Update the MSR value
            ///
            /// # Safety
            /// Modifying APIC MSRs can affect interrupt handling and cause faults.
            unsafe fn write(value: &Self) {
                $reg.write(value.0.data[0])
            }
        }
    };
}

macro_rules! msr_field {
    (
        $(#[$meta:meta])*
        $name:ident: $idx:literal
    ) => {
        $(#[$meta])*
        fn $name(&self) -> bool {
            self.0[$idx]
        }

        paste! {
            $(#[$meta])*
            fn [<set_ $name>](&mut self, value: bool) {
                self.0.set($idx, value);
            }
        }
    };
}

apic_msr!(
    /// The IA32_APIC_BASE MSR
    ///
    /// See Intel SDM volume 3A, 10.12.1
    IA32_APIC_BASE_MSR @ 0x01b:
    struct IA32ApicBaseMsr {}
);

impl IA32ApicBaseMsr {
    /// Is the current processor the bootstrap processor?
    fn is_bsp(&self) -> bool {
        self.0[8]
    }

    msr_field!(
        /// The xAPIC global enable/disable flag
        apic_enabled: 11
    );

    msr_field!(
        /// Whether or not x2APIC mode is enabled
        x2apic_enabled: 10
    );
}

apic_msr!(
    /// The Spurious Interrupt Vector Register (SVR)
    ///
    /// See Intel SDM volume 3A, 10.9
    IA32_SVR_MSR @ 0x80f:
    struct IA32SpuriousVectorRegisterMsr {}
);

impl IA32SpuriousVectorRegisterMsr {
    msr_field!(
        /// APIC software enable/disable flag
        enabled: 8
    );

    /// Set the vector number delivered when the local APIC generates a spurious
    /// interrupt.
    fn set_spurious_vector(&mut self, vector: u8) {
        self.0[..8].store(vector);
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

    let mut svr = IA32SpuriousVectorRegisterMsr::read();
    svr.set_enabled(true);
    svr.set_spurious_vector(super::SPURIOUS_INTERRUPT_VECTOR);
    // SAFETY: and yes, we are trying to enable interrupts, which is done via the
    // SVR
    unsafe { IA32SpuriousVectorRegisterMsr::write(&svr) };

    tracing::debug!("Enabled x2APIC mode");
}

/// Checks if the current processor supports x2APIC mode. It's unlikely that
/// this will vary across processors, but is possible.
pub fn supports_x2apic() -> bool {
    let cpuid = CpuId::new();
    cpuid.get_feature_info().map_or(false, |f| f.has_x2apic())
}

// Offsets for remapping PIC interrupts
pub(super) const PIC1_OFFSET: u8 = 32;
pub(super) const PIC2_OFFSET: u8 = 40;

/// Disable the legacy 8259 PIC.
///
/// See the OSDev wiki on [Local APIC configuration](https://wiki.osdev.org/APIC#Local_APIC_configuration)
/// and [PIC initialization](https://wiki.osdev.org/PIC#Initialisation).
pub(super) fn disable_pic() {
    // Since all we're doing is disabling the PIC, we don't need a whole
    // abstraction over it
    use x86_64::structures::port::*;

    // Data and command port numbers
    const PIC1_COMMAND: u16 = 0x20;
    const PIC1_DATA: u16 = 0x21;
    const PIC2_COMMAND: u16 = 0xa0;
    const PIC2_DATA: u16 = 0xa1;

    /// Delay a few microseconds, to give the PIC time to catch up
    /// See the [OSDev wiki](https://wiki.osdev.org/Inline_Assembly/Examples#IO_WAIT)
    unsafe fn io_delay() {
        u8::write_to_port(0x80, 0)
    }

    // Safety: this is the PIC initialization sequence. It's annoying.
    unsafe {
        // Start the initialization sequence in cascade mode
        // First, write a command to say we're doing initialization (0x11)
        // Then, each PIC expects 3 words on the data port:
        // - its vector offset
        // - how it's wired to the other PIC
        // - information about the environment
        u8::write_to_port(PIC1_COMMAND, 0x11);
        io_delay();
        u8::write_to_port(PIC2_COMMAND, 0x11);
        io_delay();
        // Now, write the vector offsets
        u8::write_to_port(PIC1_DATA, PIC1_OFFSET);
        io_delay();
        u8::write_to_port(PIC2_DATA, PIC2_OFFSET);
        io_delay();
        // Tell PIC1 that PIC2 is at IRQ2 (0b00000100)
        u8::write_to_port(PIC1_DATA, 4);
        io_delay();
        // Tell PIC2 its cascade identity (0b00000010)
        u8::write_to_port(PIC2_DATA, 2);
        io_delay();
        // Tell both PICs to use 8086/88 (MCS-80/85) mode
        u8::write_to_port(PIC1_DATA, 0x01);
        io_delay();
        u8::write_to_port(PIC2_DATA, 0x01);
        io_delay();
        // Mask all interrupts
        u8::write_to_port(PIC1_DATA, 0xff);
        io_delay();
        u8::write_to_port(PIC2_DATA, 0xff);
        io_delay();
    }
}
