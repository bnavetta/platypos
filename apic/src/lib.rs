#![no_std]

#[macro_use] extern crate alloc;

use core::ptr;
use alloc::vec::Vec;

use bit_field::{BitField, BitArray};
use log::debug;
use raw_cpuid::CpuId;
use spin::Mutex;
use x86_64::instructions::interrupts::without_interrupts;
use x86_64::registers::model_specific::Msr;
use x86_64::PhysAddr;

mod spurious_interrupt;
mod timer;
mod x2apic;
mod xapic;

pub use crate::spurious_interrupt::SpuriousInterruptVectorRegister;
pub use crate::timer::{DivideConfiguration, TimerVectorTable, TimerMode};
use crate::x2apic::X2Apic;
use crate::xapic::XApic;

// https://wiki.osdev.org/APIC

const IA32_APIC_BASE_MSR: Msr = Msr::new(0x1B);

/// Enumeration of APIC operating modes.
pub enum ApicMode {
    XApic,
    X2Apic,
}

/// Advanced Programmable Interrupt Controller driver.
pub struct Apic {
    use_x2apic: bool,
    mmio_base: *mut u32,
    in_use: Mutex<Vec<u64>>, // keeps track of which local APICs are being modified
}

impl Apic {
    /// Checks if the processor supports an APIC.
    pub fn is_supported() -> bool {
        let cpuid = CpuId::new();
        if let Some(feature_info) = cpuid.get_feature_info() {
            feature_info.has_apic()
        } else {
            false
        }
    }

    /// Checks if the local APIC is enabled.
    pub fn is_enabled() -> bool {
        unsafe { IA32_APIC_BASE_MSR.read() }.get_bit(11)
    }

    /// Hardware-enable the local APIC. Note that on some older architectures, the local APIC cannot
    /// be reenabled if it is hardware-disabled.
    pub unsafe fn hardware_enable() {
        let mut base_msr = IA32_APIC_BASE_MSR.read();
        base_msr.set_bit(11, true);
        IA32_APIC_BASE_MSR.write(base_msr);
    }

    /// Get the current operating mode of the local APIC.
    pub fn operating_mode() -> ApicMode {
        let x2apic_enabled = unsafe { IA32_APIC_BASE_MSR.read() }.get_bit(10);
        if x2apic_enabled {
            ApicMode::X2Apic
        } else {
            ApicMode::XApic
        }
    }

    /// Switch the local APIC's operating mode.
    ///
    /// # Unsafety
    ///
    /// Changing APIC modes affects how the APIC is accessed and can (I think) cause interrupts.
    pub unsafe fn set_operating_mode(mode: ApicMode) {
        let mut base_msr = IA32_APIC_BASE_MSR.read();

        // TODO: worth checking if already in the desired mode?
        match mode {
            ApicMode::XApic => base_msr.set_bit(10, false),
            ApicMode::X2Apic => base_msr.set_bit(10, true),
        };

        IA32_APIC_BASE_MSR.write(base_msr);
    }

    /// Checks if the _current_ processor is the bootstrap processor. The bootstrap processor is
    /// the first processor started up, which the OS initially runs on.
    pub fn is_bootstrap_processor() -> bool {
        unsafe { IA32_APIC_BASE_MSR.read() }.get_bit(8)
    }

    /// Create a new `Apic` for accessing the local APIC. This will check if the x2APIC is supported.
    /// If not, the provided `mapper` callback is used to map the xAPIC MMIO registers into the
    /// kernel address space. The returned `Apic` instance can be shared across cores, but the
    /// local APIC handles it provides cannot, and will always refer to the local APIC for the current
    /// core.
    ///
    /// # Arguments
    /// - `max_apic_id` - the highest APIC ID on the system
    /// - `mapper` - a callback for mapping MMIO registers into memory
    ///
    /// # Panics
    /// If the local APIC is not supported at all.
    pub fn new<F>(max_apic_id: usize, mapper: F) -> Apic
    where
        F: FnOnce(PhysAddr) -> *mut u32,
    {
        assert!(Apic::is_supported());

        let in_use = Mutex::new(vec![0u64; (max_apic_id + u64::BIT_LENGTH) / u64::BIT_LENGTH]);

        if X2Apic::is_supported() {
            debug!("Using x2APIC");
            Apic {
                use_x2apic: true,
                mmio_base: ptr::null_mut(),
                in_use
            }
        } else {
            // LAPIC base is page-aligned, and the MSR doesn't just contain the address
            let phys_base = PhysAddr::new(unsafe { IA32_APIC_BASE_MSR.read() }).align_down(4096u64);
            let mapped_base = mapper(phys_base);
            debug!(
                "Using xAPIC at physical address {:#x}, mapped to {:#x}",
                phys_base.as_u64(),
                mapped_base as usize
            );
            Apic {
                use_x2apic: false,
                mmio_base: mapped_base,
                in_use
            }
        }
    }

    /// Initializes the local APIC for the current processor. This will switch to the APIC mode
    /// being used, put it in a well-known state with all local interrupts masked, and enable it.
    pub unsafe fn init(&self, spurious_interrupt_vector: u8) {
        Apic::hardware_enable(); // I _think_ this has to be done first?
        // Need to be in the right mode to actually configure
        if self.use_x2apic {
            Apic::set_operating_mode(ApicMode::X2Apic);
        } else {
            Apic::set_operating_mode(ApicMode::XApic);
        }

        let mut lapic = self.local_apic();

        lapic.mask_all_interrupts();
        lapic.set_spurious_vector(spurious_interrupt_vector);
        lapic.software_enable();
    }

    pub fn local_apic(&self) -> LocalApic {
        let wrapper = if self.use_x2apic {
            ApicWrapper::X2Apic(X2Apic::new())
        } else {
            ApicWrapper::XApic(unsafe { XApic::new(self.mmio_base) })
        };

        let mut local_apic = LocalApic { parent: self, inner: wrapper };

        let local_apic_id = local_apic.id();
        assert!(!self.is_local_apic_used(local_apic_id));
        self.mark_local_apic_used(local_apic_id, true);

        local_apic
    }

    fn mark_local_apic_used(&self, apic_id: u32, used: bool) {
        let mut in_use = self.in_use.lock();
        in_use.set_bit(apic_id as usize, used);
    }

    fn is_local_apic_used(&self, apic_id: u32) -> bool {
        let in_use = self.in_use.lock();
        in_use.get_bit(apic_id as usize)
    }
}

/// Apic isn't already sync because it has a pointer to the MMIO registers for xAPIC mode. However,
/// it's safe for it to be sync because they only point to the current processor's local APIC
/// registers. The guarantee that there's only one local APIC handle per processor at a time prevents
/// aliasing.
unsafe impl core::marker::Sync for Apic {}
unsafe impl core::marker::Send for Apic {}

/// Holds the local APIC implementation
enum ApicWrapper {
    XApic(XApic),
    X2Apic(X2Apic)
}

/// A handle to the local APIC corresponding to the current processor. On each processor, there can
/// be only one reference to the local APIC at a time. When this handle is dropped, then the local
/// APIC is made available again.
pub struct LocalApic<'a> {
    parent: &'a Apic,
    inner: ApicWrapper,
}

// doesn't implement ApicImplementation, this lets us control which parts are exposed
impl <'a> LocalApic <'a> {
    /// Returns the local APIC's ID. The APIC ID is only 32 bits in x2APIC mode. In xAPIC mode, only
    /// the lower 8 bits are used.
    pub fn id(&mut self) -> u32 {
        match self.inner {
            ApicWrapper::XApic(ref mut apic) => apic.local_apic_id(),
            ApicWrapper::X2Apic(ref mut apic) => apic.local_apic_id()
        }
    }

    /// Returns the local APIC version. This is 8 bits in both x2APIC mode and xAPIC mode.
    pub fn version(&mut self) -> u8 {
        match self.inner {
            ApicWrapper::XApic(ref mut apic) => apic.local_apic_version(),
            ApicWrapper::X2Apic(ref mut apic) => apic.local_apic_version(),
        }
    }

    /// Signal that an interrupt has been processed
    pub fn end_of_interrupt(&mut self) {
        match self.inner {
            ApicWrapper::XApic(ref mut apic) => apic.end_of_interrupt(),
            ApicWrapper::X2Apic(ref mut apic) => apic.end_of_interrupt(),
        }
    }

    /// Get the initial count for the local APIC timer.
    pub fn timer_initial_count(&mut self) -> u32 {
        match self.inner {
            ApicWrapper::XApic(ref mut apic) => apic.timer_initial_count(),
            ApicWrapper::X2Apic(ref mut apic) => apic.timer_initial_count(),
        }
    }

    /// Set the initial count for the local APIC timer.
    pub fn set_timer_initial_count(&mut self, count: u32) {
        match self.inner {
            ApicWrapper::XApic(ref mut apic) => apic.set_timer_initial_count(count),
            ApicWrapper::X2Apic(ref mut apic) => apic.set_timer_initial_count(count),
        }
    }

    /// Get the timer's current count
    pub fn timer_current_count(&mut self) -> u32 {
        match self.inner {
            ApicWrapper::XApic(ref mut apic) => apic.timer_current_count(),
            ApicWrapper::X2Apic(ref mut apic) => apic.timer_current_count(),
        }
    }

    /// Get the local vector table for the APIC timer. This table determines how APIC timer
    /// interrupts are delivered.
    pub fn timer_vector_table(&mut self) -> TimerVectorTable {
        match self.inner {
            ApicWrapper::XApic(ref mut apic) => apic.timer_vector_table(),
            ApicWrapper::X2Apic(ref mut apic) => apic.timer_vector_table(),
        }
    }

    /// Set the local vector table for the APIC timer.
    ///
    /// # Unsafety
    /// This can unmask timer interrupts and change the vector they're delivered on. If the timer
    /// interrupt handler is not properly configured, interrupts can cause CPU exceptions.
    pub unsafe fn set_timer_vector_table(&mut self, table: TimerVectorTable) {
        match self.inner {
            ApicWrapper::XApic(ref mut apic) => apic.set_timer_vector_table(table),
            ApicWrapper::X2Apic(ref mut apic) => apic.set_timer_vector_table(table),
        }
    }

    /// Get the divide configuration for the APIC timer. The APIC timer frequency is the processor
    /// bus clock or core crystal clock frequency divided by this value. See section 10.5.4 of
    /// volume 3A of the Intel manual
    pub fn timer_divide_configuration(&mut self) -> DivideConfiguration {
        match self.inner {
            ApicWrapper::XApic(ref mut apic) => apic.timer_divide_configuration(),
            ApicWrapper::X2Apic(ref mut apic) => apic.timer_divide_configuration(),
        }
    }

    /// Set the divide configuration for the APIC timer.
    pub fn set_timer_divide_configuration(&mut self, configuration: DivideConfiguration) {
        match self.inner {
            ApicWrapper::XApic(ref mut apic) => apic.set_timer_divide_configuration(configuration),
            ApicWrapper::X2Apic(ref mut apic) => apic.set_timer_divide_configuration(configuration),
        }
    }

    unsafe fn set_spurious_vector(&mut self, vector: u8) {
        match self.inner {
            ApicWrapper::XApic(ref mut apic) => apic.set_spurious_vector(vector),
            ApicWrapper::X2Apic(ref mut apic) => apic.set_spurious_vector(vector),
        }
    }

    unsafe fn software_enable(&mut self) {
        match self.inner {
            ApicWrapper::XApic(ref mut apic) => apic.software_enable(),
            ApicWrapper::X2Apic(ref mut apic) => apic.software_enable(),
        }
    }

    fn mask_all_interrupts(&mut self) {
        match self.inner {
            ApicWrapper::XApic(ref mut apic) => apic.mask_all_interrupts(),
            ApicWrapper::X2Apic(ref mut apic) => apic.mask_all_interrupts(),
        }
    }
}

impl <'a> Drop for LocalApic<'a> {
    fn drop(&mut self) {
        let apic_id = self.id();
        debug_assert!(self.parent.is_local_apic_used(apic_id));
        self.parent.mark_local_apic_used(apic_id, false);
    }
}

/// Abstraction for different APIC operating modes. Both the xAPIC and x2APIC support most of the
/// same registers, but they're accessed in different ways. This trait hides those implementation
/// details.
trait ApicImplementation {
    /// Returns the local APIC's ID. The APIC ID is only 32 bits in x2APIC mode. In xAPIC mode, only
    /// the lower 8 bits are used.
    fn local_apic_id(&mut self) -> u32;

    /// Returns the local APIC version. This is 8 bits in both x2APIC mode and xAPIC mode.
    fn local_apic_version(&mut self) -> u8;

    /// Mask all interrupts which the local APIC can deliver. This is used to put the APIC in a
    /// well-known state upon initialization.
    fn mask_all_interrupts(&mut self);

    /// Signal that an interrupt has been processed
    fn end_of_interrupt(&mut self);

    /// Gets the current value of the spurious-interrupt vector register
    fn spurious_interrupt_vector_register(&mut self) -> SpuriousInterruptVectorRegister;

    /// Set the spurious-interrupt vector register
    ///
    /// # Unsafety
    /// This can enable or disable the local APIC, affecting interrupt delivery. It can also change
    /// how spurious interrupts are delivered, which is unsafe if interrupt handlers are not properly
    /// configured.
    unsafe fn set_spurious_interrupt_vector_register(
        &mut self,
        vector: SpuriousInterruptVectorRegister,
    );

    /// Set the spurious vector. This is the vector used to deliver spurious interrupts.
    ///
    /// # Unsafety
    /// If the interrupt handler for the given vector is not properly configured, spurious
    /// interrupts could cause CPU exceptions.
    unsafe fn set_spurious_vector(&mut self, vector: u8) {
        // avoid TOCTOU bugs if interrupt handlers cause other SVR modifications
        without_interrupts(|| {
            let mut register = self.spurious_interrupt_vector_register();
            register.set_spurious_vector(vector);
            self.set_spurious_interrupt_vector_register(register);
        });
    }

    /// Get the current spurious vector
    fn spurious_vector(&mut self) -> u8 {
        self.spurious_interrupt_vector_register().spurious_vector()
    }

    /// Software-enable the local APIC. See section 10.4.3 of volume 3A of the Intel manual.
    ///
    /// # Unsafety
    /// If interrupts are enabled, this allows the local APIC to send interrupts. If interrupt
    /// handlers are not properly configured, this can cause CPU exceptions.
    unsafe fn software_enable(&mut self) {
        // avoid TOCTOU bugs if interrupt handlers cause other SVR modifications
        without_interrupts(|| {
            let mut register = self.spurious_interrupt_vector_register();
            register.set_apic_enabled(true);
            self.set_spurious_interrupt_vector_register(register);
        });
    }

    /// Get the initial count for the local APIC timer.
    fn timer_initial_count(&mut self) -> u32;

    /// Set the initial count for the local APIC timer.
    fn set_timer_initial_count(&mut self, count: u32);

    /// Get the timer's current count
    fn timer_current_count(&mut self) -> u32;

    /// Get the local vector table for the APIC timer. This table determines how APIC timer
    /// interrupts are delivered.
    fn timer_vector_table(&mut self) -> TimerVectorTable;

    /// Set the local vector table for the APIC timer.
    ///
    /// # Unsafety
    /// This can unmask timer interrupts and change the vector they're delivered on. If the timer
    /// interrupt handler is not properly configured, interrupts can cause CPU exceptions.
    unsafe fn set_timer_vector_table(&mut self, table: TimerVectorTable);

    /// Get the divide configuration for the APIC timer. The APIC timer frequency is the processor
    /// bus clock or core crystal clock frequency divided by this value. See section 10.5.4 of
    /// volume 3A of the Intel manual
    fn timer_divide_configuration(&mut self) -> DivideConfiguration;

    /// Set the divide configuration for the APIC timer.
    fn set_timer_divide_configuration(&mut self, configuration: DivideConfiguration);
}
