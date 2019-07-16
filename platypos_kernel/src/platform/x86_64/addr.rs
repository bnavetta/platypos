use x86_64::{VirtAddr, PhysAddr};

include!("../shared/addr.rs");

impl VirtualAddress {
    pub fn is_valid(&self) -> bool {
        VirtAddr::try_new(self.0 as u64).is_ok()
    }
}

impl Into<VirtAddr> for VirtualAddress {
    fn into(self) -> VirtAddr {
        VirtAddr::new(self.0 as u64)
    }
}

impl PhysicalAddress {
    pub fn is_valid(&self) -> bool {
        PhysAddr::try_new(self.0 as u64).is_ok()
    }
}

impl Into<PhysAddr> for PhysicalAddress {
    fn into(self) -> PhysAddr {
        PhysAddr::new(self.0 as u64)
    }
}