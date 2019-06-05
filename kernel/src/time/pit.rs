use core::hint::spin_loop;
use core::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use bit_field::BitField;
use log::debug;
use spin::{Mutex, Once};
use x86_64::instructions::interrupts::without_interrupts;
use x86_64::instructions::port::{Port, PortWriteOnly};

// This is approximate, it's 1193181.6666 repeating
const PIT_FREQUENCY_HZ: usize = 1193182;

const NANOS_PER_SECOND: u128 = Duration::SECOND.as_nanos();

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[allow(dead_code)]
pub enum OperatingMode {
    InterruptOnTerminalCount = 0,
    HardwareReTriggerableOneShot = 1,
    RateGenerator = 2,
    SquareWaveGenerator = 3,
    SoftwareTriggeredStrobe = 4,
    HardwareTriggeredStrobe = 5,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[allow(dead_code)]
pub enum AccessMode {
    LowByteOnly = 1,
    HighByteOnly = 2,
    LowByteHighByte = 3,
}

pub struct ProgrammableIntervalTimer {
    channel0: Port<u8>,
    // not supporting channel 1, since it's frequently not implemented
    channel2: Port<u8>,
    command: PortWriteOnly<u8>,
}

impl ProgrammableIntervalTimer {
    fn new() -> ProgrammableIntervalTimer {
        ProgrammableIntervalTimer {
            channel0: Port::new(0x40),
            channel2: Port::new(0x42),
            command: PortWriteOnly::new(0x43),
        }
    }

    /// Configure one of the PIT channels
    ///
    /// # Unsafety
    /// Can cause interrupts, depending on timer configuration
    unsafe fn configure(
        &mut self,
        channel: u8,
        access_mode: AccessMode,
        operating_mode: OperatingMode,
    ) {
        debug_assert!(channel == 0 || channel == 2);

        let mut config: u8 = 0;
        // only support 16-bit binary mode
        config.set_bits(1..4, operating_mode as u8);
        config.set_bits(4..6, access_mode as u8);
        config.set_bits(6..8, channel);

        debug!(
            "Configuring PIT channel {} with access mode {:?} and operating mode {:?}",
            channel, access_mode, operating_mode
        );
        self.command.write(config);
    }

    /// Configure the PIT to send clock interrupts at `frequency`. The timer is configured to use
    /// the LowByte/HighByte access mode and operate as a square wave generator.
    pub unsafe fn configure_timer(&mut self, frequency: usize) {
        let divisor = PIT_FREQUENCY_HZ / frequency;
        self.configure(
            0,
            AccessMode::LowByteHighByte,
            OperatingMode::SquareWaveGenerator,
        );
        // low and high bytes of divisor
        self.channel0.write((divisor & 0xFF) as u8);
        self.channel0.write((divisor >> 8) as u8);
    }

    fn latch_channel(&mut self, channel: u8) {
        assert!(channel == 0 || channel == 2, "Invalid PIT channel");
        unsafe {
            self.command.write(channel << 6);
        }
    }

    fn current_count(&mut self, channel: u8, access_mode: AccessMode) -> u16 {
        without_interrupts(|| {
            if access_mode == AccessMode::LowByteHighByte {
                self.latch_channel(channel);
            }

            let port = match channel {
                0 => &mut self.channel0,
                2 => &mut self.channel2,
                _ => panic!("Invalid PIT channel"),
            };

            match access_mode {
                AccessMode::LowByteOnly => unsafe { port.read() as u16 },
                AccessMode::HighByteOnly => unsafe { (port.read() as u16) << 8 },
                AccessMode::LowByteHighByte => {
                    let low = unsafe { port.read() as u16 };
                    let high = unsafe { port.read() as u16 };
                    low + high << 8
                }
            }
        })
    }
}

const TIMER_FREQUENCY_HZ: usize = 1000;

pub fn init() {
    let mut pit = ProgrammableIntervalTimer::new();
    unsafe {
        pit.configure_timer(TIMER_FREQUENCY_HZ);
    }

    PIT.call_once(|| Mutex::new(pit));
}

static PIT: Once<Mutex<ProgrammableIntervalTimer>> = Once::new();

pub fn current_count() -> u16 {
    let mut pit = PIT.wait().expect("PIT not initialized").lock();
    pit.current_count(0, AccessMode::LowByteHighByte)
}

// TODO: overflow
pub fn pit_delay(duration: Duration) {
    let ticks = ((duration.as_nanos() * TIMER_FREQUENCY_HZ as u128) / NANOS_PER_SECOND) as u16;
    let mut pit = PIT.wait().expect("PIT not initialized").lock();
    let initial = pit.current_count(0, AccessMode::LowByteHighByte);

    while pit.current_count(0, AccessMode::LowByteHighByte) - initial < ticks {
        spin_loop()
    }
}

/// Counter of how many PIT channel 0 interrupts have fired. Note that ticks could be dropped
/// while interrupts are masked, so this isn't reliable as an absolute measure of time.
static COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn pit_timer_callback() {
    COUNTER.fetch_add(1, Ordering::SeqCst);
}

// TODO: PIT interrupts seem broken

/// Sleep for `duration` using the PIT. This relies on the PIT being configured for the expected
/// frequency.
pub fn pit_sleep(duration: Duration) {
    // ticks to wait = elapsed time in seconds * frequency
    // math done in nanoseconds for precision, but converted back to ticks since that's all the
    // resolution we have
    let ticks = (duration.as_nanos() * TIMER_FREQUENCY_HZ as u128) / NANOS_PER_SECOND;

    let initial = COUNTER.load(Ordering::SeqCst);
    while ((COUNTER.load(Ordering::SeqCst) - initial) as u128) < ticks {
        spin_loop()
    }
}
