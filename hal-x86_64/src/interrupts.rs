use platypos_common::sync::Global;
use x86_64::instructions::interrupts;

use platypos_hal as hal;

#[derive(Debug, Clone, Copy)]
pub struct Controller;

static GLOBAL: Global<Controller> = Global::new();

/// Configure the interrupt controller
pub fn init() -> &'static Controller {
    GLOBAL.init(Controller)
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
