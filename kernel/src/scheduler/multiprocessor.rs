//! Multiprocessor setup and support. This lives in the scheduler module tree because it's the part
//! of the OS that cares the most about it. Basically, application processors are started up and
//! told to run the scheduler loop, and the rest of the OS hopefully doesn't notice that it's running
//! on multiple cores now :)
use core::ptr;
use core::time::Duration;

use apic::ipi::{DeliveryMode, Destination, InterprocessorInterrupt};
use log::{debug, error, trace, info};
use volatile::Volatile;
use x86_64::structures::paging::{Page, PhysFrame, PageTableFlags, Mapper};
use x86_64::{PhysAddr, VirtAddr};

use crate::kernel_state;
use crate::system::apic::with_local_apic;
use crate::time::delay;
use crate::topology::processor::{processor_topology, Processor, ProcessorState};
use crate::util::spin_on;
use core::convert::TryInto;

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
    /// Set by the application processor once it starts up
    startup_flag: Volatile<u32>,

    /// Location of the page table for the application processor to use
    pml4: Volatile<u32>,

    /// Location of the stack for the application processor to use
    stack: Volatile<usize>,

    /// Location of the entry point for the application processor to jump to
    entry: Volatile<usize>,
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

    fn clear_startup_flag(&mut self) {
        self.startup_flag.write(0);
    }

    fn startup_flag(&self) -> u32 {
        self.startup_flag.read()
    }

    /// Set the page tables for booted APs to use
    fn set_pml4(&mut self, addr: PhysAddr) {
        self.pml4.write(addr.as_u64().try_into().expect("PML4 must be in first 4GiB of RAM to be accessible in trampoline"));
    }

    fn set_stack(&mut self, addr: VirtAddr) {
        self.stack.write(addr.as_u64() as usize)
    }

    fn set_entry_function(&mut self, addr: VirtAddr) {
        self.entry.write(addr.as_u64() as usize);
    }
}

global_asm!(
    r"#
    .global mp_processor_init

.code16
.align 4096
mp_processor_init:
    cli

    # Enable PAE and the global page feature (PGE bit)
    # The Intel manual says that paging must be enabled before setting the PGE bit, but the OSDev
    # wiki seems to think otherwise
    movl $0xa0, %eax
    movl %eax, %cr4

    # Set the PML4
    movl 0x1004, %edx
    movl %edx, %cr3

    # Read the EFER MSR
    movl $0xC0000080, %ecx
    rdmsr

    # Set the long mode enable bit
    orl $0x00000100, %eax
    wrmsr

    # Activate long mode
    # This enables paging and protection simultaneously
    movl %cr0, %ebx
    orl $0x80000001, %ebx
    movl %ebx, %cr0

    # Create a temporary GDT so we can jump into Rust
    # This assumes the long-mode trampoline code is at most 8 KiB

    # Null descriptor
    movl $0, 0x4000
    movl $0, 0x4004

    # Code descriptor (exec/read)
    movl $0, 0x4008
    movl $0x209a00, 0x400c

    # Data descriptor (read/write)
    movl $0, 0x4010
    movl $0x9200, 0x4014

    # Create GDT pointer structure
    movw $24, 0x4020 # Size (limit) in GDT
    movl $0x4000, 0x4022 # Pointer
    lgdt 0x4020

    # Far jump into the second part of the trampoline, so we can start using 64-bit instructions
    jmpl $0x0008,$0x3000

    hlt

.align 4096
.code64
mp_processor_long_init:
    movl $1, 0x1000

    movq 0x1008, %rsp

    movq 0x1010, %rax
    pushq %rax
    retq

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

    let stack = kernel_state().frame_allocator().allocate_pages(4).expect("Could not allocate processor stack");
    trampoline_data.set_stack(stack.start_address() + 4 * 4096u64);

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
    });

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

    let (code_addr, data_addr, pml4) = kernel_state().with_page_table(|pt| {
        let code_start = PhysAddr::new(TRAMPOLINE_CODE_START as u64);
        let data_start = PhysAddr::new(TRAMPOLINE_DATA_START as u64);

        // Identity-map the trampoline so it still is accessible once paging is enabled
        unsafe {
            let mut mapper = pt.active_4kib_mapper();
            let mut allocator = kernel_state().frame_allocator().page_table_allocator();

            // Map 4 pages: data at 0x1000, code at 0x2000 and 0x3000, and GDT (created by trampoline) at 0x4000
            let page_start = Page::from_start_address(VirtAddr::new(TRAMPOLINE_DATA_START as u64)).unwrap();
            let frame_start = PhysFrame::from_start_address(data_start).unwrap();
            for i in 0..4 {
                mapper.map_to(page_start + i, frame_start + i, PageTableFlags::PRESENT | PageTableFlags::WRITABLE, &mut allocator).unwrap().flush();
            }
        }

        (pt.physical_map_address(code_start), pt.physical_map_address(data_start), pt.current_pml4_location())
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
    trampoline_data.set_pml4(pml4);
    trampoline_data.set_entry_function(VirtAddr::new(ap_entry as usize as u64));

    for processor in processor_topology().processors() {
        if processor.state() == ProcessorState::Uninitialized {
            start_processor(trampoline_data, processor);
        }
    }
}

static FOO: usize = 1;

pub unsafe extern "C" fn ap_entry() -> ! {
    assert_eq!(FOO, 1);
    crate::system::gdt::install();
//    crate::interrupts::install();
//
//    info!("Hello, World!");
    crate::util::hlt_loop();
}