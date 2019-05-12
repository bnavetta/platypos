use apic::{LocalApic, LocalVectorTable, TimerMode};
use log::info;
use x86_64::structures::paging::mapper::Mapper;
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame};
use x86_64::VirtAddr;

use super::{INTERRUPT_TIMER, INTERRUPT_SPURIOUS, INTERRUPT_APIC_ERROR};

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
        info!("Local APIC has ID {:#x} and version {:#x}", lapic.id(), lapic.version());

        lapic.set_cmci_table(LocalVectorTable::DISABLED);
        lapic.set_performance_counter_table(LocalVectorTable::NMI);
        lapic.set_lint0_table(LocalVectorTable::DISABLED);
        lapic.set_lint1_table(LocalVectorTable::DISABLED);
        lapic.set_error_table(LocalVectorTable::for_vector_number(INTERRUPT_APIC_ERROR));

        let mut timer_config = lapic.timer_table();
        timer_config.set_masked(false);
        timer_config.set_vector_number(INTERRUPT_TIMER);
        timer_config.set_timer_mode(TimerMode::Periodic);
        lapic.set_timer_table(timer_config);

        lapic.set_timer_initial_count(u32::max_value());
        lapic.set_timer_divide_configuration(16);

        lapic.map_spurious_interrupts(INTERRUPT_SPURIOUS);

        LocalApic::set_local_apic_base(LocalApic::local_apic_base());
        lapic.enable();

        for _ in 0..10 {
            info!("Waiting a bit...");
        }

        info!("LAPIC timer decreased by {}", u32::max_value() - lapic.timer_current_count());
    }
}
