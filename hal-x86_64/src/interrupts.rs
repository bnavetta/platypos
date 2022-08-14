use platypos_common::sync::Global;
use raw_cpuid::CpuId;
use x86_64::instructions::interrupts;

use platypos_hal as hal;

mod apic;

#[derive(Debug, Clone, Copy)]
pub struct Controller;

static GLOBAL: Global<Controller> = Global::new();

/// Configure the interrupt controller
pub fn init() -> &'static Controller {
    let cpuid = CpuId::new();
    let has_x2apic = cpuid.get_feature_info().map_or(false, |f| f.has_x2apic());
    if !has_x2apic {
        panic!("x2apic support is required");
    }

    GLOBAL.init(Controller)
}

/// Perform processor-local initialization
pub fn init_local() {
    apic::init_local();
}

impl hal::interrupts::Controller for Controller {
    fn force_enable(&self) {
        interrupts::enable()
    }

    fn force_disable(&self) {
        interrupts::disable()
    }

    fn enabled(&self) -> bool {
        interrupts::are_enabled()
    }

    fn wait(&self) {
        interrupts::enable_and_hlt()
    }
}
