use core::mem;

use raw_cpuid::CpuId;
use x86_64::registers::model_specific::Msr;

use super::LocalApic;
use crate::spurious_interrupt::SpuriousInterruptVectorRegister;
use crate::timer::{DivideConfiguration, TimerVectorTable};

const LAPIC_ID_MSR: Msr = Msr::new(0x802);
const LAPIC_VERSION_MSR: Msr = Msr::new(0x803);
const EOI_MSR: Msr = Msr::new(0x80B);
const SPURIOUS_INTERRUPT_VECTOR_REGISTER_MSR: Msr = Msr::new(0x80F);

const TIMER_LVT_MSR: Msr = Msr::new(0x832);
const THERMAL_SENSOR_LVT_MSR: Msr = Msr::new(0x833);
const PERFORMANCE_MONITORING_LVT_MSR: Msr = Msr::new(0x834);
const LINT0_LVT_MSR: Msr = Msr::new(0x835);
const LINT1_LVT_MSR: Msr = Msr::new(0x836);
const ERROR_LVT_MSR: Msr = Msr::new(0x837);

const TIMER_INITIAL_COUNT_MSR: Msr = Msr::new(0x838);
const TIMER_CURRENT_COUNT_MSR: Msr = Msr::new(0x839);
const TIMER_DIVIDE_CONFIGURATION_MSR: Msr = Msr::new(0x83E);

/// Zeroed-out local vector table, except for the mask bit
const MASKED_LVT_VALUE: u64 = 0x00010000;
/// Local vector table to deliver as a NMI, used for the performance monitoring interrupt
const NMI_LVD_VALUE: u64 = 0x400;

/// Local APIC driver based on the x2 APIC specification
pub struct X2Apic {}

impl X2Apic {
    pub const fn new() -> X2Apic {
        X2Apic {}
    }

    /// Checks if the processor supports x2 APIC mode. Generally, x2 APIC mode is supported on
    /// Nehalem and later processors.
    ///
    /// # Returns
    /// `true` if x2 APIC mode is supported, `false` if not
    pub fn is_supported() -> bool {
        let cpuid = CpuId::new();

        if let Some(feature_info) = cpuid.get_feature_info() {
            feature_info.has_x2apic()
        } else {
            false
        }
    }
}

impl LocalApic for X2Apic {
    fn id(&mut self) -> u32 {
        unsafe { LAPIC_ID_MSR.read() as u32 }
    }

    fn version(&mut self) -> u8 {
        unsafe { LAPIC_VERSION_MSR.read() as u8 }
    }

    fn mask_all_interrupts(&mut self) {
        let mut timer_table = TimerVectorTable::new(0);
        timer_table.set_masked(true);

        unsafe {
            self.set_timer_vector_table(timer_table);
            THERMAL_SENSOR_LVT_MSR.write(MASKED_LVT_VALUE);
            PERFORMANCE_MONITORING_LVT_MSR.write(MASKED_LVT_VALUE);
            LINT0_LVT_MSR.write(MASKED_LVT_VALUE);
            LINT1_LVT_MSR.write(MASKED_LVT_VALUE);
            ERROR_LVT_MSR.write(MASKED_LVT_VALUE);
        }
    }

    fn end_of_interrupt(&mut self) {
        unsafe { EOI_MSR.write(0) }
    }

    fn spurious_interrupt_vector_register(&mut self) -> SpuriousInterruptVectorRegister {
        SpuriousInterruptVectorRegister::new(unsafe {
            SPURIOUS_INTERRUPT_VECTOR_REGISTER_MSR.read()
        } as u32)
    }

    unsafe fn set_spurious_interrupt_vector_register(
        &mut self,
        vector: SpuriousInterruptVectorRegister,
    ) {
        SPURIOUS_INTERRUPT_VECTOR_REGISTER_MSR.write(vector.as_u32() as u64);
    }

    fn timer_initial_count(&mut self) -> u32 {
        unsafe { TIMER_INITIAL_COUNT_MSR.read() as u32 }
    }

    fn set_timer_initial_count(&mut self, count: u32) {
        unsafe { TIMER_INITIAL_COUNT_MSR.write(count as u64) }
    }

    fn timer_current_count(&mut self) -> u32 {
        unsafe { TIMER_CURRENT_COUNT_MSR.read() as u32 }
    }

    fn timer_vector_table(&mut self) -> TimerVectorTable {
        TimerVectorTable::new(unsafe { TIMER_LVT_MSR.read() as u32 })
    }

    unsafe fn set_timer_vector_table(&mut self, table: TimerVectorTable) {
        TIMER_LVT_MSR.write(table.as_u32() as u64);
    }

    fn timer_divide_configuration(&mut self) -> DivideConfiguration {
        // TODO: replace mem::transmure usage
        unsafe { mem::transmute(TIMER_DIVIDE_CONFIGURATION_MSR.read() as u8) }
    }

    fn set_timer_divide_configuration(&mut self, configuration: DivideConfiguration) {
        unsafe { TIMER_DIVIDE_CONFIGURATION_MSR.write(configuration as u8 as u64) }
    }
}
