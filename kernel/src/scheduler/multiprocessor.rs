//! Multiprocessor setup and support. This lives in the scheduler module tree because it's the part
//! of the OS that cares the most about it. Basically, application processors are started up and
//! told to run the scheduler loop, and the rest of the OS hopefully doesn't notice that it's running
//! on multiple cores now :)
use core::ptr;
use core::time::Duration;

use apic::ipi::{DeliveryMode, Destination, InterprocessorInterrupt};
use log::{debug, error, trace};
use volatile::Volatile;
use x86_64::structures::paging::PhysFrame;
use x86_64::{PhysAddr, VirtAddr};

use crate::kernel_state;
use crate::system::apic::with_local_apic;
use crate::time::delay;
use crate::topology::processor::{processor_topology, Processor, ProcessorState};
use crate::util::spin_on;

// See https://wiki.osdev.org/Memory_Map_(x86). 0x00000500-0x00007BFF is guaranteed to not be used
// by the BIOS and System Management. However, the bootloader, like us, takes advantage of this and
// puts the kernel page table between 0x1000 and 0x5000. This means we have to replace the initial
// page table with our own copy before we can use this region for MP initialization.

/// Address of the shared data region for the AP trampoline to communicate with the BSP
const TRAMPOLINE_DATA_START: usize = 0x1000;

/// Address where the trampoline code is copied
const TRAMPOLINE_CODE_START: usize = 0x2000;

/// Representation of the trampoline data region
#[repr(C)]
struct TrampolineData {
    startup_flag: Volatile<u16>,
}

impl TrampolineData {
    /// Get a pointer to the global trampoline data region.
    ///
    /// # Unsafety
    /// The caller must ensure that there are not other references to the data region (except the
    /// processor being started, of course)
    unsafe fn new(addr: VirtAddr) -> &'static mut TrampolineData {
        addr.as_mut_ptr::<TrampolineData>().as_mut().unwrap()
    }

    pub fn clear_startup_flag(&mut self) {
        self.startup_flag.write(0);
    }

    pub fn startup_flag(&self) -> u16 {
        self.startup_flag.read()
    }
}

global_asm!(
    r"#
    .global mp_processor_init
    .code16
mp_processor_init:
    cli
    movw $1, 0x2000
    hlt
MP_PROCESSOR_INIT_END: .byte 0
#"
);

extern "C" {
    fn mp_processor_init() -> ();
    static MP_PROCESSOR_INIT_END: u8;
}

/// Start an application processor.
///
/// Follows [Brendan's method from the OSDev wiki](https://wiki.osdev.org/Symmetric_Multiprocessing#AP_startup).
fn start_processor(
    trampoline_data: &mut TrampolineData,
    processor: &Processor,
) {
    debug!("Attempting to start processor {}", processor.id());
    processor.mark_state_transition(ProcessorState::Starting);
    trampoline_data.clear_startup_flag();

    with_local_apic(|apic| {
        // Send the INIT IPI and de-assert
        unsafe {
            trace!("Sending INIT IPI to APIC {:#x}", processor.apic_id());
            apic.send_ipi(
                InterprocessorInterrupt::new(
                    DeliveryMode::INIT,
                    Destination::Exact(processor.apic_id()),
                ),
                true,
            );
        }
        delay(Duration::from_millis(10));

        let sipi = InterprocessorInterrupt::new(
            DeliveryMode::Startup(PhysFrame::from_start_address(PhysAddr::new(TRAMPOLINE_CODE_START as u64)).unwrap()),
            Destination::Exact(processor.apic_id()),
        );

        trace!("Sending SIPI to APIC {:#x}", processor.apic_id());
        unsafe {
            apic.send_ipi(sipi, true);
        }

        if spin_on(|| trampoline_data.startup_flag() != 0, Duration::from_millis(1)) {
            debug!("Started processor {}", processor.id());
            return;
        }

        trace!("Processor did not start in time, re-sending SIPI");
        unsafe {
            apic.send_ipi(sipi, true);
        }
    });

    if spin_on(|| trampoline_data.startup_flag() != 0, Duration::from_secs(1)) {
        debug!("Started processor {}", processor.id());
    } else {
        error!(
            "Could not start processor {} (APIC ID {:#x})",
            processor.id(),
            processor.apic_id()
        );
        processor.mark_state_transition(ProcessorState::Failed);
    }

    // The processor will mark itself as running once it's in long mode
}

/// Attempts to boot all processors in the uninitialized state
pub fn boot_application_processors() {
    let (trampoline_addr, trampoline_size) = unsafe {
        let init_addr = mp_processor_init as usize;
        let end_addr = &MP_PROCESSOR_INIT_END as *const u8 as usize;
        (init_addr, end_addr - init_addr)
    };

    debug!(
        "Found trampoline at {:#x} ({} bytes long)",
        trampoline_addr, trampoline_size
    );

    let (code_addr, data_addr) = kernel_state().with_page_table(|pt| {
        let code_start = PhysAddr::new(TRAMPOLINE_CODE_START as u64);
        let data_start = PhysAddr::new(TRAMPOLINE_DATA_START as u64);
        (pt.physical_map_address(code_start), pt.physical_map_address(data_start))
    });

    unsafe {
        ptr::copy_nonoverlapping(
            trampoline_addr as *const u8,
            code_addr.as_mut_ptr(),
            trampoline_size,
        );
    }
    debug!("Installed trampoline at {:#x}", TRAMPOLINE_CODE_START);

    // Safe because only boot_application_processors uses the trampoline data
    let trampoline_data = unsafe { TrampolineData::new(data_addr) };

    for processor in processor_topology().processors() {
        if processor.state() == ProcessorState::Uninitialized {
            start_processor(trampoline_data, processor);
        }
    }
}
