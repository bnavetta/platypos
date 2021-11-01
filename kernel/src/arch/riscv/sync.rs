//! Interrupt-handling spinlock for data structures that may also be accessed in interrupt contexts

use core::hint;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::AtomicU16;
use core::sync::atomic::Ordering;

use lock_api::GuardNoSend;
use lock_api::MappedMutexGuard;
use lock_api::Mutex;
use lock_api::MutexGuard;
use lock_api::RawMutex;
use riscv::register::sstatus;

/// Count of how many reentrant calls to [`disable_interrupts`] have been made
// TODO(smp): this should be per-hart
static INTERRUPT_DISABLE_DEPTH: AtomicU16 = AtomicU16::new(0);

pub struct RawUninterruptibleSpinlock {
    /// Whether or not the spinlock is locked
    locked: AtomicBool,
}

impl RawUninterruptibleSpinlock {
    fn try_lock_weak(&self) -> bool {
        // Disable interrupts _before_ trying to take the lock. Otherwise, there's a deadlock condition where the lock is aquired, and then the hart is immediately
        // interrupted and the interrupt handler spins on the same lock
        disable_interrupts();
        let locked = self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok();
        if !locked {
            // If we didn't actually get the lock, then reenable interrupts. This allows interrupts uninterested in the lock to continue executing while we wait.
            enable_interrupts();
        }
        locked
    }
}

unsafe impl RawMutex for RawUninterruptibleSpinlock {
    // This is largely taken from the https://github.com/rust-osdev/spinning_top implementation, with additional
    // interrupt handling.

    const INIT: RawUninterruptibleSpinlock = RawUninterruptibleSpinlock {
        locked: AtomicBool::new(false),
    };

    // Guards cannot be sent between harts because they control per-hart interrupt state
    type GuardMarker = GuardNoSend;

    fn lock(&self) {
        while !self.try_lock_weak() {
            // Optimization that spinning_top picked up from spin-rs
            // Don't retry the lock looks unlocked
            while self.is_locked() {
                hint::spin_loop();
            }
        }
    }

    fn try_lock(&self) -> bool {
        // spinning_top itself got this from parking_lock
        // The second Ordering is Relaxed because it's what gets used if the compare_exchange fails, in which case we're not accessing any critical data
        // See try_lock_weak for an explanation of the interrupt handling
        disable_interrupts();
        let locked = self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok();
        if !locked {
            enable_interrupts();
        }
        locked
    }

    unsafe fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
        // Reenable interrupts after releasing the lock, so we're not preempted by an interrupt handler that tries to get the lock too soon
        enable_interrupts();
    }

    fn is_locked(&self) -> bool {
        // Also from spinning_top - Relaxed is fine because this isn't used for synchronization, just atomicity
        self.locked.load(Ordering::Relaxed)
    }
}

/// A mutex based on busy-waiting that also manages interrupt state. This makes it safe to use for data that may be accessed during normal execution and during interrupt handling.
/// While the mutex is held, interrupts are disabled to prevent interrupt handlers from deadlocking on the spinlock.
pub type UninterruptibleSpinlock<T> = Mutex<RawUninterruptibleSpinlock, T>;

/// A RAII guard that unlocks the spinlock and reenables interrupts when it goes out of scope.
pub type UninterruptibleSpinlockGuard<'a, T> = MutexGuard<'a, RawUninterruptibleSpinlock, T>;

pub type MappedUninterruptibleSpinlockGuard<'a, T> =
    MappedMutexGuard<'a, RawUninterruptibleSpinlock, T>;

pub fn disable_interrupts() {
    // Relaxed because this is actually going to be a per-hart counter once we have SMP
    let depth = INTERRUPT_DISABLE_DEPTH.fetch_add(1, Ordering::Relaxed);
    if depth == 0 {
        // If interrupts weren't previously disabled, disable them
        unsafe { sstatus::clear_sie() };
    }
}

pub fn enable_interrupts() {
    // Relaxed because this is actually going to be a per-hart counter once we have SMP
    let depth = INTERRUPT_DISABLE_DEPTH.fetch_sub(1, Ordering::Relaxed);
    if depth == 1 {
        // If this is the last call to `enable_interrupts` in the call stack, then we can reenable interrupts
        unsafe { sstatus::set_sie() };
    }
}

// TODO: reentrant lock based on hart id
