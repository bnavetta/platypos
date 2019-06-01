use apic::{Apic, LocalApic, DivideConfiguration, TimerMode};
use log::info;
use spin::Once;
use x86_64::{
    structures::paging::{mapper::Mapper, Page, PageTableFlags, PhysFrame},
    VirtAddr,
};

use super::Interrupt;
use core::{cmp::max, time::Duration};

static APIC: Once<Apic> = Once::new();

pub fn local_apic() -> LocalApic<'static> {
    APIC.wait().expect("APIC not initialized").local_apic()
}

pub fn configure_local_apic() {
    let kernel_state = crate::kernel_state();

    let apic = APIC.call_once(|| {
        // TODO: parse ACPI tables to get max APIC ID
        Apic::new(1, |base_phys_addr| {
            let base_addr = VirtAddr::new(base_phys_addr.as_u64()); // identity-map for now, probably not the best idea

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

            base_addr.as_mut_ptr()
        })
    });

    unsafe { apic.init(Interrupt::ApicSpurious.as_u8()); }

    let mut lapic = local_apic();
    info!("Local APIC has ID {:#x} and version {:#x}", lapic.id(), lapic.version());
}

pub fn configure_apic_timer(frequency: u32) {
    use crate::timer::pit::pit_sleep;

    let mut lapic = local_apic();

    lapic.set_timer_divide_configuration(DivideConfiguration::Divide16);

    let mut lvt = lapic.timer_vector_table();
    lvt.set_masked(true);
    lvt.set_timer_mode(TimerMode::Periodic);
    unsafe { lapic.set_timer_vector_table(lvt); }

    lapic.set_timer_initial_count(0xffffffff); // -1

    pit_sleep(Duration::from_millis(1));
    let delta = 0xffffffff - lapic.timer_current_count();

    // Multiply by 16 because of divider and 1000 because we slept for a millisecond
    let cpu_bus_frequency = delta * 16 * 1000;

    info!("CPU bus frequency is {}Hz", cpu_bus_frequency);

    let counter_value = max(cpu_bus_frequency / frequency / 16, 16);
    lapic.set_timer_initial_count(counter_value);

    // Now that we've configured the timer, enable interrupts
    let mut lvt = lapic.timer_vector_table();
    lvt.set_vector(Interrupt::ApicTimer.as_u8());
    lvt.set_masked(false);
    unsafe {
        lapic.set_timer_vector_table(lvt);
    }
}
