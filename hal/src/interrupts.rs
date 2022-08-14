//! Abstractions for managing interrupt controllers

/// An interrupt controller. Different platforms may have multiple interrupt
/// controllers, different interrupt state per processor, or shared controllers.
pub trait Controller {
    // TODO: should these take &mut self?

    /// Forcibly enable interrupts.
    fn force_enable(&self);

    /// Forcibly disable interrupts.
    fn force_disable(&self);

    /// Test whether or not interrupts are enabled.
    fn enabled(&self) -> bool;

    /// Disable interrupts for as long as the guard is held. When the guard is
    /// dropped, the previous interrupt state is restored.
    fn disable(&self) -> Guard<'_, Self> {
        let enable_flag = self.enabled();

        // If interrupts were enabled before, disable them before creating the
        // guard.
        if enable_flag {
            self.force_disable();
        }

        Guard {
            enable_flag,
            controller: self,
        }
    }

    /// Wait for an interrupt to occur, usually by putting the CPU to sleep.
    /// This will ensure that interrupts are enabled before waiting.
    ///
    /// # Example
    /// ```ignore
    /// // This avoids a race condition where an interrupt
    /// // could occur between the force_disable and wait calls
    /// controller.force_disable();
    /// if nothing_to_do() {
    ///     controller.wait();
    /// }
    /// ```
    fn wait(&self);
}

/// Guard that keeps interrupts disabled while it is held
pub struct Guard<'a, C: Controller + ?Sized> {
    enable_flag: bool,
    controller: &'a C,
}

impl<'a, C: Controller + ?Sized> Drop for Guard<'a, C> {
    fn drop(&mut self) {
        if self.enable_flag {
            self.controller.force_enable();
        }
    }
}
