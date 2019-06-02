use core::mem;

use bit_field::BitField;
use x86_64::PhysAddr;

use super::{LocalApic, IA32_APIC_BASE_MSR};
use crate::spurious_interrupt::SpuriousInterruptVectorRegister;
use crate::timer::{DivideConfiguration, TimerVectorTable};

/// Zeroed-out local vector table, except for the mask bit
const MASKED_LVT_VALUE: u32 = 0x00010000;
/// Local vector table to deliver as a NMI, used for the performance monitoring interrupt
const NMI_LVD_VALUE: u64 = 0x400;

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalApicRegister {
    LocalApicID = 0x20,
    LocalApicVersion = 0x30,
    EndOfInterrupt = 0xB0,
    SpuriousInterruptVector = 0xF0,

    ErrorStatus = 0x280,

    // LVTs for interrupts
    TimerTable = 0x320,
    ThermalMonitorTable = 0x330,
    PerformanceCounterTable = 0x340,
    LINT0Table = 0x350,
    LINT1Table = 0x360,
    ErrorTable = 0x370,

    TimerInitialCount = 0x380,
    TimerCurrentCount = 0x390,
    TimerDivideConfiguration = 0x3E0,
}

impl Into<usize> for LocalApicRegister {
    fn into(self) -> usize {
        self as usize
    }
}

impl LocalApicRegister {
    /// Get the offset for this register relative to the local APIC's base address, in units of
    /// size `T`.
    #[inline]
    fn offset<T>(&self) -> usize {
        (*self as usize) / mem::size_of::<T>()
    }
}

pub struct XApic {
    base_pointer: *mut u32,
}

impl XApic {
    pub fn local_apic_base() -> PhysAddr {
        // LAPIC base is page-aligned, and the MSR doesn't just contain the address
        PhysAddr::new(unsafe { IA32_APIC_BASE_MSR.read() }).align_down(4096u64)
    }

    pub unsafe fn set_local_apic_base(base: PhysAddr) {
        let mut msr_value = base.as_u64();
        msr_value.set_bit(11, true);
        IA32_APIC_BASE_MSR.write(msr_value);
    }

    pub unsafe fn new(base: *mut u32) -> XApic {
        // unsafe because that might not actually be the base register
        XApic { base_pointer: base }
    }

    unsafe fn register_pointer(&mut self, register: LocalApicRegister) -> *mut u32 {
        self.base_pointer.add(register.offset::<u32>())
    }

    unsafe fn write(&mut self, register: LocalApicRegister, value: u32) {
        // unsafe because changing LAPIC registers can do all sorts of bad stuff
        self.register_pointer(register).write_volatile(value)
    }

    fn read(&mut self, register: LocalApicRegister) -> u32 {
        // safe because we know the register offset is valid
        unsafe { self.register_pointer(register).read_volatile() }
    }

    pub fn error_status(&mut self) -> u32 {
        self.read(LocalApicRegister::ErrorStatus)
    }
}

impl LocalApic for XApic {
    fn id(&mut self) -> u32 {
        self.read(LocalApicRegister::LocalApicID).get_bits(24..32)
    }

    fn version(&mut self) -> u8 {
        (self.read(LocalApicRegister::LocalApicVersion) & 0xFF) as u8
    }

    fn mask_all_interrupts(&mut self) {
        unsafe {
            self.write(LocalApicRegister::TimerTable, MASKED_LVT_VALUE);
            self.write(LocalApicRegister::ThermalMonitorTable, MASKED_LVT_VALUE);
            self.write(LocalApicRegister::PerformanceCounterTable, MASKED_LVT_VALUE);
            self.write(LocalApicRegister::LINT0Table, MASKED_LVT_VALUE);
            self.write(LocalApicRegister::LINT1Table, MASKED_LVT_VALUE);
            self.write(LocalApicRegister::ErrorTable, MASKED_LVT_VALUE);
        }
    }

    fn end_of_interrupt(&mut self) {
        unsafe {
            self.write(LocalApicRegister::EndOfInterrupt, 0);
        }
    }

    fn spurious_interrupt_vector_register(&mut self) -> SpuriousInterruptVectorRegister {
        SpuriousInterruptVectorRegister::new(self.read(LocalApicRegister::SpuriousInterruptVector))
    }

    unsafe fn set_spurious_interrupt_vector_register(
        &mut self,
        vector: SpuriousInterruptVectorRegister,
    ) {
        self.write(LocalApicRegister::SpuriousInterruptVector, vector.as_u32());
    }

    fn timer_initial_count(&mut self) -> u32 {
        self.read(LocalApicRegister::TimerInitialCount)
    }

    fn set_timer_initial_count(&mut self, count: u32) {
        unsafe {
            self.write(LocalApicRegister::TimerInitialCount, count);
        }
    }

    fn timer_current_count(&mut self) -> u32 {
        self.read(LocalApicRegister::TimerCurrentCount)
    }

    fn timer_vector_table(&mut self) -> TimerVectorTable {
        TimerVectorTable::new(self.read(LocalApicRegister::TimerTable))
    }

    unsafe fn set_timer_vector_table(&mut self, table: TimerVectorTable) {
        self.write(LocalApicRegister::TimerTable, table.as_u32());
    }

    fn timer_divide_configuration(&mut self) -> DivideConfiguration {
        let config: u32 = self.read(LocalApicRegister::TimerDivideConfiguration);
        // TODO: avoid mem::transmute
        unsafe { mem::transmute(config as u8) }
    }

    fn set_timer_divide_configuration(&mut self, configuration: DivideConfiguration) {
        unsafe {
            self.write(
                LocalApicRegister::TimerDivideConfiguration,
                configuration as u8 as u32,
            );
        }
    }
}