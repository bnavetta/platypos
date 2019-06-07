use x86_64::structures::paging::PhysFrame;

use bit_field::BitField;

/// Destination for an IPI
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Destination {
    /// Send only to the specified APIC ID
    Exact(u32),
    /// Send to the processor which issued the IPI
    Current,
    /// Send to all processors, including the one which issued the IPI
    All,
    /// Send to all processors except the one which issued the IPI
    AllButCurrent,
}

/// Delivery mode of an IPI. This determines the kind of IPI which is sent.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DeliveryMode {
    /// Deliver the interrupt with the specified vector number
    Fixed(u8),

    /// Deliver an SMI interrupt
    SMI,

    /// Deliver an NMI interrupt
    NMI,

    /// Deliver an INIT request, causing the target processors to perform an INIT
    INIT,

    /// Send a synchronization message to *all local APICs in the system*, ignoring the specified
    /// destination. This message tells the APICs to set their arbitration IDs to their APIC IDs
    /// and is not supported/required on all processors.
    INITLevelDeAssert,

    /// Send a startup IPI, telling the target processors to run the startup routine at the given
    /// location.
    Startup(PhysFrame),
}

#[derive(Debug, Copy, Clone)]
pub struct InterprocessorInterrupt {
    mode: DeliveryMode,
    destination: Destination,
}

impl InterprocessorInterrupt {
    pub fn new(mode: DeliveryMode, destination: Destination) -> InterprocessorInterrupt {
        InterprocessorInterrupt { mode, destination }
    }

    pub fn encode(&self) -> u64 {
        let mut value = 0u64;

        match self.mode {
            DeliveryMode::Fixed(vector) => {
                value.set_bits(0..8, vector as u64);
                // mode is already 0b000
            }
            DeliveryMode::SMI => {
                // vector is already 0
                value.set_bits(8..11, 0b010);
            }
            DeliveryMode::NMI => {
                // vector is already 0
                value.set_bits(8..11, 0b100);
            }
            DeliveryMode::INIT => {
                // vector is already 0
                value.set_bits(8..11, 0b101);
            }
            DeliveryMode::INITLevelDeAssert => {
                // vector is already 0
                assert_eq!(
                    self.destination,
                    Destination::All,
                    "Destination should be \"all including self\" for an INIT Level De-Assert"
                );
                value.set_bits(8..11, 0b101);
            }
            DeliveryMode::Startup(code) => {
                let page = code.start_address().as_u64() / 4096u64;
                assert!(
                    page <= u8::max_value() as u64,
                    "SIPI page {:#x} is out of bounds",
                    page
                );
                value.set_bits(0..11, page);
            }
        };

        // The level (bit 14) must be 1 unless performing an INIT level de-assert
        // The Intel manual says that for an INIT level de-assert, the trigger mode bit can be either
        // edge (0) or level (1), but the OSDev wiki says to always use level.
        if self.mode == DeliveryMode::INITLevelDeAssert {
            value.set_bits(14..16, 0b01);
        } else {
            value.set_bits(14..16, 0b10);
        }

        // bit 11 is the destination mode. We only support 0 (logical)

        match self.destination {
            Destination::Exact(id) => {
                // destination shorthand is already 0b00
                value.set_bits(56..64, id as u64);
            }
            Destination::Current => {
                value.set_bits(18..20, 0b01);
            }
            Destination::All => {
                value.set_bits(18..20, 0b10);
            }
            Destination::AllButCurrent => {
                value.set_bits(18..20, 0b11);
            }
        }

        value
    }
}
