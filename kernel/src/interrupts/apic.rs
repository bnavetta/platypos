use apic::{LocalApic, LocalVectorTable, TimerMode};
use log::info;
use x86_64::{
    structures::paging::{mapper::Mapper, Page, PageTableFlags, PhysFrame},
    VirtAddr,
};

use super::Interrupt;
use core::time::Duration;
use core::cmp::max;

pub fn local_apic() -> LocalApic {
    unsafe { LocalApic::new(VirtAddr::new(LocalApic::local_apic_base().as_u64()).as_mut_ptr()) }
}

pub fn configure_local_apic() {
    let kernel_state = crate::kernel_state();

    let base_phys_addr = LocalApic::local_apic_base();
    info!("Local APIC located at {:?}", base_phys_addr);
    let base_addr = VirtAddr::new(base_phys_addr.as_u64()); // identity-map for now, probably not the best idea

    // TODO: will possibly need to map LAPIC for each core separately?

    kernel_state.with_page_table(|pt| {
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            pt.active_4kib_mapper()
                .map_to(
                    Page::containing_address(base_addr),
                    PhysFrame::containing_address(base_phys_addr),
                    flags,
                    &mut kernel_state.frame_allocator().page_table_allocator(),
                )
                .expect("Unable to map LAPIC registers")
                .flush();
        }
    });

    unsafe {
        let mut lapic = LocalApic::new(base_addr.as_mut_ptr());
        info!(
            "Local APIC has ID {:#x} and version {:#x}",
            lapic.id(),
            lapic.version()
        );

        lapic.set_cmci_table(LocalVectorTable::DISABLED);
        lapic.set_performance_counter_table(LocalVectorTable::NMI);
        lapic.set_lint0_table(LocalVectorTable::DISABLED);
        lapic.set_lint1_table(LocalVectorTable::DISABLED);
        lapic.set_error_table(LocalVectorTable::for_vector_number(
            Interrupt::ApicError.as_u8(),
        ));

        let mut timer_config = lapic.timer_table();
        timer_config.set_masked(true);
        timer_config.set_vector_number(Interrupt::ApicTimer.as_u8());
        timer_config.set_timer_mode(TimerMode::Periodic);
        lapic.set_timer_table(timer_config);

        lapic.map_spurious_interrupts(Interrupt::ApicSpurious.as_u8());

        LocalApic::set_local_apic_base(LocalApic::local_apic_base());
        lapic.enable();
    }
}

pub fn configure_apic_timer(frequency: u32) {
    use crate::timer::pit::pit_sleep;

    let mut lapic = local_apic();

    lapic.set_timer_divide_configuration(16);
    lapic.set_timer_initial_count(0xffffffff); // -1
    pit_sleep(Duration::from_millis(1));
    let delta = 0xffffffff - lapic.timer_current_count();

    // Multiply by 16 because of divider and 1000 because we slept for a millisecond
    let cpu_bus_frequency = delta * 16 * 1000;

    info!("CPU bus frequency is {}Hz", cpu_bus_frequency);

    let counter_value = max(cpu_bus_frequency / frequency / 16, 16);
    lapic.set_timer_initial_count(counter_value);

    // Now that we've configured the timer, enable interrupts
    let mut timer_table = lapic.timer_table();
    timer_table.set_timer_mode(TimerMode::Periodic);
    timer_table.set_masked(false);
    unsafe { lapic.set_timer_table(timer_table); }
    lapic.set_timer_divide_configuration(16);
}