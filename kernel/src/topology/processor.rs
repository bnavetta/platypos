//! Representation of discovered CPU topology. At the moment, this is just a flat list of logical
//! CPUs. At some point, it would be nice to get into [Detecting CPU Topology](https://wiki.osdev.org/Detecting_CPU_Topology_(80x86)).
//! A lot of the OS also assumes that all cores are fairly similar anyways, so it's probably not
//! worth caring about topology until that won't break things.
//!
//! As part of this topology, each logical CPU is assigned a "logical ID". Logical IDs are
//! contiguous and sequential. This makes indexing for processor-specific data much easier, since
//! it corresponds to array indices.

use alloc::vec::Vec;

use acpi;
use crossbeam_utils::atomic::AtomicCell;
use hashbrown::HashMap;
use log::info;
use spin::Once;

/// Description of a logical CPU.
///
/// In general, a system consists of one or more NUMA domains. Each NUMA domain contains one or more
/// chips, each chip contains one or more cores, and each core contains one or more logical CPUs
/// (if there is hyperthreading). For the purposes of being able to schedule things, use local
/// APICs, and so on, we only really care about the logical CPUs.
#[derive(Debug)]
pub struct Processor {
    logical_id: u32,
    apic_id: u32,
    processor_id: u32,
    is_bootstrap_processor: bool,
    state: AtomicCell<ProcessorState>,
}

impl Processor {
    fn new(logical_id: u32, processor: &acpi::Processor) -> Processor {
        Processor {
            logical_id,
            apic_id: processor.local_apic_id as u32,
            processor_id: processor.processor_uid as u32,
            is_bootstrap_processor: !processor.is_ap,
            state: AtomicCell::new(match processor.state {
                acpi::ProcessorState::Running => {
                    assert!(
                        !processor.is_ap,
                        "Only the bootstrap processor should be running"
                    );
                    ProcessorState::Running
                }
                acpi::ProcessorState::WaitingForSipi => ProcessorState::Uninitialized,
                acpi::ProcessorState::Disabled => ProcessorState::Disabled,
            }),
        }
    }

    /// The logical ID of this processor, used by the OS.
    pub fn id(&self) -> u32 {
        self.logical_id
    }

    /// The ID of the local APIC associated with this processor, used for sending IPIs and configuring
    /// interrupt handling.
    pub fn apic_id(&self) -> u32 {
        self.apic_id
    }

    /// True if this processor is the bootstrap processor
    pub fn is_bootstrap_processor(&self) -> bool {
        self.is_bootstrap_processor
    }

    /// Gets this processor's current state
    pub fn state(&self) -> ProcessorState {
        self.state.load()
    }

    /// Update the flag recording this processor's current state. Fails if the transition is invalid
    /// given the current recorded processor state.
    ///
    /// # Allowed transitions
    /// * `Uninitialized` -> `Starting`
    /// * `Starting` -> `Running`
    /// * `Starting` -> `Failed`
    pub fn mark_state_transition(&self, to: ProcessorState) -> bool {
        let prev = match to {
            // Cannot transition into the uninitialized state
            ProcessorState::Uninitialized => return false,
            // Cannot transition into the disabled state
            ProcessorState::Disabled => return false,
            ProcessorState::Starting => ProcessorState::Uninitialized,
            ProcessorState::Running => ProcessorState::Starting,
            ProcessorState::Failed => ProcessorState::Starting,
        };

        self.state.compare_and_swap(prev, to) == prev
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ProcessorState {
    /// The OS has not started this processor
    Uninitialized,

    /// The processor is starting up
    Starting,

    /// The processor has been initialized by the OS and is fully running.
    Running,

    /// The OS could not start this processor
    Failed,

    /// This processor was marked as unusable
    Disabled,
}

/// Representation of discovered system CPU topology.
pub struct ProcessorTopology {
    /// Mapping from APIC IDs to logical IDs.
    apic_to_logical: HashMap<u32, u32>,
    /// Processor information, indexed by logical ID
    processors: Vec<Processor>,
}

impl ProcessorTopology {
    /// Translate a local APIC ID to the corresponding logical processor ID
    pub fn logical_id(&self, apic_id: u32) -> Option<u32> {
        self.apic_to_logical.get(&apic_id).cloned()
    }

    /// List known processors on the system
    pub fn processors(&self) -> &Vec<Processor> {
        &self.processors
    }
}

static TOPOLOGY: Once<ProcessorTopology> = Once::new();

pub fn init(bsp: &Option<acpi::Processor>, aps: &Vec<acpi::Processor>) {
    let mut processors = Vec::with_capacity(1 + aps.len());
    let mut apic_to_logical = HashMap::with_capacity(1 + aps.len());

    let mut logical_id = 0;
    if let Some(bsp) = bsp {
        // TODO: does the ACPI library not support 32-bit APIC IDs?
        processors.push(Processor::new(logical_id, bsp));
        apic_to_logical.insert(bsp.local_apic_id as u32, logical_id);
        logical_id += 1
    }

    for ap in aps.iter() {
        processors.push(Processor::new(logical_id, ap));
        apic_to_logical.insert(ap.local_apic_id as u32, logical_id);
        logical_id += 1
    }

    info!("System supports {} logical processors:", processors.len());
    for processor in processors.iter() {
        info!("    - {:?}", processor);
    }

    TOPOLOGY.call_once(|| ProcessorTopology {
        processors,
        apic_to_logical,
    });
}

pub fn processor_topology() -> &'static ProcessorTopology {
    TOPOLOGY.wait().expect("Processor topology not initialized")
}
