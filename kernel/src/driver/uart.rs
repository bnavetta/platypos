//! NS16550a UART driver using memory-mapped I/O
use core::fmt;

pub struct Uart {
    base_addr: *mut u8,
}

/// Configuration for a NS16550a UART device
#[derive(Debug, Clone, Copy)]
pub struct UartConfig {
    /// The base address of the UART's MMIO registers
    pub base_address: usize,
    /// The frequency in Hz of the baud rate generator's input clock
    pub clock_frequency: u32,
}

/// Receive Buffer Register offset
const RBR_OFFSET: usize = 0;
/// Transmitter Holding Register offset
const THR_OFFSET: usize = 0;
/// Line Control Register offset
const LCR_OFFSET: usize = 3;
/// FIFO Control Register offset
const FCR_OFFSET: usize = 2;
/// Interrupt Enable Register offset
const IER_OFFSET: usize = 1;
/// Low (least) byte of the divisor latch register
const DLL_OFFSET: usize = 0;
/// High (most) byte of the divisor latch register
const DLM_OFFSET: usize = 1;

impl Uart {
    /// Create and initialize new UART driver with the given configuration
    pub unsafe fn new(config: UartConfig) -> Uart {
        let mut driver = Uart {
            base_addr: config.base_address as *mut u8,
        };

        driver.init(config.clock_frequency);
        driver
    }

    /// Configures the UART device.
    ///
    /// # Safety
    /// This function must only be called once
    unsafe fn init(&mut self, clock_frequency: u32) {
        // Set the word length to 8 bits by setting bits 0 and 1 of the line control register
        let lcr = 0b11;
        self.write(LCR_OFFSET, lcr);
        // Enable the FIFO queue for characters by setting bit 0 of the FIFO control register
        self.write(FCR_OFFSET, 0b1);
        // Enable receiver buffer interrupts by setting bit 0 of the interrupt enable register
        self.write(IER_OFFSET, 0b1);

        // Set the divisor based on the provided clock rate for a signaling rate of 2400 BAUD.
        // According to the NS16500a specification, the formula is:
        //    divisor = ceil(clock_hz / (baud_sps * 16))
        // With QEMU, a safe default for the clock speed if this isn't working is 22.729MHz (22_729_000 Hz), for
        // a divisor of 592.
        let divisor = clock_frequency.unstable_div_ceil(2400 * 16);

        // The divisor register is two bytes written independently.
        let divisor_low = (divisor & 0xff) as u8;
        let divisor_high = (divisor >> 8) as u8;

        // The two divisor registers (DLL for divisor latch least and DLM for divisor latch most) use the same
        // base address as the receiver/transmitter register (the one we get/set data with) and the interrupt
        // enable register. In order to set the divisor, we first have to open the divisor latch by setting the
        // divisor latch access bit in the line control register to 1.
        self.write(LCR_OFFSET, lcr | 1 << 7);
        self.write(DLL_OFFSET, divisor_low);
        self.write(DLM_OFFSET, divisor_high);

        // Now that we've set the divisor latch, clear the DLAB so that we have access to the other registers we need
        self.write(LCR_OFFSET, lcr);
    }

    pub fn write_byte(&mut self, byte: u8) {
        unsafe { self.write(THR_OFFSET, byte) };
    }

    /// Write `value` to the UART MMIO register at `offset`
    unsafe fn write(&mut self, offset: usize, value: u8) {
        self.base_addr.add(offset).write_volatile(value);
    }

    /// Read the value of the UART MMIO register at `offset`
    unsafe fn read(&self, offset: usize) -> u8 {
        self.base_addr.add(offset).read_volatile()
    }
}

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.bytes() {
            self.write_byte(c);
        }
        Ok(())
    }
}

// Safety: the UART driver's raw pointer refers to a MMIO region available from all threads
unsafe impl Send for Uart {}
