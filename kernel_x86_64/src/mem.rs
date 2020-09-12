//! x86-64 memory

use platypos_kernel::mem::alloc::SlabAllocator;
use platypos_pal as pal;
use platypos_pal::mem::{VirtualAddress, ValidateAddressError, PhysicalAddress};

use crate::platform::Platform;

/// Set up the global memory allocator
#[global_allocator]
static ALLOCATOR: SlabAllocator<Platform> = SlabAllocator::new();

/// The x86-64 memory model
pub struct MemoryModel;

impl pal::mem::MemoryModel<Platform> for MemoryModel {
    const FRAME_SIZE: usize = 4096;

    fn physical_address(raw: usize) -> Result<PhysicalAddress<Platform>, ValidateAddressError> {
        // Physical addresses limited to 48 bits
        if raw & 0xffff_0000_0000_0000 == 0 {
            // Safety: just checked that the address doesn't use the high 48 bits
            Ok(unsafe { PhysicalAddress::new_unchecked(raw) })
        } else {
            Err(ValidateAddressError::invalid_physical_address(raw))
        }
    }

    fn virtual_address(raw: usize) -> Result<VirtualAddress<Platform>, ValidateAddressError> {
        // TODO: could use cpuid to get actual most significant bit
        // Bits 48-63 must equal bit 47
        let high_bits = raw >> 47;
        if high_bits == 0 || high_bits == 0x1ffff {
            // Safety: we just checked that the address is canonical
            Ok(unsafe { VirtualAddress::new_unchecked(raw) })
        } else {
            Err(ValidateAddressError::invalid_virtual_address(raw))
        }
    }
}