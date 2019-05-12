#![no_std]

use core::mem;

use bit_field::BitField;
use kutil::log2;
use x86_64::registers::model_specific::Msr;
use x86_64::PhysAddr;
use crate

// https://wiki.osdev.org/APIC

const IA32_APIC_BASE_MSR: Msr = Msr::new(0x1B);

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalApicRegister {
    LocalApicID = 0x20,
    LocalApicVersion = 0x30,
    EndOfInterrupt = 0xB0,
    SpuriousInterruptVector = 0xF0,

    ErrorStatus = 0x280,

    // LVTs for interrupts
    CorrectedMachineCheckInterruptTable = 0x2F0,
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

pub struct LocalVectorTable(u32);

impl LocalVectorTable {
    /// LVT configuration to disable the corresponding interrupt
    pub const DISABLED: LocalVectorTable = LocalVectorTable(0x10000);

    /// LVT configuration to send a NMI for the corresponding interrupt
    pub const NMI: LocalVectorTable = LocalVectorTable(0x400);

    pub fn for_vector_number(vector_number: u8) -> LocalVectorTable {
        // works because vector number is low byte
        LocalVectorTable(vector_number as u32)
    }

    pub fn vector_number(&self) -> u8 {
        self.0.get_bits(0..8) as u8
    }

    pub fn set_vector_number(&mut self, vector_number: u8) {
        self.0.set_bits(0..8, vector_number as u32);
    }

    pub fn masked(&self) -> bool {
        self.0.get_bit(16)
    }

    pub fn set_masked(&mut self, masked: bool) {
        self.0.set_bit(16, masked);
    }

    pub fn timer_mode(&self) -> TimerMode {
        unsafe { mem::transmute(self.0.get_bits(17..19) as u8) }
    }

    pub fn set_timer_mode(&mut self, timer_mode: TimerMode) {
        self.0.set_bits(17..19, timer_mode as u32);
    }

    pub fn delivery_mode(&self) -> DeliveryMode {
        unsafe { mem::transmute(self.0.get_bits(8..11) as u8) }
    }

    pub fn set_delivery_mode(&mut self, delivery_mode: DeliveryMode) {
        self.0.set_bits(8..11, delivery_mode as u32)
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerMode {
    OneShot = 0,
    Periodic = 1,
    TscDeadline = 2,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    Fixed = 0,
    SMI = 2,
    NonMaskableInterrupt = 4,
    Init = 5,
    ExternalInterrupt = 7
}

pub struct LocalApic {
    base_pointer: *mut u32,
}

impl LocalApic {
    pub fn local_apic_base() -> PhysAddr {
        // LAPIC base is page-aligned, and the MSR doesn't just contain the address
        PhysAddr::new(unsafe { IA32_APIC_BASE_MSR.read() }).align_down(4096u64)
    }

    pub unsafe fn set_local_apic_base(base: PhysAddr) {
        let mut msr_value = base.as_u64();
        msr_value.set_bit(11, true);
        IA32_APIC_BASE_MSR.write(msr_value);
    }

    pub unsafe fn new(base: *mut u32) -> LocalApic {
        // unsafe because that might not actually be the base register
        LocalApic { base_pointer: base }
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

    pub fn id(&mut self) -> u32 {
        self.read(LocalApicRegister::LocalApicID)
    }

    pub fn version(&mut self) -> u32 {
        self.read(LocalApicRegister::LocalApicVersion).get_bits(0..8)
    }

    pub unsafe fn enable(&mut self) {
        // TODO: might not actually be worth reading this, if previous settings don't matter
        let mut spurious_interrupt_vector = self.read(LocalApicRegister::SpuriousInterruptVector);
        spurious_interrupt_vector.set_bit(8, true);
        self.write(
            LocalApicRegister::SpuriousInterruptVector,
            spurious_interrupt_vector,
        );
    }

    pub unsafe fn map_spurious_interrupts(&mut self, to_interrupt: u8) {
        let mut spurious_interrupt_vector = self.read(LocalApicRegister::SpuriousInterruptVector);
        spurious_interrupt_vector.set_bits(0..8, to_interrupt as u32);
        self.write(
            LocalApicRegister::SpuriousInterruptVector,
            spurious_interrupt_vector,
        );
    }

    /// Notify the LAPIC that an interrupt has been processed
    pub fn end_of_interrupt(&mut self) {
        unsafe { self.write(LocalApicRegister::EndOfInterrupt, 0); }
    }

    pub fn error_status(&mut self) -> u32 {
        self.read(LocalApicRegister::ErrorStatus)
    }

    // Get/set LVTs for local interrupts

    pub fn cmci_table(&mut self) -> LocalVectorTable {
        LocalVectorTable(self.read(LocalApicRegister::CorrectedMachineCheckInterruptTable))
    }

    pub unsafe fn set_cmci_table(&mut self, table: LocalVectorTable) {
        self.write(LocalApicRegister::CorrectedMachineCheckInterruptTable, table.0)
    }

    pub fn timer_table(&mut self) -> LocalVectorTable {
        LocalVectorTable(self.read(LocalApicRegister::TimerTable))
    }

    pub unsafe fn set_timer_table(&mut self, timer_config: LocalVectorTable) {
        self.write(LocalApicRegister::TimerTable, timer_config.0)
    }

    pub fn thermal_monitor_table(&mut self) -> LocalVectorTable {
        LocalVectorTable(self.read(LocalApicRegister::ThermalMonitorTable))
    }

    pub unsafe fn set_thermal_monitor_table(&mut self, table: LocalVectorTable) {
        self.write(LocalApicRegister::ThermalMonitorTable, table.0)
    }

    pub fn performance_counter_table(&mut self) -> LocalVectorTable {
        LocalVectorTable(self.read(LocalApicRegister::PerformanceCounterTable))
    }

    pub unsafe fn set_performance_counter_table(&mut self, table: LocalVectorTable) {
        self.write(LocalApicRegister::PerformanceCounterTable, table.0)
    }

    pub fn lint0_table(&mut self) -> LocalVectorTable {
        LocalVectorTable(self.read(LocalApicRegister::LINT0Table))
    }

    pub unsafe fn set_lint0_table(&mut self, table: LocalVectorTable) {
        self.write(LocalApicRegister::LINT0Table, table.0)
    }

    pub fn lint1_table(&mut self) -> LocalVectorTable {
        LocalVectorTable(self.read(LocalApicRegister::LINT1Table))
    }

    pub unsafe fn set_lint1_table(&mut self, table: LocalVectorTable) {
        self.write(LocalApicRegister::LINT1Table, table.0)
    }

    pub fn error_table(&mut self) -> LocalVectorTable {
        LocalVectorTable(self.read(LocalApicRegister::ErrorTable))
    }

    pub unsafe fn set_error_table(&mut self, table: LocalVectorTable) {
        self.write(LocalApicRegister::ErrorTable, table.0)
    }

    pub fn timer_initial_count(&mut self) -> u32 {
        self.read(LocalApicRegister::TimerInitialCount)
    }

    pub fn set_timer_initial_count(&mut self, count: u32) {
        unsafe { self.write(LocalApicRegister::TimerInitialCount, count); }
    }

    pub fn timer_current_count(&mut self) -> u32 {
        self.read(LocalApicRegister::TimerCurrentCount)
    }

    /// Configure the divisor for the LAPIC timer. The processor's bus clock or core crystal clock
    /// will be divided by this factor to determine the LAPIC timer frequency
    pub fn set_timer_divide_configuration(&mut self, divide_by: u8) {
        assert!(divide_by.is_power_of_two() || divide_by == 1, "Invalid timer divisor {}", divide_by);

        // Based on figure 10-10 in section 10.5.4 of the Intel manual
        let value = (log2(divide_by as usize) as u8).wrapping_sub(1);

        // This is what the table says, it seems perverse
        let mut register_value: u32 = 0;
        register_value.set_bits(0..2, value.get_bits(0..2).into());
        register_value.set_bit(3, value.get_bit(2));

        // TODO: unit test this

        unsafe { self.write(LocalApicRegister::TimerDivideConfiguration, register_value); }
    }
}
