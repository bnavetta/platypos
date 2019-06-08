use apic::{Apic, DivideConfiguration, LocalApic, TimerMode};
use log::info;
use spin::Once;
use x86_64::{
    structures::paging::{mapper::Mapper, Page, PageTableFlags, PhysFrame},
    VirtAddr,
};

use core::{cmp::max, time::Duration};

use crate::interrupts::Interrupt;
use crate::topology::processor::processor_topology;

static APIC: Once<Apic> = Once::new();

/// Execute a closure with the local APIC associated with the current processor.
///
/// # Panics
/// * If in a nested call to `with_local_apic`
/// * If the local APIC has not yet been initialized.
pub fn with_local_apic<F, T>(f: F) -> T
where
    F: FnOnce(&mut dyn LocalApic) -> T,
{
    APIC.wait()
        .expect("APIC not initialized")
        .with_local_apic(f)
}

/// Get the local APIC's ID
pub fn local_apic_id() -> u32 {
    APIC.wait().expect("APIC not initialized").local_apic_id()
}

/// Initialize the APIC. This should only be called once, on the bootstrap processor.
pub fn init() {
    let kernel_state = crate::kernel_state();

    let max_apic_id = processor_topology()
        .processors()
        .iter()
        .map(|p| p.apic_id())
        .max()
        .unwrap();

    let apic = APIC.call_once(|| {
        Apic::new(max_apic_id as usize, |base_phys_addr| {
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

    unsafe {
        apic.init(Interrupt::ApicSpurious.as_u8());
    }

    apic.with_local_apic(|lapic| {
        info!(
            "Local APIC has ID {:#x} and version {:#x}",
            lapic.id(),
            lapic.version()
        );
    });
}

pub fn configure_apic_timer(frequency: u32) {
    use crate::time::delay;

    with_local_apic(|lapic| {
        lapic.set_timer_divide_configuration(DivideConfiguration::Divide16);

        let mut lvt = lapic.timer_vector_table();
        lvt.set_masked(true);
        lvt.set_timer_mode(TimerMode::Periodic);
        unsafe {
            lapic.set_timer_vector_table(lvt);
        }

        lapic.set_timer_initial_count(0xffffffff); // -1

        delay(Duration::from_millis(1));
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
    });
}
